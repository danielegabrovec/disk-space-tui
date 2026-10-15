//! Rapporto testuale (`--report`): stesso motore della TUI, output stampabile.

use crate::scan::{self, Kind, Node, Progress};
use crate::util::{fmt_count, fmt_pct, fmt_size};
use crate::{cleanup, tr, trf};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};

fn print_tree(n: &Node, depth: usize, max_depth: usize, top: usize, total: u64) {
    for c in n.children.iter().filter(|c| c.kind == Kind::Dir).take(top) {
        if c.size < total / 500 {
            break;
        }
        println!(
            "{}{:>10}  {:>6}  {}{}",
            "    ".repeat(depth),
            fmt_size(c.size),
            fmt_pct(c.size as f64 / total.max(1) as f64),
            c.name,
            if c.denied {
                tr!("  (accesso negato)", "  (access denied)")
            } else {
                ""
            }
        );
        if depth + 1 < max_depth {
            print_tree(c, depth + 1, max_depth, top, total);
        }
    }
}

pub fn run(path: &Path, top: usize, depth: usize) {
    let prog = Arc::new(Progress::default());
    let started = Instant::now();
    let p2 = prog.clone();
    let ticker = std::thread::spawn(move || {
        while !p2.cancel.load(Relaxed) {
            eprint!(
                "\r  {}",
                trf!(
                    "{} file, {}…        ",
                    "{} files, {}…        ",
                    fmt_count(p2.files.load(Relaxed)),
                    fmt_size(p2.bytes.load(Relaxed))
                )
            );
            let _ = std::io::stderr().flush();
            std::thread::sleep(Duration::from_millis(500));
        }
        eprintln!();
    });
    let tree = scan::scan(path, &prog);
    prog.cancel.store(true, Relaxed);
    let _ = ticker.join();

    println!(
        "{}",
        trf!(
            "== {} — {} in {} file · {:.1} s · cartelle non accessibili: {}",
            "== {} — {} in {} files · {:.1} s · unreadable folders: {}",
            path.display(),
            fmt_size(tree.size),
            fmt_count(tree.files),
            started.elapsed().as_secs_f64(),
            fmt_count(prog.errors.load(Relaxed))
        )
    );
    println!("\n-- {}", tr!("Cartelle più grandi", "Biggest folders"));
    print_tree(&tree, 0, depth, top, tree.size);

    println!("\n-- {}", tr!("File più grandi", "Biggest files"));
    for (p, s) in scan::top_files(&tree, path, 30) {
        println!("{:>10}  {}", fmt_size(s), p.display());
    }

    let dev = scan::dev_artifacts(&tree, path);
    if !dev.is_empty() {
        let tot: u64 = dev.iter().map(|d| d.size).sum();
        println!(
            "\n-- {}",
            trf!(
                "File di compilazione rigenerabili ({} in {} cartelle)",
                "Regenerable build files ({} in {} folders)",
                fmt_size(tot),
                dev.len()
            )
        );
        let mut by_kind: Vec<(&str, u64, usize)> = Vec::new();
        for d in &dev {
            match by_kind.iter_mut().find(|k| k.0 == d.what) {
                Some(k) => {
                    k.1 += d.size;
                    k.2 += 1;
                }
                None => by_kind.push((d.what, d.size, 1)),
            }
        }
        by_kind.sort_unstable_by_key(|k| std::cmp::Reverse(k.1));
        for (what, size, n) in by_kind {
            println!("{:>10}  {n:>4} ×  {what}", fmt_size(size));
        }
        println!();
        for d in dev.iter().take(30) {
            println!("{:>10}  {}", fmt_size(d.size), d.path.display());
        }
    }

    println!(
        "\n-- {}",
        tr!("Voci di pulizia note", "Known cleanup entries")
    );
    for t in cleanup::fixed_targets() {
        let (s, partial) = cleanup::dir_size(&t.path);
        println!(
            "{:>11}  {:<13} {}  [{}]",
            format!("{}{}", if partial { "≥" } else { "" }, fmt_size(s)),
            t.safety.label(),
            t.name,
            t.path.display()
        );
    }
}
