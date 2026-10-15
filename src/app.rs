//! Stato dell'applicazione, tasti, mouse e operazioni in background.

use crate::assess::{self, Level, Verdict};
use crate::cleanup::{self, Action, Safety, Target};
use crate::i18n::{file_manager, trash_name};
use crate::scan::{self, Kind, Node, Progress};
use crate::ui;
use crate::util::{fmt_count, fmt_size, plural};
use crate::{tr, trf};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Esplora,
    FileGrandi,
    Pulizia,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Esplora, Tab::FileGrandi, Tab::Pulizia];
    pub fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Size,
    Name,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Info,
    Ok,
    Warn,
    Err,
}

pub struct Drive {
    pub mount: PathBuf,
    pub total: u64,
    pub free: u64,
    pub fs: String,
}

pub struct Place {
    pub label: String,
    pub hint: String,
    pub path: PathBuf,
    /// Spazio usato/totale, solo per i dischi.
    pub usage: Option<(u64, u64)>,
}

/// Gruppi della scheda Pulizia, nell'ordine in cui compaiono.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Sicuro,
    Cache,
    Progetti,
    Attenzione,
    Manuale,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::Sicuro,
        Group::Cache,
        Group::Progetti,
        Group::Attenzione,
        Group::Manuale,
    ];
    pub fn of(t: &Target) -> Group {
        if t.from_scan {
            return Group::Progetti;
        }
        match t.safety {
            Safety::Sicuro => Group::Sicuro,
            Safety::Rigenerabile => Group::Cache,
            Safety::Attenzione => Group::Attenzione,
            Safety::Manuale => Group::Manuale,
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Group::Sicuro => tr!(
                "SICURO — si cancella senza conseguenze",
                "SAFE — can be deleted without consequences"
            ),
            Group::Cache => tr!(
                "CACHE — si riscaricano da sole quando servono",
                "CACHES — re-downloaded automatically when needed"
            ),
            Group::Progetti => tr!(
                "FILE DI COMPILAZIONE DEI PROGETTI — si ricreano con la build",
                "PROJECT BUILD FILES — recreated by the build"
            ),
            Group::Attenzione => tr!(
                "CON ATTENZIONE — si perde qualcosa di scaricato",
                "WITH CAUTION — you lose something downloaded"
            ),
            Group::Manuale => tr!(
                "DA GESTIRE CON LA LORO APP — qui solo per sapere quanto pesano",
                "MANAGE WITH THEIR APP — listed here only to show their size"
            ),
        }
    }
    pub fn safety(self) -> Safety {
        match self {
            Group::Sicuro => Safety::Sicuro,
            Group::Cache | Group::Progetti => Safety::Rigenerabile,
            Group::Attenzione => Safety::Attenzione,
            Group::Manuale => Safety::Manuale,
        }
    }
}

pub enum Row {
    Header(Group, u64),
    Item(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    Open,
    ScanHere,
    ShowInFileManager,
    CopyPath,
    ShowInTree,
    Trash,
    Delete,
    CleanOne,
    Toggle,
}

pub struct MenuItem {
    /// Tasto rapido; `'\n'` = Invio, `' '` = Spazio.
    pub key: char,
    pub label: String,
    pub cmd: Cmd,
    pub danger: bool,
    /// Motivo per cui la voce non è disponibile.
    pub disabled: Option<String>,
}

pub struct Menu {
    pub title: String,
    pub path: PathBuf,
    pub size: u64,
    pub files: u64,
    pub is_dir: bool,
    pub target_id: Option<usize>,
    pub items: Vec<MenuItem>,
    pub sel: usize,
}

pub enum Pending {
    Delete {
        full: PathBuf,
        size: u64,
        permanent: bool,
    },
    Clean {
        ids: Vec<usize>,
    },
    Quit,
}

pub enum Modal {
    None,
    Help,
    Menu(Menu),
    Confirm {
        title: String,
        body: Vec<String>,
        warn: Option<String>,
        danger: bool,
        yes_label: String,
        yes_focus: bool,
        action: Pending,
    },
    Input {
        buf: String,
    },
}

pub enum Msg {
    ScanDone {
        root: PathBuf,
        tree: Node,
        elapsed: Duration,
        errors: u64,
        cancelled: bool,
    },
    TargetSize {
        id: usize,
        size: u64,
        partial: bool,
    },
    OpDone {
        label: String,
        freed: u64,
        errors: u64,
        first_err: Option<String>,
        kept_repos: u64,
        gone: Vec<PathBuf>,
        remeasure: Vec<usize>,
        trashed: bool,
    },
}

pub struct Scanning {
    pub root: PathBuf,
    pub prog: Arc<Progress>,
    pub started: Instant,
    /// Spazio occupato del disco, se si analizza un disco intero: serve per la stima.
    pub drive_used: Option<u64>,
}

/// Zone cliccabili, aggiornate a ogni disegno.
#[derive(Default)]
pub struct Hits {
    pub tabs: Vec<(Rect, Tab)>,
    /// Area interna della lista principale e primo elemento visibile.
    pub list: Option<(Rect, usize)>,
    /// Pezzi del percorso: quante cartelle tenere aprendo quel pezzo.
    pub crumbs: Vec<(Rect, usize)>,
    pub menu: Vec<(Rect, usize)>,
    pub buttons: Vec<(Rect, bool)>,
}

pub struct App {
    pub tab: Tab,
    pub modal: Modal,
    pub quit: bool,
    pub tick: u64,
    pub drives: Vec<Drive>,
    pub places: Vec<Place>,
    pub home_state: ListState,
    pub scanning: Option<Scanning>,
    pub tree: Option<Node>,
    tree_version: u64,
    pub root_path: PathBuf,
    pub scan_info: String,
    pub nav: Vec<usize>,
    pub explore_state: ListState,
    pub sort: Sort,
    pub filter: String,
    pub filter_edit: bool,
    pub big_files: Vec<(PathBuf, u64)>,
    pub big_state: ListState,
    pub targets: Vec<Target>,
    pub tgt_state: ListState,
    next_id: usize,
    pub busy: Option<String>,
    pub status: String,
    pub tone: Tone,
    pub hits: Hits,
    last_click: Option<(Instant, u16, u16)>,
    tags: Option<(Vec<usize>, u64, Vec<Level>)>,
    verdict: Option<(PathBuf, Verdict)>,
    restore: Option<Vec<String>>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

/// File system virtuali o di servizio che non vanno mostrati come dischi.
const PSEUDO_FS: &[&str] = &[
    "tmpfs",
    "devtmpfs",
    "squashfs",
    "overlay",
    "proc",
    "sysfs",
    "efivarfs",
    "devfs",
    "autofs",
    "ramfs",
    "cgroup",
    "cgroup2",
    "fuse.snapfuse",
    "nsfs",
    "tracefs",
    "debugfs",
    "securityfs",
    "pstore",
    "bpf",
    "configfs",
    "mqueue",
    "hugetlbfs",
    "fusectl",
];

pub fn read_drives() -> Vec<Drive> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut v: Vec<Drive> = disks
        .list()
        .iter()
        .filter(|d| d.total_space() > 0)
        .map(|d| Drive {
            mount: d.mount_point().to_path_buf(),
            total: d.total_space(),
            free: d.available_space(),
            fs: d.file_system().to_string_lossy().into_owned(),
        })
        .filter(|d| {
            let m = d.mount.to_string_lossy();
            !PSEUDO_FS.contains(&d.fs.to_ascii_lowercase().as_str())
                && ![
                    "/snap/",
                    "/boot/efi",
                    "/run/",
                    "/dev/",
                    "/sys/",
                    "/proc/",
                    "/System/Volumes/",
                    "/private/var/vm",
                ]
                .iter()
                .any(|pre| m.starts_with(pre))
        })
        .collect();
    v.sort_by(|a, b| a.mount.cmp(&b.mount));
    v.dedup_by(|a, b| a.mount == b.mount);
    v
}

fn build_places(drives: &[Drive]) -> Vec<Place> {
    let mut v: Vec<Place> = drives
        .iter()
        .map(|d| Place {
            label: trf!("Disco {}", "Drive {}", d.mount.display()),
            hint: trf!(
                "liberi {} su {}",
                "{} free of {}",
                fmt_size(d.free),
                fmt_size(d.total)
            ),
            path: d.mount.clone(),
            usage: Some((d.total - d.free, d.total)),
        })
        .collect();
    if let Some(h) = cleanup::home_dir() {
        let mut quick: Vec<(&str, &str)> = vec![
            (tr!("Profilo utente", "Home folder"), ""),
            (tr!("Desktop", "Desktop"), "Desktop"),
            (tr!("Documenti", "Documents"), "Documents"),
            (tr!("Download", "Downloads"), "Downloads"),
        ];
        if cfg!(windows) {
            quick.push((
                tr!("Dati delle app (AppData)", "App data (AppData)"),
                "AppData",
            ));
        } else if cfg!(target_os = "macos") {
            quick.push((
                tr!("Dati delle app (Library)", "App data (Library)"),
                "Library",
            ));
        } else {
            quick.push((
                tr!("Cache delle app (~/.cache)", "App caches (~/.cache)"),
                ".cache",
            ));
        }
        for (label, rel) in quick {
            let p = if rel.is_empty() {
                h.clone()
            } else {
                h.join(rel)
            };
            if p.is_dir() {
                v.push(Place {
                    label: label.into(),
                    hint: p.display().to_string(),
                    path: p,
                    usage: None,
                });
            }
        }
    }
    v
}

fn move_sel(state: &mut ListState, len: usize, delta: isize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let cur = state.selected().unwrap_or(0) as isize;
    state.select(Some(
        (cur.saturating_add(delta)).clamp(0, len as isize - 1) as usize
    ));
}

/// Spostamento richiesto da un tasto di movimento (frecce, pagine, inizio/fine).
fn nav_delta(code: KeyCode) -> Option<isize> {
    Some(match code {
        KeyCode::Up => -1,
        KeyCode::Down => 1,
        KeyCode::PageUp => -15,
        KeyCode::PageDown => 15,
        KeyCode::Home => isize::MIN / 2,
        KeyCode::End => isize::MAX / 2,
        _ => return None,
    })
}

/// Canc/Del = cestino, Maiusc+Canc = definitivo; `d`/`D` come alternative.
fn is_delete(k: &KeyEvent) -> Option<bool> {
    match k.code {
        KeyCode::Delete => Some(k.modifiers.contains(KeyModifiers::SHIFT)),
        KeyCode::Char('d') => Some(false),
        KeyCode::Char('D') => Some(true),
        _ => None,
    }
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let targets = cleanup::fixed_targets();
        let next_id = targets.len();
        let drives = read_drives();
        let places = build_places(&drives);
        let mut app = App {
            tab: Tab::Esplora,
            modal: Modal::None,
            quit: false,
            tick: 0,
            drives,
            places,
            home_state: ListState::default().with_selected(Some(0)),
            scanning: None,
            tree: None,
            tree_version: 0,
            root_path: PathBuf::new(),
            scan_info: String::new(),
            nav: Vec::new(),
            explore_state: ListState::default().with_selected(Some(0)),
            sort: Sort::Size,
            filter: String::new(),
            filter_edit: false,
            big_files: Vec::new(),
            big_state: ListState::default().with_selected(Some(0)),
            targets,
            tgt_state: ListState::default().with_selected(Some(1)),
            next_id,
            busy: None,
            status: tr!(
                "Benvenuto! Scegli un disco o una cartella e premi Invio (o fai doppio clic) per analizzarla.",
                "Welcome! Pick a drive or a folder and press Enter (or double-click) to analyze it."
            )
            .into(),
            tone: Tone::Info,
            hits: Hits::default(),
            last_click: None,
            tags: None,
            verdict: None,
            restore: None,
            tx,
            rx,
        };
        let ids = app.targets.iter().map(|t| t.id).collect();
        app.measure_targets(ids);
        app
    }

    pub fn run(mut self, term: &mut DefaultTerminal) -> io::Result<()> {
        while !self.quit {
            while let Ok(m) = self.rx.try_recv() {
                self.on_msg(m);
            }
            term.draw(|f| ui::draw(f, &mut self))?;
            if event::poll(Duration::from_millis(120))? {
                match event::read()? {
                    // Su Windows arrivano anche gli eventi di rilascio: si tengono solo le pressioni.
                    Event::Key(k) if k.kind != KeyEventKind::Release => self.on_key(k),
                    Event::Mouse(m) => self.on_mouse(m),
                    _ => {}
                }
            }
            self.tick = self.tick.wrapping_add(1);
        }
        if let Some(s) = &self.scanning {
            s.prog.cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn say(&mut self, tone: Tone, text: impl Into<String>) {
        self.tone = tone;
        self.status = text.into();
    }

    fn busy_warning(&mut self) {
        self.say(
            Tone::Warn,
            tr!(
                "Attendi la fine dell'operazione in corso.",
                "Wait for the current operation to finish."
            ),
        );
    }

    // ------------------------------------------------------------ vista corrente

    pub fn view(&self) -> Option<&Node> {
        self.tree.as_ref().map(|t| scan::node_at(t, &self.nav))
    }

    pub fn view_path(&self) -> PathBuf {
        self.path_at(self.nav.len())
    }

    /// Percorso della cartella che si ottiene tenendo le prime `depth` cartelle aperte.
    pub fn path_at(&self, depth: usize) -> PathBuf {
        let mut p = self.root_path.clone();
        if let Some(t) = &self.tree {
            let mut n = t;
            for &i in self.nav.iter().take(depth) {
                n = &n.children[i];
                p.push(&n.name);
            }
        }
        p
    }

    fn current_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(t) = &self.tree {
            let mut n = t;
            for &i in &self.nav {
                n = &n.children[i];
                out.push(n.name.clone());
            }
        }
        out
    }

    /// Indici (nei figli della cartella corrente) delle voci mostrate, con filtro e ordinamento.
    pub fn visible(&self) -> Vec<usize> {
        let Some(n) = self.view() else {
            return Vec::new();
        };
        let f = self.filter.to_lowercase();
        let mut v: Vec<usize> = (0..n.children.len())
            .filter(|&i| f.is_empty() || n.children[i].name.to_lowercase().contains(&f))
            .collect();
        if self.sort == Sort::Name {
            v.sort_by_cached_key(|&i| {
                let c = &n.children[i];
                (c.kind != Kind::Dir, c.name.to_lowercase())
            });
        }
        v
    }

    fn selected_index(&self) -> Option<usize> {
        self.visible().get(self.explore_state.selected()?).copied()
    }

    pub fn selected_child(&self) -> Option<&Node> {
        self.view()?.children.get(self.selected_index()?)
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        let c = self.selected_child()?;
        Some(if c.kind == Kind::Group {
            self.view_path()
        } else {
            self.view_path().join(&c.name)
        })
    }

    fn select_child(&mut self, child: usize) {
        let pos = self.visible().iter().position(|&i| i == child).unwrap_or(0);
        self.explore_state.select(Some(pos));
    }

    fn set_nav(&mut self, nav: Vec<usize>, select_child: Option<usize>) {
        self.nav = nav;
        self.filter.clear();
        self.filter_edit = false;
        match select_child {
            Some(c) => self.select_child(c),
            None => self.explore_state.select(Some(0)),
        }
    }

    /// Porta la vista sulla cartella `names` (o sulla più vicina che esiste ancora)
    /// e seleziona `select` se c'è, altrimenti la posizione `fallback`.
    fn goto_names(&mut self, names: &[String], select: Option<&str>, fallback: usize) {
        let Some(tree) = &self.tree else { return };
        let nav = scan::resolve_names(tree, names);
        let view = scan::node_at(tree, &nav);
        let child = select.and_then(|s| {
            view.children
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(s))
        });
        let len = view.children.len();
        self.nav = nav;
        match child {
            Some(c) => self.select_child(c),
            None => self.explore_state.select(if len == 0 {
                None
            } else {
                Some(fallback.min(len - 1))
            }),
        }
    }

    /// Etichette («rigenerabile», «sistema»…) delle voci della cartella corrente, in cache.
    pub fn view_tags(&mut self) -> &[Level] {
        let fresh = matches!(&self.tags, Some((nav, ver, _)) if *nav == self.nav && *ver == self.tree_version);
        if !fresh {
            let base = self.view_path();
            let tags = match self.view() {
                Some(n) => n
                    .children
                    .iter()
                    .map(|c| match c.kind {
                        Kind::Group => Level::Ignoto,
                        _ => assess::quick(&base.join(&c.name), c.kind == Kind::Dir, &self.targets),
                    })
                    .collect(),
                None => Vec::new(),
            };
            self.tags = Some((self.nav.clone(), self.tree_version, tags));
        }
        self.tags.as_ref().map(|t| t.2.as_slice()).unwrap_or(&[])
    }

    /// Valutazione completa di un percorso, in cache sull'ultimo chiesto.
    pub fn verdict(&mut self, path: &Path, is_dir: bool) -> Verdict {
        if let Some((p, v)) = &self.verdict
            && p == path
        {
            return v.clone();
        }
        let v = assess::full(path, is_dir, &self.targets);
        self.verdict = Some((path.to_path_buf(), v.clone()));
        v
    }

    // ------------------------------------------------------------ Pulizia: righe con i gruppi

    pub fn clean_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        for g in Group::ALL {
            let items: Vec<usize> = (0..self.targets.len())
                .filter(|&i| Group::of(&self.targets[i]) == g)
                .collect();
            if items.is_empty() {
                continue;
            }
            let total = items.iter().filter_map(|&i| self.targets[i].size).sum();
            rows.push(Row::Header(g, total));
            rows.extend(items.into_iter().map(Row::Item));
        }
        rows
    }

    pub fn selected_target(&self) -> Option<usize> {
        match self.clean_rows().get(self.tgt_state.selected()?)? {
            Row::Item(i) => Some(*i),
            Row::Header(..) => None,
        }
    }

    /// Sposta la selezione nella Pulizia saltando le intestazioni dei gruppi.
    fn move_clean(&mut self, delta: isize) {
        let rows = self.clean_rows();
        if rows.is_empty() {
            self.tgt_state.select(None);
            return;
        }
        let last = rows.len() as isize - 1;
        let start = (self.tgt_state.selected().unwrap_or(0) as isize)
            .saturating_add(delta)
            .clamp(0, last);
        let step = if delta < 0 { -1 } else { 1 };
        let find = |step: isize| {
            let mut j = start;
            while (0..=last).contains(&j) {
                if matches!(rows[j as usize], Row::Item(_)) {
                    return Some(j as usize);
                }
                j += step;
            }
            None
        };
        // Salta le intestazioni nella direzione del movimento; in cima o in fondo torna indietro.
        self.tgt_state.select(find(step).or_else(|| find(-step)));
    }

    // ------------------------------------------------------------ operazioni in background

    fn measure_targets(&mut self, ids: Vec<usize>) {
        let jobs: Vec<(usize, PathBuf)> = self
            .targets
            .iter_mut()
            .filter(|t| ids.contains(&t.id))
            .map(|t| {
                t.size = None;
                (t.id, t.path.clone())
            })
            .collect();
        let tx = self.tx.clone();
        thread::spawn(move || {
            for (id, path) in jobs {
                let (size, partial) = cleanup::dir_size(&path);
                if tx.send(Msg::TargetSize { id, size, partial }).is_err() {
                    return;
                }
            }
        });
    }

    pub fn start_scan(&mut self, root: PathBuf) {
        if self.scanning.is_some() {
            return self.say(
                Tone::Warn,
                tr!(
                    "C'è già un'analisi in corso: Esc per interromperla.",
                    "An analysis is already running: Esc to stop it."
                ),
            );
        }
        if self.busy.is_some() {
            return self.busy_warning();
        }
        self.restore = if self.tree.is_some() && root == self.root_path {
            Some(self.current_names())
        } else {
            None
        };
        // Liberare un albero grande può richiedere un attimo: lo si fa fuori dall'interfaccia.
        if let Some(old) = self.tree.take() {
            thread::spawn(move || drop(old));
        }
        self.big_files.clear();
        self.targets.retain(|t| !t.from_scan);
        self.nav.clear();
        self.filter.clear();
        self.tab = Tab::Esplora;
        let prog = Arc::new(Progress::default());
        let drive_used = self
            .drives
            .iter()
            .find(|d| d.mount == root)
            .map(|d| d.total - d.free);
        let (tx, p, r) = (self.tx.clone(), prog.clone(), root.clone());
        let spawned = thread::Builder::new().stack_size(64 << 20).spawn(move || {
            let t = Instant::now();
            let tree = scan::scan(&r, &p);
            let _ = tx.send(Msg::ScanDone {
                root: r,
                tree,
                elapsed: t.elapsed(),
                errors: p.errors.load(Ordering::Relaxed),
                cancelled: p.cancel.load(Ordering::Relaxed),
            });
        });
        if spawned.is_err() {
            return self.say(
                Tone::Err,
                tr!(
                    "Impossibile avviare l'analisi.",
                    "Could not start the analysis."
                ),
            );
        }
        self.say(
            Tone::Info,
            trf!("Analisi di {}…", "Analyzing {}…", root.display()),
        );
        self.scanning = Some(Scanning {
            root,
            prog,
            started: Instant::now(),
            drive_used,
        });
    }

    fn on_msg(&mut self, m: Msg) {
        match m {
            Msg::ScanDone {
                root,
                tree,
                elapsed,
                errors,
                cancelled,
            } => {
                self.scanning = None;
                self.big_files = scan::top_files(&tree, &root, 500);
                for a in scan::dev_artifacts(&tree, &root) {
                    let kind = a
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    // Nome leggibile: il progetto relativo alla cartella analizzata, es. «mio-sito/apps/web › .next».
                    let project = a
                        .path
                        .parent()
                        .map(|p| match p.strip_prefix(&root) {
                            Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
                            _ => p
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        })
                        .unwrap_or_default();
                    let parent = a
                        .path
                        .parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    self.targets.push(Target {
                        id: self.next_id,
                        name: format!("{project} › {kind}"),
                        detail: trf!(
                            "{} — progetto in {parent}",
                            "{} — project in {parent}",
                            a.what
                        ),
                        path: a.path,
                        safety: Safety::Rigenerabile,
                        action: Action::DeleteDir,
                        size: Some(a.size),
                        partial: false,
                        selected: false,
                        from_scan: true,
                    });
                    self.next_id += 1;
                }
                let unreadable = if errors > 0 {
                    trf!(
                        " · {} cartelle non leggibili",
                        " · {} unreadable folders",
                        fmt_count(errors)
                    )
                } else {
                    String::new()
                };
                self.scan_info = trf!(
                    "{} in {} file · {:.1} s{}",
                    "{} in {} files · {:.1} s{}",
                    fmt_size(tree.size),
                    fmt_count(tree.files),
                    elapsed.as_secs_f64(),
                    unreadable
                );
                if cancelled {
                    self.say(
                        Tone::Warn,
                        tr!(
                            "Analisi interrotta: i dati sono parziali (F5 per rifarla).",
                            "Analysis stopped: data is partial (F5 to run it again)."
                        ),
                    );
                } else {
                    let artifacts: u64 = self
                        .targets
                        .iter()
                        .filter(|t| t.from_scan)
                        .filter_map(|t| t.size)
                        .sum();
                    let extra = if artifacts > 0 {
                        trf!(
                            " · {} di file di compilazione nella scheda 3 Pulizia",
                            " · {} of build files in tab 3 Cleanup",
                            fmt_size(artifacts)
                        )
                    } else {
                        String::new()
                    };
                    self.say(
                        Tone::Ok,
                        trf!(
                            "Analisi completata: {}{extra}",
                            "Analysis complete: {}{extra}",
                            self.scan_info
                        ),
                    );
                }
                self.root_path = root;
                self.tree = Some(tree);
                self.tree_version += 1;
                self.set_nav(Vec::new(), None);
                self.big_state.select(Some(0));
                if let Some(names) = self.restore.take() {
                    self.goto_names(&names, None, 0);
                }
                self.drives = read_drives();
            }
            Msg::TargetSize { id, size, partial } => {
                if let Some(t) = self.targets.iter_mut().find(|t| t.id == id) {
                    t.size = Some(size);
                    t.partial = partial;
                }
            }
            Msg::OpDone {
                label,
                freed,
                errors,
                first_err,
                kept_repos,
                gone,
                remeasure,
                trashed,
            } => {
                self.busy = None;
                for p in &gone {
                    self.prune(p);
                }
                if !remeasure.is_empty() {
                    self.measure_targets(remeasure);
                }
                self.drives = read_drives();
                let mut text = if trashed && errors == 0 {
                    trf!(
                        "«{label}» spostato nel {} ({}). Lo spazio si libera svuotando il {} (scheda 3).",
                        "“{label}” moved to the {} ({}). The space is freed when you empty the {} (tab 3).",
                        trash_name(),
                        fmt_size(freed),
                        trash_name()
                    )
                } else if errors == 0 {
                    trf!(
                        "{label}: liberati {}.",
                        "{label}: freed {}.",
                        fmt_size(freed)
                    )
                } else {
                    trf!(
                        "{label}: liberati {} · {} elementi non eliminati (in uso o servono permessi){}",
                        "{label}: freed {} · {} items not deleted (in use or permission needed){}",
                        fmt_size(freed),
                        fmt_count(errors),
                        first_err.map(|e| format!(" — {e}")).unwrap_or_default()
                    )
                };
                if kept_repos > 0 {
                    text.push_str(&trf!(
                        " · {kept_repos} repository git lasciati intatti",
                        " · {kept_repos} git repositories left untouched"
                    ));
                }
                self.say(if errors == 0 { Tone::Ok } else { Tone::Warn }, text);
            }
        }
    }

    /// Toglie dall'albero e dalle liste un percorso che non esiste più.
    fn prune(&mut self, p: &Path) {
        let names = self.current_names();
        let sel_name = self.selected_child().map(|c| c.name.clone());
        let sel_pos = self.explore_state.selected().unwrap_or(0);
        if let Some(rel) = scan::rel_names(&self.root_path, p)
            && let Some(tree) = self.tree.as_mut()
        {
            let path = scan::resolve_names(tree, &rel);
            if !path.is_empty() && path.len() == rel.len() {
                scan::remove_at(tree, &path);
                self.tree_version += 1;
            }
        }
        self.big_files.retain(|(f, _)| !f.starts_with(p));
        self.targets
            .retain(|t| !(t.from_scan && t.path.starts_with(p)));
        self.verdict = None;
        self.goto_names(&names, sel_name.as_deref(), sel_pos);
        let len = self.visible().len();
        move_sel(&mut self.explore_state, len, 0);
        move_sel(&mut self.big_state, self.big_files.len(), 0);
        self.move_clean(0);
    }

    fn ask_delete(&mut self, full: PathBuf, size: u64, files: u64, is_dir: bool, permanent: bool) {
        if self.busy.is_some() {
            return self.busy_warning();
        }
        if let Some(why) = cleanup::protection(&full) {
            return self.say(
                Tone::Err,
                trf!(
                    "Non si elimina da qui: {why}.",
                    "It cannot be deleted from here: {why}."
                ),
            );
        }
        let name = full
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let what = if is_dir {
            trf!(
                "Cartella con {} file",
                "Folder with {} files",
                fmt_count(files)
            )
        } else {
            tr!("File", "File").into()
        };
        let body = vec![
            format!("{what}: {name}"),
            trf!("Dimensione: {}", "Size: {}", fmt_size(size)),
            trf!("Percorso: {}", "Path: {}", full.display()),
            String::new(),
            if permanent {
                tr!(
                    "Verrà ELIMINATO DEFINITIVAMENTE: non si potrà recuperare.",
                    "It will be DELETED PERMANENTLY: it cannot be recovered."
                )
                .into()
            } else {
                trf!(
                    "Verrà spostato nel {}: puoi ripristinarlo. Lo spazio si libera svuotando il {}.",
                    "It will be moved to the {}: you can restore it. The space is freed when you empty the {}.",
                    trash_name(),
                    trash_name()
                )
            },
        ];
        let rigenerabile = is_dir
            && full
                .parent()
                .is_some_and(|p| scan::artifact_kind(p, &name).is_some());
        self.modal = Modal::Confirm {
            title: if permanent {
                tr!(" Eliminare definitivamente? ", " Delete permanently? ").into()
            } else {
                trf!(" Spostare nel {}? ", " Move to the {}? ", trash_name())
            },
            body,
            warn: cleanup::warning(&full).map(String::from),
            danger: permanent,
            yes_label: if permanent {
                tr!("Elimina definitivamente", "Delete permanently").into()
            } else {
                trf!("Sposta nel {}", "Move to the {}", trash_name())
            },
            // Per le eliminazioni definitive Invio è su «Annulla», salvo i file di compilazione.
            yes_focus: !permanent || rigenerabile,
            action: Pending::Delete {
                full,
                size,
                permanent,
            },
        };
    }

    fn ask_clean(&mut self, only: Option<usize>) {
        if self.busy.is_some() {
            return self.busy_warning();
        }
        let ids: Vec<usize> = match only {
            Some(i) => vec![self.targets[i].id],
            None => {
                let sel: Vec<usize> = self
                    .targets
                    .iter()
                    .filter(|t| t.selected && t.actionable())
                    .map(|t| t.id)
                    .collect();
                if sel.is_empty() {
                    match self.selected_target() {
                        Some(i) => vec![self.targets[i].id],
                        None => Vec::new(),
                    }
                } else {
                    sel
                }
            }
        };
        let chosen: Vec<&Target> = self
            .targets
            .iter()
            .filter(|t| ids.contains(&t.id) && t.actionable())
            .collect();
        if chosen.is_empty() {
            let msg = match self.selected_target().map(|i| &self.targets[i]) {
                Some(t) if !t.actionable() => {
                    trf!(
                        "«{}» va gestita a mano: {}",
                        "“{}” must be handled by hand: {}",
                        t.name,
                        t.detail
                    )
                }
                _ => tr!(
                    "Seleziona almeno una voce con Spazio (o clic sulla casella).",
                    "Select at least one entry with Space (or click its checkbox)."
                )
                .into(),
            };
            return self.say(Tone::Warn, msg);
        }
        let total: u64 = chosen.iter().filter_map(|t| t.size).sum();
        let definitive = chosen
            .iter()
            .any(|t| matches!(t.action, Action::DeleteDir | Action::EmptyRecycleBin));
        let danger = chosen.iter().any(|t| t.safety == Safety::Attenzione);
        let mut body: Vec<String> = chosen
            .iter()
            .take(10)
            .map(|t| {
                format!(
                    "• {:<48} {:>10}",
                    t.name,
                    t.size.map(fmt_size).unwrap_or_else(|| "…".into())
                )
            })
            .collect();
        if chosen.len() > 10 {
            body.push(trf!("  … e altre {}", "  … and {} more", chosen.len() - 10));
        }
        body.push(String::new());
        body.push(trf!(
            "Spazio da liberare: fino a {}. I file in uso vengono saltati.",
            "Space to free: up to {}. Files in use are skipped.",
            fmt_size(total)
        ));
        let ids = chosen.iter().map(|t| t.id).collect();
        let n = chosen.len();
        self.modal = Modal::Confirm {
            title: if n == 1 {
                tr!(" Pulire questa voce? ", " Clean this entry? ").into()
            } else {
                trf!(
                    " Pulire {}? ",
                    " Clean {}? ",
                    plural(n, tr!("voce", "entry"), tr!("voci", "entries"))
                )
            },
            body,
            warn: definitive.then(|| {
                tr!(
                    "L'eliminazione è definitiva (non passa dal cestino).",
                    "Deletion is permanent (it does not go through the trash)."
                )
                .into()
            }),
            danger,
            yes_label: tr!("Pulisci", "Clean").into(),
            yes_focus: !danger,
            action: Pending::Clean { ids },
        };
    }

    fn execute(&mut self, action: Pending) {
        let tx = self.tx.clone();
        match action {
            Pending::Quit => self.quit = true,
            Pending::Delete {
                full,
                size,
                permanent,
            } => {
                let name = full
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.busy = Some(trf!("Elimino «{name}»…", "Deleting “{name}”…"));
                let recycle: Vec<usize> = if permanent {
                    Vec::new()
                } else {
                    // Dopo uno spostamento nel cestino, si rimisura la voce del cestino.
                    self.targets
                        .iter()
                        .filter(|t| {
                            matches!(t.action, Action::EmptyRecycleBin) || t.name == trash_name()
                        })
                        .map(|t| t.id)
                        .collect()
                };
                thread::spawn(move || {
                    let o = cleanup::delete_path(&full, size, permanent);
                    let _ = tx.send(Msg::OpDone {
                        label: name,
                        freed: o.freed,
                        errors: o.errs.count,
                        first_err: o.errs.first,
                        kept_repos: 0,
                        gone: if o.gone { vec![full] } else { Vec::new() },
                        remeasure: recycle,
                        trashed: !permanent,
                    });
                });
            }
            Pending::Clean { ids } => {
                let jobs: Vec<(usize, PathBuf, Action, u64, bool)> = self
                    .targets
                    .iter_mut()
                    .filter(|t| ids.contains(&t.id))
                    .map(|t| {
                        t.selected = false;
                        (
                            t.id,
                            t.path.clone(),
                            t.action.clone(),
                            t.size.unwrap_or(0),
                            t.from_scan,
                        )
                    })
                    .collect();
                self.busy = Some(trf!(
                    "Pulizia di {} in corso…",
                    "Cleaning {}…",
                    plural(jobs.len(), tr!("voce", "entry"), tr!("voci", "entries"))
                ));
                let label = tr!("Pulizia", "Cleanup").to_string();
                thread::spawn(move || {
                    let (mut freed, mut errors, mut first_err, mut kept_repos) = (0, 0, None, 0);
                    let (mut gone, mut remeasure) = (Vec::new(), Vec::new());
                    for (id, path, action, size, from_scan) in jobs {
                        let o = cleanup::run(&path, &action, size);
                        freed += o.freed;
                        errors += o.errs.count;
                        kept_repos += o.errs.kept_repos;
                        if first_err.is_none() {
                            first_err = o.errs.first;
                        }
                        if o.gone {
                            gone.push(path);
                        }
                        if !from_scan {
                            remeasure.push(id);
                        }
                    }
                    let _ = tx.send(Msg::OpDone {
                        label,
                        freed,
                        errors,
                        first_err,
                        kept_repos,
                        gone,
                        remeasure,
                        trashed: false,
                    });
                });
            }
        }
    }

    // ------------------------------------------------------------ menu delle azioni

    fn item(key: char, label: impl Into<String>, cmd: Cmd) -> MenuItem {
        MenuItem {
            key,
            label: label.into(),
            cmd,
            danger: false,
            disabled: None,
        }
    }

    fn delete_items(path: &Path) -> [MenuItem; 2] {
        let blocked = cleanup::protection(path).map(|w| trf!("protetto: {w}", "protected: {w}"));
        [
            MenuItem {
                key: 'd',
                label: trf!("Sposta nel {}", "Move to the {}", trash_name()),
                cmd: Cmd::Trash,
                danger: false,
                disabled: blocked.clone(),
            },
            MenuItem {
                key: 'e',
                label: tr!("Elimina definitivamente", "Delete permanently").into(),
                cmd: Cmd::Delete,
                danger: true,
                disabled: blocked,
            },
        ]
    }

    fn show_item() -> MenuItem {
        Self::item(
            'o',
            trf!("Mostra in {}", "Show in {}", file_manager()),
            Cmd::ShowInFileManager,
        )
    }

    fn copy_item() -> MenuItem {
        Self::item(
            'c',
            tr!("Copia il percorso", "Copy the path"),
            Cmd::CopyPath,
        )
    }

    fn open_menu(&mut self) {
        let menu = match self.tab {
            Tab::Esplora => {
                let Some(c) = self.selected_child() else {
                    return;
                };
                let (kind, name, size, files, empty) = (
                    c.kind,
                    c.name.clone(),
                    c.size,
                    c.files,
                    c.children.is_empty(),
                );
                let path = self.view_path().join(&name);
                let mut items = Vec::new();
                match kind {
                    Kind::Dir => {
                        let mut open =
                            Self::item('\n', tr!("Apri la cartella", "Open the folder"), Cmd::Open);
                        if empty {
                            open.disabled =
                                Some(tr!("vuota o non leggibile", "empty or unreadable").into());
                        }
                        items.push(open);
                        items.push(Self::item(
                            'n',
                            tr!("Analizza solo questa cartella", "Analyze only this folder"),
                            Cmd::ScanHere,
                        ));
                        items.push(Self::show_item());
                        items.push(Self::copy_item());
                        items.extend(Self::delete_items(&path));
                    }
                    Kind::File => {
                        items.push(Self::show_item());
                        items.push(Self::copy_item());
                        items.extend(Self::delete_items(&path));
                    }
                    Kind::Group => items.push(Self::item(
                        'o',
                        trf!(
                            "Apri la cartella in {}",
                            "Open the folder in {}",
                            file_manager()
                        ),
                        Cmd::ShowInFileManager,
                    )),
                }
                let path = if kind == Kind::Group {
                    self.view_path()
                } else {
                    path
                };
                Menu {
                    title: name,
                    path,
                    size,
                    files,
                    is_dir: kind == Kind::Dir,
                    target_id: None,
                    items,
                    sel: 0,
                }
            }
            Tab::FileGrandi => {
                let Some((path, size)) = self
                    .big_state
                    .selected()
                    .and_then(|i| self.big_files.get(i))
                    .cloned()
                else {
                    return;
                };
                let mut items = vec![
                    Self::item(
                        't',
                        tr!(
                            "Mostra nella sua cartella (scheda Esplora)",
                            "Show in its folder (Explore tab)"
                        ),
                        Cmd::ShowInTree,
                    ),
                    Self::show_item(),
                    Self::copy_item(),
                ];
                items.extend(Self::delete_items(&path));
                let title = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Menu {
                    title,
                    path,
                    size,
                    files: 1,
                    is_dir: false,
                    target_id: None,
                    items,
                    sel: 0,
                }
            }
            Tab::Pulizia => {
                let Some(i) = self.selected_target() else {
                    return;
                };
                let t = &self.targets[i];
                let mut clean = Self::item(
                    'p',
                    tr!("Pulisci questa voce", "Clean this entry"),
                    Cmd::CleanOne,
                );
                let mut toggle = Self::item(
                    ' ',
                    if t.selected {
                        tr!("Togli dalla selezione", "Remove from selection")
                    } else {
                        tr!("Aggiungi alla selezione", "Add to selection")
                    },
                    Cmd::Toggle,
                );
                if !t.actionable() {
                    clean.disabled = Some(
                        tr!("va gestita con la sua app", "must be handled with its app").into(),
                    );
                    toggle.disabled = clean.disabled.clone();
                }
                let items = vec![
                    clean,
                    toggle,
                    Self::item(
                        'n',
                        tr!(
                            "Analizza la cartella (scheda Esplora)",
                            "Analyze the folder (Explore tab)"
                        ),
                        Cmd::ScanHere,
                    ),
                    Self::show_item(),
                    Self::copy_item(),
                ];
                Menu {
                    title: t.name.clone(),
                    path: t.path.clone(),
                    size: t.size.unwrap_or(0),
                    files: 0,
                    is_dir: true,
                    target_id: Some(i),
                    items,
                    sel: 0,
                }
            }
        };
        self.modal = Modal::Menu(menu);
    }

    fn run_menu(&mut self, menu: Menu, idx: usize) {
        let Some(item) = menu.items.get(idx) else {
            return;
        };
        if let Some(why) = &item.disabled {
            self.say(
                Tone::Warn,
                trf!(
                    "«{}» non disponibile: {why}.",
                    "“{}” is not available: {why}.",
                    item.label
                ),
            );
            self.modal = Modal::Menu(Menu { sel: idx, ..menu });
            return;
        }
        match item.cmd {
            Cmd::Open => self.activate(),
            Cmd::ScanHere => self.start_scan(menu.path),
            Cmd::ShowInFileManager => cleanup::open_in_file_manager(&menu.path),
            Cmd::CopyPath => {
                if cleanup::copy_to_clipboard(&menu.path.display().to_string()) {
                    self.say(
                        Tone::Ok,
                        trf!(
                            "Percorso copiato: {}",
                            "Path copied: {}",
                            menu.path.display()
                        ),
                    );
                } else {
                    self.say(
                        Tone::Err,
                        tr!(
                            "Non sono riuscito a copiare negli appunti (su Linux serve wl-copy, xclip o xsel).",
                            "Could not copy to the clipboard (on Linux it needs wl-copy, xclip or xsel)."
                        ),
                    );
                }
            }
            Cmd::ShowInTree => self.reveal_in_tree(&menu.path),
            Cmd::Trash => self.ask_delete(menu.path, menu.size, menu.files, menu.is_dir, false),
            Cmd::Delete => self.ask_delete(menu.path, menu.size, menu.files, menu.is_dir, true),
            Cmd::CleanOne => self.ask_clean(menu.target_id),
            Cmd::Toggle => {
                if let Some(t) = menu.target_id.and_then(|i| self.targets.get_mut(i)) {
                    t.selected = !t.selected;
                }
            }
        }
    }

    fn reveal_in_tree(&mut self, path: &Path) {
        let parent = path.parent().unwrap_or(path);
        if let Some(names) = scan::rel_names(&self.root_path, parent) {
            let file = path.file_name().map(|n| n.to_string_lossy().into_owned());
            self.filter.clear();
            self.goto_names(&names, file.as_deref(), 0);
            self.tab = Tab::Esplora;
        }
    }

    // ------------------------------------------------------------ azioni principali

    /// Invio / doppio clic sulla voce selezionata.
    fn activate(&mut self) {
        match self.tab {
            Tab::Esplora if self.scanning.is_some() => {}
            Tab::Esplora if self.tree.is_none() => {
                if let Some(p) = self.home_state.selected().and_then(|i| self.places.get(i)) {
                    self.start_scan(p.path.clone());
                }
            }
            Tab::Esplora => {
                let Some(idx) = self.selected_index() else {
                    return;
                };
                let Some(c) = self.selected_child() else {
                    return;
                };
                let (kind, empty, denied) = (c.kind, c.children.is_empty(), c.denied);
                match kind {
                    Kind::Dir if !empty => {
                        let mut nav = self.nav.clone();
                        nav.push(idx);
                        self.set_nav(nav, None);
                    }
                    Kind::Dir if denied => self.say(
                        Tone::Warn,
                        tr!(
                            "Cartella non leggibile: avvia il programma come amministratore (o con sudo).",
                            "Unreadable folder: run the program as administrator (or with sudo)."
                        ),
                    ),
                    Kind::Dir => self.say(Tone::Info, tr!("La cartella è vuota.", "The folder is empty.")),
                    _ => self.open_menu(),
                }
            }
            Tab::FileGrandi => self.open_menu(),
            Tab::Pulizia => self.ask_clean(None),
        }
    }

    fn go_up(&mut self) {
        if let Some(i) = self.nav.pop() {
            let nav = self.nav.clone();
            self.set_nav(nav, Some(i));
        } else {
            self.say(
                Tone::Info,
                tr!(
                    "Sei nella cartella di partenza. N = analizza un'altra cartella, G = scrivi un percorso.",
                    "You are at the starting folder. N = analyze another folder, G = type a path."
                ),
            );
        }
    }

    fn toggle_target(&mut self) {
        if let Some(i) = self.selected_target() {
            let t = &mut self.targets[i];
            if t.actionable() {
                t.selected = !t.selected;
            } else {
                let msg = trf!(
                    "«{}» va gestita a mano: {}",
                    "“{}” must be handled by hand: {}",
                    t.name,
                    t.detail
                );
                self.say(Tone::Warn, msg);
            }
        }
    }

    fn new_analysis(&mut self) {
        if let Some(old) = self.tree.take() {
            thread::spawn(move || drop(old));
        }
        self.big_files.clear();
        self.targets.retain(|t| !t.from_scan);
        self.nav.clear();
        self.filter.clear();
        self.drives = read_drives();
        self.places = build_places(&self.drives);
        self.tab = Tab::Esplora;
        self.say(
            Tone::Info,
            tr!(
                "Scegli un disco o una cartella e premi Invio.",
                "Pick a drive or a folder and press Enter."
            ),
        );
    }

    fn move_current(&mut self, delta: isize) {
        match self.tab {
            Tab::Esplora if self.tree.is_some() => {
                let len = self.visible().len();
                move_sel(&mut self.explore_state, len, delta);
            }
            Tab::Esplora => move_sel(&mut self.home_state, self.places.len(), delta),
            Tab::FileGrandi => move_sel(&mut self.big_state, self.big_files.len(), delta),
            Tab::Pulizia => self.move_clean(delta),
        }
    }

    // ------------------------------------------------------------ tasti

    pub fn on_key(&mut self, k: KeyEvent) {
        if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        if self.on_modal_key(k) {
            return;
        }
        if self.filter_edit {
            match k.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.filter_edit = false;
                }
                KeyCode::Enter | KeyCode::Down | KeyCode::Up => {
                    self.filter_edit = false;
                    if k.code != KeyCode::Enter {
                        self.move_current(nav_delta(k.code).unwrap_or(0));
                    }
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.explore_state.select(Some(0));
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.explore_state.select(Some(0));
                }
                _ => {}
            }
            return;
        }

        // Tasti validi ovunque.
        match k.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                if self.busy.is_some() || self.scanning.is_some() {
                    self.modal = Modal::Confirm {
                        title: tr!(" Uscire? ", " Quit? ").into(),
                        body: vec![
                            tr!(
                                "C'è un'operazione in corso: uscendo la interrompi.",
                                "An operation is running: quitting will stop it."
                            )
                            .into(),
                        ],
                        warn: None,
                        danger: false,
                        yes_label: tr!("Esci", "Quit").into(),
                        yes_focus: false,
                        action: Pending::Quit,
                    };
                } else {
                    self.quit = true;
                }
                return;
            }
            KeyCode::Char('?') | KeyCode::F(1) => return self.modal = Modal::Help,
            KeyCode::Tab => return self.tab = Tab::ALL[(self.tab.index() + 1) % 3],
            KeyCode::BackTab => return self.tab = Tab::ALL[(self.tab.index() + 2) % 3],
            KeyCode::Char('1') => return self.tab = Tab::Esplora,
            KeyCode::Char('2') => return self.tab = Tab::FileGrandi,
            KeyCode::Char('3') => return self.tab = Tab::Pulizia,
            KeyCode::Char('g') | KeyCode::Char('G') => {
                return self.modal = Modal::Input { buf: String::new() };
            }
            _ => {}
        }
        if let Some(d) = nav_delta(k.code) {
            return self.move_current(d);
        }

        match self.tab {
            Tab::Esplora if self.scanning.is_some() => {
                if k.code == KeyCode::Esc {
                    if let Some(s) = &self.scanning {
                        s.prog.cancel.store(true, Ordering::Relaxed);
                    }
                    self.say(
                        Tone::Info,
                        tr!("Interruzione dell'analisi…", "Stopping the analysis…"),
                    );
                }
            }
            Tab::Esplora if self.tree.is_none() => {
                if k.code == KeyCode::Enter {
                    self.activate();
                }
            }
            Tab::Esplora => self.key_explore(k),
            Tab::FileGrandi => self.key_big(k),
            Tab::Pulizia => self.key_clean(k),
        }
    }

    /// Tasti quando è aperta una finestra. Restituisce true se il tasto è stato gestito.
    fn on_modal_key(&mut self, k: KeyEvent) -> bool {
        match std::mem::replace(&mut self.modal, Modal::None) {
            Modal::None => return false,
            Modal::Help => {}
            Modal::Menu(mut m) => match k.code {
                KeyCode::Esc | KeyCode::Left | KeyCode::Backspace => {}
                KeyCode::Up => {
                    m.sel = m.sel.saturating_sub(1);
                    self.modal = Modal::Menu(m);
                }
                KeyCode::Down => {
                    m.sel = (m.sel + 1).min(m.items.len().saturating_sub(1));
                    self.modal = Modal::Menu(m);
                }
                KeyCode::Enter | KeyCode::Right => {
                    let i = m.sel;
                    self.run_menu(m, i);
                }
                KeyCode::Delete => {
                    let cmd = if k.modifiers.contains(KeyModifiers::SHIFT) {
                        Cmd::Delete
                    } else {
                        Cmd::Trash
                    };
                    match m.items.iter().position(|it| it.cmd == cmd) {
                        Some(i) => self.run_menu(m, i),
                        None => self.modal = Modal::Menu(m),
                    }
                }
                KeyCode::Char(c) => match m
                    .items
                    .iter()
                    .position(|it| it.key == c.to_ascii_lowercase())
                {
                    Some(i) => self.run_menu(m, i),
                    None => self.modal = Modal::Menu(m),
                },
                _ => self.modal = Modal::Menu(m),
            },
            Modal::Confirm {
                title,
                body,
                warn,
                danger,
                yes_label,
                yes_focus,
                action,
            } => match k.code {
                KeyCode::Char('s' | 'S' | 'y' | 'Y') => self.execute(action),
                KeyCode::Enter if yes_focus => self.execute(action),
                KeyCode::Char('n' | 'N') | KeyCode::Esc | KeyCode::Enter => {
                    self.say(Tone::Info, tr!("Annullato.", "Cancelled."))
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                    self.modal = Modal::Confirm {
                        title,
                        body,
                        warn,
                        danger,
                        yes_label,
                        yes_focus: !yes_focus,
                        action,
                    };
                }
                _ => {
                    self.modal = Modal::Confirm {
                        title,
                        body,
                        warn,
                        danger,
                        yes_label,
                        yes_focus,
                        action,
                    }
                }
            },
            Modal::Input { mut buf } => match k.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    let p = PathBuf::from(expand_home(buf.trim().trim_matches('"')));
                    if p.is_dir() {
                        self.start_scan(p);
                    } else {
                        self.say(
                            Tone::Err,
                            trf!(
                                "Cartella non trovata: {}",
                                "Folder not found: {}",
                                p.display()
                            ),
                        );
                    }
                }
                KeyCode::Backspace => {
                    buf.pop();
                    self.modal = Modal::Input { buf };
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    self.modal = Modal::Input { buf };
                }
                _ => self.modal = Modal::Input { buf },
            },
        }
        true
    }

    fn key_explore(&mut self, k: KeyEvent) {
        if let Some(permanent) = is_delete(&k) {
            return self.delete_selected(permanent);
        }
        match k.code {
            KeyCode::Enter | KeyCode::Right => self.activate(),
            KeyCode::Backspace | KeyCode::Left => self.go_up(),
            KeyCode::Esc if !self.filter.is_empty() => self.filter.clear(),
            KeyCode::Esc => self.go_up(),
            KeyCode::Char(' ') | KeyCode::Char('m') | KeyCode::Menu => self.open_menu(),
            KeyCode::Char('/') | KeyCode::Char('f') | KeyCode::Char('F') => {
                self.filter_edit = true;
                self.say(
                    Tone::Info,
                    tr!(
                        "Scrivi per filtrare i nomi in questa cartella · Invio conferma · Esc annulla",
                        "Type to filter names in this folder · Enter confirms · Esc cancels"
                    ),
                );
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.sort = if self.sort == Sort::Size {
                    Sort::Name
                } else {
                    Sort::Size
                };
                self.explore_state.select(Some(0));
                let msg = if self.sort == Sort::Name {
                    tr!("Ordinati per nome.", "Sorted by name.")
                } else {
                    tr!("Ordinati per dimensione.", "Sorted by size.")
                };
                self.say(Tone::Info, msg);
            }
            KeyCode::Char('o') | KeyCode::Char('O') => {
                if let Some(p) = self.selected_path() {
                    cleanup::open_in_file_manager(&p);
                }
            }
            KeyCode::F(5) | KeyCode::Char('r') | KeyCode::Char('R') => {
                self.start_scan(self.root_path.clone())
            }
            KeyCode::Char('n') | KeyCode::Char('N') => self.new_analysis(),
            _ => {}
        }
    }

    fn delete_selected(&mut self, permanent: bool) {
        let Some(c) = self.selected_child() else {
            return;
        };
        if c.kind == Kind::Group {
            return self.say(
                Tone::Warn,
                trf!(
                    "Il gruppo di file piccoli non si elimina in blocco: O = aprilo in {}.",
                    "The group of small files cannot be deleted at once: O = open it in {}.",
                    file_manager()
                ),
            );
        }
        let (size, files, is_dir) = (c.size, c.files, c.kind == Kind::Dir);
        let full = self.view_path().join(&c.name);
        self.ask_delete(full, size, files, is_dir, permanent);
    }

    fn key_big(&mut self, k: KeyEvent) {
        let Some((path, size)) = self
            .big_state
            .selected()
            .and_then(|i| self.big_files.get(i))
            .cloned()
        else {
            return;
        };
        if let Some(permanent) = is_delete(&k) {
            return self.ask_delete(path, size, 1, false, permanent);
        }
        match k.code {
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('m') | KeyCode::Menu => {
                self.open_menu()
            }
            KeyCode::Char('o') | KeyCode::Char('O') => cleanup::open_in_file_manager(&path),
            KeyCode::Char('t') | KeyCode::Char('T') | KeyCode::Right => self.reveal_in_tree(&path),
            _ => {}
        }
    }

    fn key_clean(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Char(' ') => {
                self.toggle_target();
                self.move_clean(1);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let mut n = 0;
                for t in &mut self.targets {
                    if t.safety == Safety::Sicuro && t.actionable() && t.size.unwrap_or(0) > 0 {
                        t.selected = true;
                        n += 1;
                    }
                }
                self.say(
                    Tone::Info,
                    trf!(
                        "Voci sicure selezionate: {n} · Invio = pulisci",
                        "Safe entries selected: {n} · Enter = clean"
                    ),
                );
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                for t in &mut self.targets {
                    t.selected = false;
                }
                self.say(Tone::Info, tr!("Selezione azzerata.", "Selection cleared."));
            }
            KeyCode::Enter => self.activate(),
            KeyCode::Char('m') | KeyCode::Menu | KeyCode::Right => self.open_menu(),
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if let Some(p) = self.selected_target().map(|i| self.targets[i].path.clone()) {
                    self.start_scan(p);
                }
            }
            KeyCode::Char('o') | KeyCode::Char('O') => {
                if let Some(i) = self.selected_target() {
                    cleanup::open_in_file_manager(&self.targets[i].path);
                }
            }
            KeyCode::F(5) | KeyCode::Char('r') | KeyCode::Char('R') => {
                let ids = self
                    .targets
                    .iter()
                    .filter(|t| !t.from_scan)
                    .map(|t| t.id)
                    .collect();
                self.measure_targets(ids);
                self.say(
                    Tone::Info,
                    tr!("Ricalcolo delle dimensioni…", "Measuring sizes again…"),
                );
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ mouse

    fn on_mouse(&mut self, m: MouseEvent) {
        match m.kind {
            MouseEventKind::ScrollUp => {
                if let Modal::Menu(menu) = &mut self.modal {
                    menu.sel = menu.sel.saturating_sub(1);
                } else if matches!(self.modal, Modal::None) {
                    self.move_current(-3);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Modal::Menu(menu) = &mut self.modal {
                    menu.sel = (menu.sel + 1).min(menu.items.len().saturating_sub(1));
                } else if matches!(self.modal, Modal::None) {
                    self.move_current(3);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => self.click(m.column, m.row, false),
            MouseEventKind::Down(MouseButton::Right) => self.click(m.column, m.row, true),
            _ => {}
        }
    }

    fn click(&mut self, x: u16, y: u16, right: bool) {
        let inside = |r: &Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        match &self.modal {
            Modal::None => {}
            Modal::Menu(_) => {
                let hit = self.hits.menu.iter().find(|(r, _)| inside(r)).map(|h| h.1);
                if let Modal::Menu(m) = std::mem::replace(&mut self.modal, Modal::None)
                    && let Some(i) = hit
                {
                    self.run_menu(m, i);
                }
                return;
            }
            Modal::Confirm { .. } => {
                if let Some(&(_, yes)) = self.hits.buttons.iter().find(|(r, _)| inside(r))
                    && let Modal::Confirm { action, .. } =
                        std::mem::replace(&mut self.modal, Modal::None)
                {
                    if yes {
                        self.execute(action);
                    } else {
                        self.say(Tone::Info, tr!("Annullato.", "Cancelled."));
                    }
                }
                return;
            }
            Modal::Help => {
                self.modal = Modal::None;
                return;
            }
            Modal::Input { .. } => return,
        }
        if let Some(&(_, tab)) = self.hits.tabs.iter().find(|(r, _)| inside(r)) {
            self.tab = tab;
            return;
        }
        if let Some(&(_, depth)) = self.hits.crumbs.iter().find(|(r, _)| inside(r)) {
            if depth < self.nav.len() {
                let child = self.nav[depth];
                let nav = self.nav[..depth].to_vec();
                self.set_nav(nav, Some(child));
            }
            return;
        }
        let Some((area, offset)) = self.hits.list else {
            return;
        };
        if !inside(&area) {
            return;
        }
        let row = offset + (y - area.y) as usize;
        let double = self.last_click.is_some_and(|(t, lx, ly)| {
            t.elapsed() < Duration::from_millis(450) && lx == x && ly == y
        });
        self.last_click = Some((Instant::now(), x, y));
        let len = match self.tab {
            Tab::Esplora if self.tree.is_some() => self.visible().len(),
            Tab::Esplora => self.places.len(),
            Tab::FileGrandi => self.big_files.len(),
            Tab::Pulizia => self.clean_rows().len(),
        };
        if row >= len {
            return;
        }
        let state = match self.tab {
            Tab::Esplora if self.tree.is_some() => &mut self.explore_state,
            Tab::Esplora => &mut self.home_state,
            Tab::FileGrandi => &mut self.big_state,
            Tab::Pulizia => &mut self.tgt_state,
        };
        state.select(Some(row));
        if self.tab == Tab::Pulizia {
            if self.selected_target().is_none() {
                return self.move_clean(1);
            }
            // Clic sulla casella o doppio clic = seleziona; destro = menu.
            if right {
                self.open_menu();
            } else if double || x < area.x + 6 {
                self.toggle_target();
            }
            return;
        }
        if right {
            if self.tree.is_some() || self.tab != Tab::Esplora {
                self.open_menu();
            }
        } else if double {
            self.activate();
        }
    }
}

/// `~/progetti` → percorso completo (comodo su Linux/macOS).
fn expand_home(s: &str) -> String {
    match (s.strip_prefix('~'), cleanup::home_dir()) {
        (Some(rest), Some(h)) => format!("{}{}", h.display(), rest),
        _ => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::fs;

    fn render(app: &mut App) -> String {
        let mut t = Terminal::new(TestBackend::new(150, 40)).unwrap();
        t.draw(|f| ui::draw(f, app)).unwrap();
        t.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    fn key(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn wait_op(app: &mut App) {
        loop {
            let m = app
                .rx
                .recv_timeout(Duration::from_secs(30))
                .expect("operazione non terminata");
            let done = matches!(m, Msg::OpDone { .. });
            app.on_msg(m);
            if done {
                break;
            }
        }
    }

    #[test]
    fn renders_screens_and_deletes() {
        let base = std::env::temp_dir().join(format!("disk-space-tui-ui-{}", std::process::id()));
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("grande.bin"), vec![0u8; 5000]).unwrap();
        fs::write(base.join("sub").join("piccolo.bin"), vec![0u8; 1000]).unwrap();

        let mut app = App::new();
        let home = render(&mut app);
        assert!(
            home.contains("Cosa vuoi analizzare") || home.contains("What do you want to analyze"),
            "{home}"
        );

        let tree = scan::scan(&base, &Progress::default());
        app.on_msg(Msg::ScanDone {
            root: base.clone(),
            tree,
            elapsed: Duration::from_millis(3),
            errors: 0,
            cancelled: false,
        });
        let s = render(&mut app);
        assert!(s.contains("grande.bin") && s.contains("sub"), "{s}");

        // Entra in sub con Invio e torna su con Backspace.
        key(&mut app, KeyCode::Down);
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.nav.len(), 1);
        assert!(render(&mut app).contains("piccolo.bin"));
        key(&mut app, KeyCode::Backspace);
        assert_eq!(app.nav.len(), 0);
        assert_eq!(app.selected_child().unwrap().name, "sub");

        // Filtro per nome.
        key(&mut app, KeyCode::Char('/'));
        for c in "grande".chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.visible().len(), 1);
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.visible().len(), 2);

        // Menu delle azioni e tutte le schede.
        key(&mut app, KeyCode::Char(' '));
        assert!(matches!(app.modal, Modal::Menu(_)));
        key(&mut app, KeyCode::Esc);
        for tab in [KeyCode::Char('2'), KeyCode::Char('3'), KeyCode::Char('?')] {
            key(&mut app, tab);
            render(&mut app);
        }
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::Char('1'));

        // Eliminazione definitiva del file più grande: Invio di default è «Annulla».
        key(&mut app, KeyCode::Home);
        app.on_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::SHIFT));
        assert!(matches!(app.modal, Modal::Confirm { .. }));
        key(&mut app, KeyCode::Enter);
        assert!(
            base.join("grande.bin").exists(),
            "Invio non deve confermare un'eliminazione definitiva"
        );
        app.on_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::SHIFT));
        key(&mut app, KeyCode::Char('y'));
        wait_op(&mut app);
        assert!(!base.join("grande.bin").exists());
        assert_eq!(app.tree.as_ref().unwrap().size, 1000);
        assert!(
            app.big_files
                .iter()
                .all(|(p, _)| !p.ends_with("grande.bin"))
        );
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn clean_selection_skips_headers() {
        let mut app = App::new();
        let rows = app.clean_rows();
        if rows.is_empty() {
            return;
        }
        app.tgt_state.select(Some(0));
        app.move_clean(0);
        assert!(app.selected_target().is_some());
        for _ in 0..rows.len() * 2 {
            app.move_clean(1);
            assert!(app.selected_target().is_some());
        }
        for _ in 0..rows.len() * 2 {
            app.move_clean(-1);
            assert!(app.selected_target().is_some());
        }
    }
}

#[cfg(test)]
mod dump {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn print(app: &mut App, w: u16, h: u16, label: &str) {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| ui::draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        println!("===== {label} ({w}x{h})");
        for y in 0..h {
            let line: String = (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect();
            println!("{}", line.trim_end());
        }
    }

    /// Stampa le schermate: `DUMP_ROOT=<cartella> DUMP_LANG=en cargo test dump_screens -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_screens() {
        if let Some(l) = std::env::var("DUMP_LANG")
            .ok()
            .as_deref()
            .and_then(crate::i18n::parse)
        {
            crate::i18n::set(l);
        }
        let mut app = App::new();
        let start = Instant::now();
        while app.targets.iter().any(|t| t.size.is_none())
            && start.elapsed() < Duration::from_secs(60)
        {
            if let Ok(m) = app.rx.recv_timeout(Duration::from_millis(200)) {
                app.on_msg(m);
            }
        }
        print(&mut app, 140, 34, "home");
        let root = PathBuf::from(std::env::var("DUMP_ROOT").unwrap_or_else(|_| ".".into()));
        let tree = scan::scan(&root, &Progress::default());
        app.on_msg(Msg::ScanDone {
            root,
            tree,
            elapsed: Duration::from_millis(3),
            errors: 0,
            cancelled: false,
        });
        print(&mut app, 140, 34, "explore");
        if let Ok(target) = std::env::var("DUMP_OPEN")
            && let Some(pos) = app
                .visible()
                .iter()
                .position(|&i| app.view().unwrap().children[i].name == target)
        {
            app.explore_state.select(Some(pos));
            app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            print(&mut app, 140, 26, "explore subfolder");
        }
        app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        print(&mut app, 140, 34, "menu");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
        print(&mut app, 140, 34, "confirm");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        print(&mut app, 90, 30, "explore (narrow)");
        app.tab = Tab::FileGrandi;
        print(&mut app, 140, 30, "big files");
        app.tab = Tab::Pulizia;
        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        print(&mut app, 140, 40, "cleanup");
        app.modal = Modal::Help;
        print(&mut app, 140, 34, "help");
    }
}
