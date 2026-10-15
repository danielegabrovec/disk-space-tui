//! Disegno dell'interfaccia.

use crate::app::{App, Modal, Row, Sort, Tab, Tone};
use crate::assess::{self, Level};
use crate::i18n::{file_manager, key_del, key_shift_del, trash_name};
use crate::scan::Kind;
use crate::util::{bar, fmt_count, fmt_pct, fmt_size, plural, ratio_color};
use crate::{tr, trf};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, BorderType, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap,
};
use std::path::Path;
use std::sync::atomic::Ordering::Relaxed;

const SPIN: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
const DIM: Style = Style::new().fg(Color::DarkGray);
const ACCENT: Color = Color::Cyan;
const HIGHLIGHT: Style = Style::new()
    .bg(Color::Rgb(38, 52, 78))
    .add_modifier(Modifier::BOLD);
/// Larghezza minima per mostrare il pannello di dettaglio a destra.
const WIDE: u16 = 110;
const DETAIL_W: u16 = 46;

pub fn draw(f: &mut Frame, app: &mut App) {
    app.hits = Default::default();
    let dh = (app.drives.len().clamp(1, 4) + 2) as u16;
    let [head, tabs, body, status, keys] = Layout::vertical([
        Constraint::Length(dh),
        Constraint::Length(1),
        Constraint::Min(6),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(f.area());

    drives(f, head, app);
    tabs_bar(f, tabs, app);
    match app.tab {
        Tab::Esplora if app.scanning.is_some() => scanning(f, body, app),
        Tab::Esplora if app.tree.is_some() => explore(f, body, app),
        Tab::Esplora => home(f, body, app),
        Tab::FileGrandi => big_files(f, body, app),
        Tab::Pulizia => cleanup(f, body, app),
    }
    status_bar(f, status, app);
    keys_bar(f, keys, app);
    modal(f, app);
}

// ---------------------------------------------------------------- pezzi comuni

fn clamp_state(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
    } else {
        state.select(Some(state.selected().unwrap_or(0).min(len - 1)));
    }
}

fn panel(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(DIM)
        .title(title.into())
}

fn focused_panel(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ACCENT))
        .title(title.into())
}

fn details_title() -> String {
    format!(" {} ", tr!("Dettagli", "Details"))
}

/// Divide il corpo in lista + dettaglio (a destra se c'è spazio, altrimenti sotto).
fn split_detail(area: Rect, bottom_h: u16) -> (Rect, Rect) {
    if area.width >= WIDE {
        let [l, r] =
            Layout::horizontal([Constraint::Min(40), Constraint::Length(DETAIL_W)]).areas(area);
        (l, r)
    } else {
        let [l, r] =
            Layout::vertical([Constraint::Min(5), Constraint::Length(bottom_h)]).areas(area);
        (l, r)
    }
}

/// Disegna una lista con selezione e restituisce l'area interna (per i clic).
fn render_list(
    f: &mut Frame,
    area: Rect,
    block: Block<'static>,
    items: Vec<ListItem<'static>>,
    state: &mut ListState,
) -> Rect {
    let inner = block.inner(area);
    clamp_state(state, items.len());
    let list = List::new(items)
        .block(block)
        .highlight_style(HIGHLIGHT)
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, area, state);
    inner
}

fn chip(key: &str) -> Span<'static> {
    Span::styled(
        format!(" {key} "),
        Style::new().fg(Color::Black).bg(Color::Gray).bold(),
    )
}

fn kv(k: &str, v: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{k:<15}"), DIM),
        Span::styled(v, Style::new().bold()),
    ])
}

fn drive_of<'a>(app: &'a App, p: &Path) -> Option<&'a crate::app::Drive> {
    app.drives
        .iter()
        .filter(|d| p.starts_with(&d.mount))
        .max_by_key(|d| d.mount.as_os_str().len())
}

fn enter() -> &'static str {
    tr!("Invio", "Enter")
}

fn space() -> &'static str {
    tr!("Spazio", "Space")
}

fn spinner(app: &App) -> &'static str {
    SPIN[(app.tick as usize / 2) % SPIN.len()]
}

// ---------------------------------------------------------------- testata e schede

fn drives(f: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = app
        .drives
        .iter()
        .take(4)
        .map(|d| {
            let used = d.total.saturating_sub(d.free);
            let r = if d.total > 0 {
                used as f64 / d.total as f64
            } else {
                0.0
            };
            Line::from(vec![
                Span::styled(format!(" {:<5}", d.mount.display()), Style::new().bold()),
                Span::styled(bar(r, 24), Style::new().fg(ratio_color(r))),
                Span::styled(format!(" {:>6} ", fmt_pct(r)), Style::new().bold()),
                Span::styled(
                    trf!(" liberi {:>9}", " free {:>9}", fmt_size(d.free)),
                    Style::new().fg(Color::Green).bold(),
                ),
                Span::styled(trf!("  su {}", "  of {}", fmt_size(d.total)), DIM),
                Span::styled(format!("  · {}", d.fs), DIM),
            ])
        })
        .collect();
    let title = Line::from(vec![Span::styled(
        " Disk Space TUI ",
        Style::new().bold().fg(ACCENT),
    )]);
    f.render_widget(Paragraph::new(lines).block(panel(title)), area);
}

fn tabs_bar(f: &mut Frame, area: Rect, app: &mut App) {
    let selectable: u64 = app
        .targets
        .iter()
        .filter(|t| t.actionable())
        .filter_map(|t| t.size)
        .sum();
    let labels = [
        (Tab::Esplora, format!(" 1  {} ", tr!("Esplora", "Explore"))),
        (
            Tab::FileGrandi,
            if app.big_files.is_empty() {
                format!(" 2  {} ", tr!("File grandi", "Big files"))
            } else {
                format!(
                    " 2  {} ({}) ",
                    tr!("File grandi", "Big files"),
                    app.big_files.len()
                )
            },
        ),
        (
            Tab::Pulizia,
            format!(
                " 3  {} ({}) ",
                tr!("Pulizia", "Cleanup"),
                fmt_size(selectable)
            ),
        ),
    ];
    let mut spans = vec![Span::raw(" ")];
    let mut x = area.x + 1;
    for (tab, label) in labels {
        let style = if app.tab == tab {
            Style::new().fg(Color::Black).bg(ACCENT).bold()
        } else {
            Style::new().fg(Color::Gray).bg(Color::Rgb(40, 40, 48))
        };
        let s = Span::styled(label, style);
        let w = s.width() as u16;
        app.hits.tabs.push((
            Rect {
                x,
                y: area.y,
                width: w,
                height: 1,
            },
            tab,
        ));
        x += w + 1;
        spans.push(s);
        spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ---------------------------------------------------------------- schermata iniziale

fn home(f: &mut Frame, area: Rect, app: &mut App) {
    let [left, right] = if area.width >= WIDE {
        Layout::horizontal([Constraint::Min(50), Constraint::Length(DETAIL_W + 8)]).areas(area)
    } else {
        Layout::vertical([Constraint::Min(5), Constraint::Length(7)]).areas(area)
    };
    let items: Vec<ListItem> = app
        .places
        .iter()
        .map(|p| {
            let mut spans = vec![Span::styled(
                format!("{:<26}", p.label),
                Style::new().bold(),
            )];
            if let Some((used, total)) = p.usage {
                let r = if total > 0 {
                    used as f64 / total as f64
                } else {
                    0.0
                };
                spans.push(Span::styled(bar(r, 16), Style::new().fg(ratio_color(r))));
                spans.push(Span::raw(format!(" {:>6}  ", fmt_pct(r))));
            }
            spans.push(Span::styled(p.hint.clone(), DIM));
            ListItem::new(Line::from(spans))
        })
        .collect();
    let block = focused_panel(
        Line::from(tr!(
            " Cosa vuoi analizzare? ",
            " What do you want to analyze? "
        ))
        .bold(),
    );
    let inner = render_list(f, left, block, items, &mut app.home_state);
    app.hits.list = Some((inner, app.home_state.offset()));

    let step = |n: &str, t: &str, d: &str| {
        vec![
            Line::from(vec![
                Span::styled(
                    format!(" {n} "),
                    Style::new().fg(Color::Black).bg(ACCENT).bold(),
                ),
                Span::raw(" "),
                Span::styled(t.to_string(), Style::new().bold()),
            ]),
            Line::from(Span::styled(format!("    {d}"), DIM)),
            Line::from(""),
        ]
    };
    let mut lines = Vec::new();
    lines.extend(step(
        "1",
        tr!("Scegli un disco o una cartella", "Pick a drive or a folder"),
        tr!(
            "Invio o doppio clic · G = scrivi un percorso",
            "Enter or double-click · G = type a path"
        ),
    ));
    lines.extend(step(
        "2",
        tr!(
            "Esplora le cartelle più grandi",
            "Explore the biggest folders"
        ),
        tr!(
            "I Dettagli dicono se si può eliminare",
            "Details tell you if it is safe to delete"
        ),
    ));
    lines.extend(step(
        "3",
        tr!("Pulisci", "Clean up"),
        tr!(
            "Scheda 3: temporanei, cache, build",
            "Tab 3: temporary files, caches, builds"
        ),
    ));
    lines.push(Line::from(vec![
        Span::styled(tr!(" Mouse: ", " Mouse: "), Style::new().bold()),
        Span::styled(
            tr!(
                "clic, doppio clic, rotella, destro = azioni",
                "click, double-click, wheel, right = actions"
            ),
            DIM,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            tr!(" Nessun rischio: ", " No surprises: "),
            Style::new().bold(),
        ),
        Span::styled(
            tr!(
                "ogni eliminazione chiede conferma.",
                "every deletion asks for confirmation."
            ),
            DIM,
        ),
    ]));
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(panel(tr!(" Come funziona ", " How it works "))),
        right,
    );
}

// ---------------------------------------------------------------- analisi in corso

fn scanning(f: &mut Frame, area: Rect, app: &App) {
    let Some(s) = &app.scanning else { return };
    let files = s.prog.files.load(Relaxed);
    let dirs = s.prog.dirs.load(Relaxed);
    let bytes = s.prog.bytes.load(Relaxed);
    let errors = s.prog.errors.load(Relaxed);
    let current = s.prog.current.lock().map(|c| c.clone()).unwrap_or_default();
    let el = s.started.elapsed().as_secs_f64();
    let block = focused_panel(
        Line::from(trf!(
            " {} Analisi in corso ",
            " {} Analyzing ",
            spinner(app)
        ))
        .bold(),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [text_area, gauge_area] =
        Layout::vertical([Constraint::Length(9), Constraint::Length(3)]).areas(inner);
    let rate = fmt_count((files as f64 / el.max(0.1)) as u64);
    let lines = vec![
        Line::from(""),
        kv(tr!("  Cartella", "  Folder"), s.root.display().to_string()),
        kv(
            tr!("  Trovati", "  Found"),
            trf!(
                "{} in {} file e {} cartelle",
                "{} in {} files and {} folders",
                fmt_size(bytes),
                fmt_count(files),
                fmt_count(dirs)
            ),
        ),
        kv(
            tr!("  Tempo", "  Time"),
            trf!(
                "{el:.0} s  ·  {rate} file al secondo",
                "{el:.0} s  ·  {rate} files per second"
            ),
        ),
        kv(tr!("  Non leggibili", "  Unreadable"), fmt_count(errors)),
        Line::from(vec![
            Span::styled(format!("{:<15}", tr!("  Sto leggendo", "  Reading")), DIM),
            Span::styled(current, DIM),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            chip("Esc"),
            Span::styled(
                tr!(
                    " interrompi e guarda i dati parziali",
                    " stop and look at the partial data"
                ),
                DIM,
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), text_area);
    if let Some(used) = s.drive_used.filter(|u| *u > 0) {
        let r = (bytes as f64 / used as f64).min(1.0);
        let g = Gauge::default()
            .block(panel(tr!(" Avanzamento stimato ", " Estimated progress ")))
            .gauge_style(Style::new().fg(ACCENT).bg(Color::Rgb(30, 30, 36)))
            .ratio(r)
            .label(fmt_pct(r));
        f.render_widget(g, gauge_area);
    }
}

// ---------------------------------------------------------------- Esplora

fn crumbs(f: &mut Frame, area: Rect, app: &mut App) {
    let mut spans = vec![Span::styled(tr!(" Percorso: ", " Path: "), DIM)];
    let mut x = area.x + spans[0].width() as u16;
    let mut parts: Vec<String> = vec![app.root_path.display().to_string()];
    if let Some(t) = &app.tree {
        let mut n = t;
        for &i in &app.nav {
            n = &n.children[i];
            parts.push(n.name.clone());
        }
    }
    let last = parts.len() - 1;
    for (depth, p) in parts.into_iter().enumerate() {
        if depth > 0 {
            let sep = Span::styled(" › ", DIM);
            x += sep.width() as u16;
            spans.push(sep);
        }
        let style = if depth == last {
            Style::new().bold().fg(Color::White)
        } else {
            Style::new().fg(ACCENT).underlined()
        };
        let s = Span::styled(p, style);
        let w = s.width() as u16;
        if depth < last {
            app.hits.crumbs.push((
                Rect {
                    x,
                    y: area.y,
                    width: w,
                    height: 1,
                },
                depth,
            ));
        }
        x += w;
        spans.push(s);
    }
    if !app.nav.is_empty() {
        spans.push(Span::styled(
            tr!(
                "   (clic su un pezzo del percorso per tornarci)",
                "   (click a part of the path to go back there)"
            ),
            DIM,
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn explore(f: &mut Frame, area: Rect, app: &mut App) {
    let [crumb_area, rest] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(4)]).areas(area);
    crumbs(f, crumb_area, app);
    let (list_area, detail_area) = split_detail(rest, 6);

    let tags = app.view_tags().to_vec();
    let visible = app.visible();
    let Some(node) = app.view() else { return };
    let parent = node.size.max(1) as f64;
    let items: Vec<ListItem<'static>> = visible
        .iter()
        .map(|&i| {
            let c = &node.children[i];
            let r = c.size as f64 / parent;
            let mut spans = vec![
                Span::styled(format!("{:>10} ", fmt_size(c.size)), Style::new().bold()),
                Span::styled(
                    bar(r, 12),
                    Style::new().fg(ratio_color(1.0 - (1.0 - r).powi(3))),
                ),
                Span::styled(format!(" {:>6}  ", fmt_pct(r)), DIM),
            ];
            match c.kind {
                Kind::Dir => spans.push(Span::styled(
                    format!("▸ {}", c.name),
                    Style::new().fg(Color::LightBlue).bold(),
                )),
                Kind::File => spans.push(Span::raw(format!("   {}", c.name))),
                Kind::Group => spans.push(Span::styled(format!("   {}", c.name), DIM.italic())),
            }
            if c.denied {
                spans.push(Span::styled(
                    tr!("  non leggibile", "  unreadable"),
                    Style::new().fg(Color::Red),
                ));
            }
            if let Some(tag) = tags.get(i).and_then(|l| l.tag()) {
                let l = tags[i];
                spans.push(Span::styled(
                    format!("  {} {tag}", l.icon()),
                    Style::new().fg(l.color()),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let (count, size, files) = (visible.len(), node.size, node.files);
    let sort = if app.sort == Sort::Size {
        tr!("per dimensione", "by size")
    } else {
        tr!("per nome", "by name")
    };
    let mut title = vec![Span::styled(
        trf!(
            " {} · ordinate {sort} ",
            " {} · sorted {sort} ",
            plural(count, tr!("voce", "item"), tr!("voci", "items"))
        ),
        Style::new().bold(),
    )];
    if app.filter_edit || !app.filter.is_empty() {
        title.push(Span::styled(
            trf!(
                " cerca: «{}»{} ",
                " search: “{}”{} ",
                app.filter,
                if app.filter_edit { "▌" } else { "" }
            ),
            Style::new().fg(Color::Black).bg(Color::Yellow),
        ));
    }
    let block = focused_panel(Line::from(title)).title_bottom(
        Line::from(trf!(
            " in questa cartella: {} · {} file ",
            " in this folder: {} · {} files ",
            fmt_size(size),
            fmt_count(files)
        ))
        .right_aligned()
        .style(DIM),
    );
    let empty = items.is_empty();
    let inner = render_list(f, list_area, block, items, &mut app.explore_state);
    if empty {
        let msg = if app.filter.is_empty() {
            tr!("Cartella vuota.", "Empty folder.")
        } else {
            tr!(
                "Nessun nome contiene il testo cercato (Esc per togliere il filtro).",
                "No name contains the search text (Esc removes the filter)."
            )
        };
        f.render_widget(Paragraph::new(Span::styled(msg, DIM)), inner);
    }
    app.hits.list = Some((inner, app.explore_state.offset()));

    // Dettaglio della voce selezionata.
    let sel = app
        .selected_child()
        .map(|c| (c.name.clone(), c.size, c.files, c.kind, c.denied));
    let Some((name, csize, cfiles, kind, denied)) = sel else {
        f.render_widget(
            Paragraph::new("").block(panel(details_title())),
            detail_area,
        );
        return;
    };
    let path = if kind == Kind::Group {
        app.view_path()
    } else {
        app.view_path().join(&name)
    };
    let disk = drive_of(app, &path).map(|d| d.total).unwrap_or(0);
    let what = match kind {
        Kind::Dir => trf!("Cartella · {} file", "Folder · {} files", fmt_count(cfiles)),
        Kind::File => tr!("File", "File").into(),
        Kind::Group => trf!(
            "{} file piccoli raggruppati",
            "{} small files grouped together",
            fmt_count(cfiles)
        ),
    };
    let head = Line::from(vec![
        Span::styled(name.clone(), Style::new().bold().fg(Color::White)),
        Span::styled(format!("  {}", fmt_size(csize)), Style::new().bold()),
    ]);
    let mut facts = vec![
        Line::from(Span::styled(what, DIM)),
        Line::from(""),
        kv(tr!("Dimensione", "Size"), fmt_size(csize)),
        kv(
            tr!("Della cartella", "Of the folder"),
            fmt_pct(csize as f64 / size.max(1) as f64),
        ),
    ];
    if disk > 0 {
        facts.push(kv(
            tr!("Del disco", "Of the drive"),
            fmt_pct(csize as f64 / disk as f64),
        ));
    }
    if denied {
        facts.push(Line::from(Span::styled(
            tr!(
                "Non leggibile: servono permessi di amministratore.",
                "Unreadable: administrator rights are needed."
            ),
            Style::new().fg(Color::Red),
        )));
    }
    let actions: Vec<(String, String)> = match kind {
        Kind::Dir => vec![
            (enter().into(), tr!("apri", "open").into()),
            (space().into(), tr!("tutte le azioni", "all actions").into()),
            (key_del().into(), trash_name().into()),
        ],
        Kind::File => vec![
            (space().into(), tr!("tutte le azioni", "all actions").into()),
            (key_del().into(), trash_name().into()),
            ("O".into(), file_manager().into()),
        ],
        Kind::Group => vec![("O".into(), trf!("apri in {}", "open in {}", file_manager()))],
    };
    let verdict = if kind == Kind::Group {
        None
    } else {
        Some(app.verdict(&path, kind == Kind::Dir))
    };
    detail_panel(f, detail_area, head, facts, verdict, &path, &actions);
}

/// Pannello «Dettagli»: fatti, valutazione «si può eliminare?», azioni, percorso.
/// Se c'è poco spazio (pannello in basso) restano solo intestazione e valutazione.
fn detail_panel(
    f: &mut Frame,
    area: Rect,
    head: Line<'static>,
    facts: Vec<Line<'static>>,
    verdict: Option<assess::Verdict>,
    path: &Path,
    actions: &[(String, String)],
) {
    let roomy = area.width >= DETAIL_W - 2 && area.height > 12;
    let mut lines = vec![head];
    if roomy {
        lines.extend(facts);
    }
    if let Some(v) = verdict {
        if roomy {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(vec![Span::styled(
            format!(" {} {} ", v.level.icon(), v.title),
            Style::new().fg(Color::Black).bg(v.level.color()).bold(),
        )]));
        lines.push(Line::from(Span::styled(
            v.text,
            Style::new().fg(if v.level == Level::Ignoto {
                Color::Gray
            } else {
                v.level.color()
            }),
        )));
    }
    if roomy {
        lines.push(Line::from(""));
        for (k, d) in actions {
            lines.push(Line::from(vec![
                chip(k),
                Span::styled(format!(" {d}"), DIM),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(path.display().to_string(), DIM)));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(panel(details_title())),
        area,
    );
}

// ---------------------------------------------------------------- File grandi

fn big_files(f: &mut Frame, area: Rect, app: &mut App) {
    if app.big_files.is_empty() {
        let msg = if app.scanning.is_some() {
            tr!("Analisi in corso…", "Analyzing…")
        } else {
            tr!(
                "Qui compaiono i 500 file più grandi dopo un'analisi.\n\nVai su  1 Esplora  e scegli un disco o una cartella.",
                "The 500 biggest files appear here after an analysis.\n\nGo to  1 Explore  and pick a drive or a folder."
            )
        };
        f.render_widget(
            Paragraph::new(msg)
                .block(panel(format!(" {} ", tr!("File grandi", "Big files"))))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let (list_area, detail_area) = split_detail(area, 6);
    let root = app.root_path.clone();
    let items: Vec<ListItem<'static>> = app
        .big_files
        .iter()
        .map(|(p, s)| {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let dir = p
                .parent()
                .map(|d| d.strip_prefix(&root).unwrap_or(d).display().to_string())
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>10}  ", fmt_size(*s)), Style::new().bold()),
                Span::raw(name),
                Span::styled(format!("   {dir}"), DIM),
            ]))
        })
        .collect();
    let total: u64 = app.big_files.iter().map(|f| f.1).sum();
    let block = focused_panel(
        Line::from(trf!(
            " I {} file più grandi in {} ",
            " The {} biggest files in {} ",
            app.big_files.len(),
            root.display()
        ))
        .bold(),
    )
    .title_bottom(
        Line::from(trf!(
            " insieme fanno {} ",
            " together they take {} ",
            fmt_size(total)
        ))
        .right_aligned()
        .style(DIM),
    );
    let inner = render_list(f, list_area, block, items, &mut app.big_state);
    app.hits.list = Some((inner, app.big_state.offset()));

    let Some((path, size)) = app
        .big_state
        .selected()
        .and_then(|i| app.big_files.get(i))
        .cloned()
    else {
        return;
    };
    let disk = drive_of(app, &path).map(|d| d.total).unwrap_or(0);
    let head = Line::from(vec![
        Span::styled(
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Style::new().bold().fg(Color::White),
        ),
        Span::styled(format!("  {}", fmt_size(size)), Style::new().bold()),
    ]);
    let mut facts = vec![
        Line::from(Span::styled(tr!("File", "File"), DIM)),
        Line::from(""),
        kv(tr!("Dimensione", "Size"), fmt_size(size)),
    ];
    if disk > 0 {
        facts.push(kv(
            tr!("Del disco", "Of the drive"),
            fmt_pct(size as f64 / disk as f64),
        ));
    }
    let v = app.verdict(&path, false);
    let actions = [
        (
            enter().to_string(),
            tr!("tutte le azioni", "all actions").to_string(),
        ),
        (
            "T".to_string(),
            tr!("mostra nella cartella", "show in its folder").to_string(),
        ),
        (key_del().to_string(), trash_name().to_string()),
    ];
    detail_panel(f, detail_area, head, facts, Some(v), &path, &actions);
}

// ---------------------------------------------------------------- Pulizia

fn cleanup(f: &mut Frame, area: Rect, app: &mut App) {
    let (list_area, detail_area) = split_detail(area, 6);
    let rows = app.clean_rows();
    let width = list_area.width.saturating_sub(4) as usize;
    let measuring_label = tr!("misuro…", "measuring…");
    let items: Vec<ListItem<'static>> = rows
        .iter()
        .map(|r| match r {
            Row::Header(g, total) => {
                let color = g.safety().color();
                let left = format!(" {} ", g.title());
                let right = format!(" {} ", fmt_size(*total));
                let pad = width.saturating_sub(left.chars().count() + right.chars().count() + 2);
                ListItem::new(Line::from(vec![
                    Span::styled(left, Style::new().fg(color).bold()),
                    Span::styled("─".repeat(pad), Style::new().fg(color).dim()),
                    Span::styled(right, Style::new().fg(color).bold()),
                ]))
            }
            Row::Item(i) => {
                let t = &app.targets[*i];
                let (check, cstyle) = if !t.actionable() {
                    ("   ", DIM)
                } else if t.selected {
                    ("[✓]", Style::new().fg(Color::Yellow).bold())
                } else {
                    ("[ ]", Style::new().fg(Color::Gray))
                };
                let size = match t.size {
                    None => measuring_label.to_string(),
                    Some(s) => format!("{}{}", if t.partial { "≥" } else { "" }, fmt_size(s)),
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {check} "), cstyle),
                    Span::styled(format!("{size:>11}  "), Style::new().bold()),
                    Span::styled(
                        t.name.clone(),
                        if t.selected {
                            Style::new().fg(Color::Yellow)
                        } else {
                            Style::new()
                        },
                    ),
                ]))
            }
        })
        .collect();
    let sel: Vec<_> = app.targets.iter().filter(|t| t.selected).collect();
    let sel_total: u64 = sel.iter().filter_map(|t| t.size).sum();
    let summary = if sel.is_empty() {
        Line::from(vec![Span::styled(
            tr!(
                " Spazio = seleziona · A = tutte le sicure · Invio = pulisci la voce evidenziata ",
                " Space = select · A = all safe ones · Enter = clean the highlighted entry "
            ),
            DIM,
        )])
    } else {
        Line::from(vec![
            Span::styled(
                format!(
                    " ✓ {} · {} ",
                    plural(
                        sel.len(),
                        tr!("voce selezionata", "entry selected"),
                        tr!("voci selezionate", "entries selected")
                    ),
                    fmt_size(sel_total)
                ),
                Style::new().fg(Color::Black).bg(Color::Yellow).bold(),
            ),
            Span::raw(" "),
            chip(enter()),
            Span::styled(tr!(" pulisci ", " clean "), Style::new().bold()),
        ])
    };
    let measuring = app.targets.iter().filter(|t| t.size.is_none()).count();
    let mut title = vec![Span::styled(
        tr!(" Cosa si può pulire ", " What can be cleaned "),
        Style::new().bold(),
    )];
    if measuring > 0 {
        title.push(Span::styled(
            trf!(
                " {} misuro ancora {}… ",
                " {} still measuring {}… ",
                spinner(app),
                plural(measuring, tr!("voce", "entry"), tr!("voci", "entries"))
            ),
            DIM,
        ));
    }
    let block = focused_panel(Line::from(title)).title_bottom(summary.right_aligned());
    let inner = render_list(f, list_area, block, items, &mut app.tgt_state);
    app.hits.list = Some((inner, app.tgt_state.offset()));

    let Some(i) = app.selected_target() else {
        f.render_widget(
            Paragraph::new("").block(panel(details_title())),
            detail_area,
        );
        return;
    };
    let t = &app.targets[i];
    let level = assess::from_safety(t.safety);
    let mut lines = vec![
        Line::from(Span::styled(
            t.name.clone(),
            Style::new().bold().fg(Color::White),
        )),
        Line::from(""),
        kv(
            tr!("Dimensione", "Size"),
            t.size
                .map(fmt_size)
                .unwrap_or_else(|| measuring_label.into()),
        ),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!(" {} {} ", level.icon(), t.safety.label()),
            Style::new().fg(Color::Black).bg(t.safety.color()).bold(),
        )]),
        Line::from(Span::styled(t.detail.clone(), Style::new().fg(Color::Gray))),
        Line::from(""),
        Line::from(vec![
            Span::styled(tr!("Cosa fa: ", "What it does: "), Style::new().bold()),
            Span::raw(t.action.describe()),
        ]),
    ];
    if detail_area.width >= DETAIL_W - 2 {
        lines.push(Line::from(""));
        let acts: Vec<(&str, &str)> = if t.actionable() {
            vec![
                (space(), tr!("seleziona", "select")),
                (enter(), tr!("pulisci", "clean")),
                ("N", tr!("analizza la cartella", "analyze the folder")),
                ("M", tr!("tutte le azioni", "all actions")),
            ]
        } else {
            vec![
                ("N", tr!("analizza la cartella", "analyze the folder")),
                ("O", file_manager()),
            ]
        };
        for (k, d) in acts {
            lines.push(Line::from(vec![
                chip(k),
                Span::styled(format!(" {d}"), DIM),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(t.path.display().to_string(), DIM)));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(panel(details_title())),
        detail_area,
    );
}

// ---------------------------------------------------------------- barre in basso

fn status_bar(f: &mut Frame, area: Rect, app: &App) {
    let line = match &app.busy {
        Some(b) => Line::from(vec![
            Span::styled(
                format!(" {} ", spinner(app)),
                Style::new().fg(Color::Yellow),
            ),
            Span::styled(b.clone(), Style::new().fg(Color::Yellow).bold()),
        ]),
        None => {
            let (icon, color) = match app.tone {
                Tone::Info => ("ℹ", Color::Gray),
                Tone::Ok => ("✓", Color::Green),
                Tone::Warn => ("⚠", Color::Yellow),
                Tone::Err => ("✗", Color::Red),
            };
            Line::from(vec![
                Span::styled(format!(" {icon} "), Style::new().fg(color).bold()),
                Span::styled(app.status.clone(), Style::new().fg(color)),
            ])
        }
    };
    f.render_widget(Paragraph::new(line), area);
}

fn keys_bar(f: &mut Frame, area: Rect, app: &App) {
    let yes = tr!("S", "Y");
    let keys: Vec<(&str, &str)> = match (&app.modal, app.tab) {
        (Modal::Menu(_), _) => vec![
            ("↑↓", tr!("scegli", "choose")),
            (enter(), tr!("esegui", "run")),
            (tr!("lettera", "letter"), tr!("scorciatoia", "shortcut")),
            ("Esc", tr!("chiudi", "close")),
        ],
        (Modal::Confirm { .. }, _) => vec![
            (yes, tr!("sì", "yes")),
            ("N", "no"),
            ("←→", tr!("cambia pulsante", "switch button")),
            (enter(), tr!("conferma", "confirm")),
            ("Esc", tr!("annulla", "cancel")),
        ],
        (Modal::Input { .. }, _) => vec![
            (enter(), tr!("analizza", "analyze")),
            ("Esc", tr!("annulla", "cancel")),
        ],
        (Modal::Help, _) => vec![(tr!("un tasto qualsiasi", "any key"), tr!("chiudi", "close"))],
        _ if app.filter_edit => vec![
            (tr!("scrivi", "type"), tr!("filtra i nomi", "filter names")),
            (enter(), tr!("conferma", "confirm")),
            ("Esc", tr!("togli il filtro", "remove the filter")),
        ],
        (_, Tab::Esplora) if app.scanning.is_some() => {
            vec![
                ("Esc", tr!("interrompi", "stop")),
                ("Tab", tr!("altra scheda", "other tab")),
                ("Q", tr!("esci", "quit")),
            ]
        }
        (_, Tab::Esplora) if app.tree.is_some() => vec![
            (enter(), tr!("apri", "open")),
            ("←", tr!("indietro", "back")),
            (space(), tr!("azioni", "actions")),
            (key_del(), trash_name()),
            (key_shift_del(), tr!("elimina", "delete")),
            ("/", tr!("cerca", "search")),
            ("S", tr!("ordina", "sort")),
            ("F5", tr!("aggiorna", "refresh")),
            ("N", tr!("nuova analisi", "new analysis")),
            ("?", tr!("aiuto", "help")),
        ],
        (_, Tab::Esplora) => vec![
            ("↑↓", tr!("scegli", "choose")),
            (enter(), tr!("analizza", "analyze")),
            ("G", tr!("scrivi percorso", "type a path")),
            ("Tab", tr!("schede", "tabs")),
            ("?", tr!("aiuto", "help")),
            ("Q", tr!("esci", "quit")),
        ],
        (_, Tab::FileGrandi) => vec![
            (enter(), tr!("azioni", "actions")),
            ("T", tr!("mostra nella cartella", "show in folder")),
            (key_del(), trash_name()),
            (key_shift_del(), tr!("elimina", "delete")),
            ("O", file_manager()),
            ("?", tr!("aiuto", "help")),
        ],
        (_, Tab::Pulizia) => vec![
            (space(), tr!("seleziona", "select")),
            ("A", tr!("tutte le sicure", "all safe")),
            ("X", tr!("nessuna", "none")),
            (enter(), tr!("pulisci", "clean")),
            ("M", tr!("azioni", "actions")),
            ("N", tr!("analizza", "analyze")),
            ("F5", tr!("ricalcola", "re-measure")),
            ("?", tr!("aiuto", "help")),
        ],
    };
    let mut spans = vec![Span::raw(" ")];
    let mut width = 1;
    for (k, d) in keys {
        let c = chip(k);
        let label = Span::styled(format!(" {d}  "), Style::new().fg(Color::Gray));
        let w = c.width() + label.width();
        if width + w > area.width as usize {
            break;
        }
        width += w;
        spans.push(c);
        spans.push(label);
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ---------------------------------------------------------------- finestre

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(4));
    let h = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn modal(f: &mut Frame, app: &mut App) {
    let area = f.area();
    match &app.modal {
        Modal::None => {}
        Modal::Help => help(f, area),
        Modal::Menu(m) => {
            let r = centered(area, 66, m.items.len() as u16 + 5);
            f.render_widget(Clear, r);
            let block = focused_panel(Line::from(format!(" {} ", m.title)).bold()).title_bottom(
                Line::from(format!(" {} ", fmt_size(m.size)))
                    .right_aligned()
                    .style(DIM),
            );
            let inner = block.inner(r);
            f.render_widget(block, r);
            let mut lines = vec![
                Line::from(Span::styled(
                    tr!(" Cosa vuoi fare?", " What do you want to do?"),
                    DIM,
                )),
                Line::from(""),
            ];
            for (i, it) in m.items.iter().enumerate() {
                let key = match it.key {
                    ' ' => space().to_string(),
                    '\n' => enter().to_string(),
                    c => c.to_ascii_uppercase().to_string(),
                };
                let base = if it.disabled.is_some() {
                    DIM
                } else if it.danger {
                    Style::new().fg(Color::LightRed)
                } else {
                    Style::new()
                };
                let mut spans = vec![
                    Span::styled(
                        if i == m.sel { " ▶ " } else { "   " },
                        Style::new().fg(ACCENT),
                    ),
                    Span::styled(
                        format!("{:<9}", format!("[{key}]")),
                        Style::new().fg(ACCENT).bold(),
                    ),
                    Span::styled(it.label.clone(), base),
                ];
                if let Some(why) = &it.disabled {
                    spans.push(Span::styled(format!("  ({why})"), DIM));
                }
                let mut line = Line::from(spans);
                if i == m.sel {
                    line = line.style(HIGHLIGHT);
                }
                let y = inner.y + 2 + i as u16;
                if y < inner.y + inner.height {
                    app.hits.menu.push((
                        Rect {
                            x: inner.x,
                            y,
                            width: inner.width,
                            height: 1,
                        },
                        i,
                    ));
                }
                lines.push(line);
            }
            f.render_widget(Paragraph::new(lines), inner);
        }
        Modal::Confirm {
            title,
            body,
            warn,
            danger,
            yes_label,
            yes_focus,
            ..
        } => {
            let color = if *danger { Color::Red } else { Color::Yellow };
            let mut lines: Vec<Line> = vec![Line::from("")];
            lines.extend(body.iter().map(|l| Line::from(format!(" {l}"))));
            if let Some(w) = warn {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!(" ⚠ {w}"),
                    Style::new().fg(Color::Yellow).bold(),
                )));
            }
            lines.push(Line::from(""));
            let h = lines.len() as u16 + 4;
            let r = centered(area, 92, h);
            f.render_widget(Clear, r);
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .title(Line::from(title.clone()).bold())
                .border_style(Style::new().fg(color));
            let inner = block.inner(r);
            f.render_widget(block, r);
            let [text, buttons] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), text);

            let off = Style::new().fg(Color::Gray).bg(Color::Rgb(50, 50, 58));
            let yes_style = if *yes_focus {
                Style::new()
                    .fg(Color::Black)
                    .bg(if *danger {
                        Color::LightRed
                    } else {
                        Color::Yellow
                    })
                    .bold()
            } else {
                off
            };
            let no_style = if !*yes_focus {
                Style::new().fg(Color::Black).bg(Color::Gray).bold()
            } else {
                off
            };
            let yes = Span::styled(format!("  {} · {yes_label}  ", tr!("S", "Y")), yes_style);
            let no = Span::styled(format!("  N · {}  ", tr!("Annulla", "Cancel")), no_style);
            let (yw, nw) = (yes.width() as u16, no.width() as u16);
            let x0 = buttons.x + 1;
            app.hits.buttons.push((
                Rect {
                    x: x0,
                    y: buttons.y,
                    width: yw,
                    height: 1,
                },
                true,
            ));
            app.hits.buttons.push((
                Rect {
                    x: x0 + yw + 3,
                    y: buttons.y,
                    width: nw,
                    height: 1,
                },
                false,
            ));
            f.render_widget(
                Paragraph::new(Line::from(vec![Span::raw(" "), yes, Span::raw("   "), no])),
                buttons,
            );
        }
        Modal::Input { buf } => {
            let r = centered(area, 92, 6);
            f.render_widget(Clear, r);
            let example = if cfg!(windows) {
                "C:\\Users\\…\\Documents"
            } else {
                "~/Documents"
            };
            let lines = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(buf.clone(), Style::new().bold()),
                    Span::styled("▌", Style::new().fg(ACCENT)),
                ]),
                Line::from(Span::styled(
                    trf!(
                        " Scrivi o incolla un percorso (es. {example}) · Invio = analizza · Esc = annulla",
                        " Type or paste a path (e.g. {example}) · Enter = analyze · Esc = cancel"
                    ),
                    DIM,
                )),
            ];
            f.render_widget(
                Paragraph::new(lines).block(focused_panel(
                    Line::from(tr!(
                        " Quale cartella vuoi analizzare? ",
                        " Which folder do you want to analyze? "
                    ))
                    .bold(),
                )),
                r,
            );
        }
    }
}

fn help(f: &mut Frame, area: Rect) {
    let section = |t: &str| {
        Line::from(Span::styled(
            format!(" {t}"),
            Style::new().bold().fg(ACCENT),
        ))
    };
    let row = |k: &str, d: String| {
        Line::from(vec![
            Span::raw("   "),
            chip(k),
            Span::styled(format!(" {d}"), Style::new()),
        ])
    };
    let note = |t: String| Line::from(Span::styled(format!("   {t}"), DIM));
    let lines = vec![
        Line::from(""),
        section(tr!("Ovunque", "Everywhere")),
        row("Tab", tr!("cambia scheda (anche 1, 2, 3 o clic sulle schede)", "switch tab (also 1, 2, 3 or click the tabs)").into()),
        row(
            tr!("↑ ↓ PagSu PagGiù", "↑ ↓ PgUp PgDn"),
            tr!("muoviti nelle liste (anche con la rotella)", "move in the lists (the mouse wheel works too)").into(),
        ),
        row("G", tr!("scrivi o incolla un percorso da analizzare", "type or paste a path to analyze").into()),
        row("Q", tr!("esci", "quit").into()),
        Line::from(""),
        section(tr!("1 · Esplora", "1 · Explore")),
        row(tr!("Invio  →", "Enter  →"), tr!("apri la cartella (o doppio clic)", "open the folder (or double-click)").into()),
        row(
            "←  Backspace",
            tr!("torna alla cartella superiore (o clic sul percorso in alto)", "go to the parent folder (or click the path at the top)").into(),
        ),
        row(space(), tr!("menu con tutte le azioni (o tasto destro)", "menu with all actions (or right-click)").into()),
        row(
            key_del(),
            trf!(
                "sposta nel {}     {}  elimina definitivamente",
                "move to the {}     {}  delete permanently",
                trash_name(),
                key_shift_del()
            ),
        ),
        row("/", tr!("cerca per nome          S  ordina per dimensione/nome", "search by name          S  sort by size/name").into()),
        row("F5", tr!("rifai l'analisi         N  analizza un'altra cartella", "analyze again           N  analyze another folder").into()),
        Line::from(""),
        section(tr!("3 · Pulizia", "3 · Cleanup")),
        row(
            space(),
            tr!("seleziona (o clic sulla casella)     A  tutte le voci sicure", "select (or click the checkbox)     A  all safe entries").into(),
        ),
        row(enter(), tr!("pulisci le voci selezionate (o quella evidenziata)", "clean the selected entries (or the highlighted one)").into()),
        Line::from(""),
        section(tr!("Sicurezza", "Safety")),
        note(
            tr!(
                "Le cartelle di sistema e quelle principali del profilo non si possono eliminare.",
                "System folders and the main profile folders cannot be deleted."
            )
            .into(),
        ),
        note(
            tr!(
                "Il pannello Dettagli dice se una voce è rigenerabile, protetta o da valutare.",
                "The Details panel tells you whether an entry is regenerable, protected or up to you."
            )
            .into(),
        ),
        note(trf!(
            "Il {} non libera spazio finché non lo svuoti (voce in Pulizia).",
            "The {} does not free space until you empty it (entry in Cleanup).",
            trash_name()
        )),
        note(
            tr!(
                "Per pulire le cartelle di sistema avvia il programma come amministratore (o con sudo).",
                "To clean system folders run the program as administrator (or with sudo)."
            )
            .into(),
        ),
        Line::from(""),
    ];
    let r = centered(area, 98, lines.len() as u16 + 2);
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(lines).block(focused_panel(
            Line::from(tr!(
                " Aiuto · premi un tasto per chiudere ",
                " Help · press any key to close "
            ))
            .bold(),
        )),
        r,
    );
}
