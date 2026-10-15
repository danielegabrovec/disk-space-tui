//! disk-space-tui — vedere cosa occupa il disco e liberare spazio in sicurezza.
//! Windows, Linux e macOS; interfaccia in italiano o inglese.
//!
//! Uso:
//!   disk-space-tui                          interfaccia interattiva (TUI, con mouse)
//!   disk-space-tui --report <cartella>      rapporto testuale (top cartelle, file, pulizie)
//!   disk-space-tui --artifacts <cartella>   elenco dei file di compilazione (byte, percorso, tipo)
//!   disk-space-tui --delete-list <file>     elimina definitivamente i percorsi elencati (uno per riga)
//!   disk-space-tui --clean-temp             svuota la cartella temporanea (file > 24 ore, salta i repository git)
//!   --lang it|en                            lingua (predefinita: quella del sistema)

mod app;
mod assess;
mod cleanup;
mod i18n;
mod report;
mod scan;
mod ui;
mod util;

use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;
use std::path::{Path, PathBuf};
use util::fmt_size;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!("disk-space-tui {VERSION}");
    println!();
    println!(
        "{}",
        tr!(
            "Vedi cosa occupa il disco e libera spazio in sicurezza (Windows, Linux, macOS).",
            "See what takes up your disk and free space safely (Windows, Linux, macOS)."
        )
    );
    println!();
    println!("{}", tr!("Uso:", "Usage:"));
    println!(
        "  disk-space-tui                          {}",
        tr!("interfaccia interattiva", "interactive interface")
    );
    println!(
        "  disk-space-tui --report <dir> [--top N] [--depth D]   {}",
        tr!("rapporto testuale", "text report")
    );
    println!(
        "  disk-space-tui --artifacts <dir>        {}",
        tr!(
            "elenco dei file di compilazione (TSV)",
            "list of build artifacts (TSV)"
        )
    );
    println!(
        "  disk-space-tui --delete-list <file>     {}",
        tr!(
            "elimina definitivamente i percorsi elencati",
            "permanently delete the listed paths"
        )
    );
    println!(
        "  disk-space-tui --clean-temp             {}",
        tr!(
            "svuota i temporanei (file > 24 ore)",
            "empty the temp folder (files > 24 hours)"
        )
    );
    println!(
        "  --lang it|en                            {}",
        tr!("lingua dell'interfaccia", "interface language")
    );
    println!("  -h, --help · -V, --version");
}

fn main() -> std::io::Result<()> {
    // Le cartelle molto annidate fanno ricorsione profonda: thread con stack ampio.
    let _ = rayon::ThreadPoolBuilder::new()
        .stack_size(32 << 20)
        .build_global();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg_after = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    i18n::set(
        arg_after("--lang")
            .as_deref()
            .and_then(i18n::parse)
            .unwrap_or_else(i18n::detect),
    );

    if args.iter().any(|a| a == "-h" || a == "--help") {
        usage();
        return Ok(());
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("disk-space-tui {VERSION}");
        return Ok(());
    }
    if args.iter().any(|a| a == "--report") {
        let path = arg_after("--report")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let num = |flag: &str, default: usize| {
            arg_after(flag)
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        report::run(&path, num("--top", 12), num("--depth", 2));
        return Ok(());
    }
    if let Some(root) = arg_after("--artifacts") {
        list_artifacts(Path::new(&root));
        return Ok(());
    }
    if let Some(list) = arg_after("--delete-list") {
        delete_list(Path::new(&list))?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--clean-temp") {
        let action = cleanup::Action::ClearContents {
            min_age: Some(std::time::Duration::from_secs(24 * 3600)),
            skip_hidden: !cfg!(windows),
        };
        let o = cleanup::run(&std::env::temp_dir(), &action, 0);
        println!(
            "{}",
            trf!(
                "TEMP: liberati {} · non eliminati (in uso) {} · repository git lasciati {}",
                "TEMP: freed {} · not deleted (in use) {} · git repositories kept {}",
                fmt_size(o.freed),
                o.errs.count,
                o.errs.kept_repos
            )
        );
        return Ok(());
    }

    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        prev(info);
    }));
    let result = app::App::new().run(&mut terminal);
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn list_artifacts(root: &Path) {
    let tree = scan::scan(root, &scan::Progress::default());
    for a in scan::dev_artifacts(&tree, root) {
        println!("{}\t{}\t{}", a.size, a.path.display(), a.what);
    }
}

/// Elimina definitivamente ogni percorso del file (uno per riga), rispettando le protezioni.
fn delete_list(list: &Path) -> std::io::Result<()> {
    let text = std::fs::read_to_string(list)?;
    let (mut freed, mut failed) = (0u64, 0u64);
    for line in text
        .lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}'))
        .filter(|l| !l.is_empty())
    {
        let p = Path::new(line);
        if !p.exists() {
            println!("{}  {line}", tr!("ASSENTE ", "MISSING "));
            continue;
        }
        if let Some(why) = cleanup::protection(p) {
            println!("{} {line} ({why})", tr!("PROTETTO", "PROTECTED"));
            failed += 1;
            continue;
        }
        let size = cleanup::dir_size(p).0;
        let o = cleanup::delete_path(p, size, true);
        freed += o.freed;
        if o.gone {
            println!("OK {:>10}  {line}", fmt_size(o.freed));
        } else {
            failed += 1;
            println!(
                "{} {:>10}  {line} · {}{}",
                tr!("PARZIALE", "PARTIAL "),
                fmt_size(o.freed),
                trf!("{} non eliminati", "{} not deleted", o.errs.count),
                o.errs.first.map(|e| format!(" — {e}")).unwrap_or_default()
            );
        }
    }
    println!(
        "{}",
        trf!(
            "TOTALE liberati {} · voci non completate {failed}",
            "TOTAL freed {} · entries not completed {failed}",
            fmt_size(freed)
        )
    );
    Ok(())
}
