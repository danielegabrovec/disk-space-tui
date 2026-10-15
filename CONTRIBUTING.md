# Contributing to Disk Space TUI

Thanks for helping! Bug reports, ideas and pull requests are welcome.
*Segnalazioni e contributi anche in italiano.*

## Getting started

```sh
git clone https://github.com/danielegabrovec/disk-space-tui
cd disk-space-tui
cargo run --release                 # the interactive interface
cargo run --release -- --report .   # the text report
```

Rust 1.88 or newer is required.

## Before opening a pull request

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same checks on Windows, Linux and macOS.

To review UI changes without a real terminal, print every screen as text:

```sh
DUMP_ROOT=/some/folder DUMP_LANG=en cargo test --release dump_screens -- --ignored --nocapture
```

## Ground rules

1. **Safety first.** Anything that deletes files, changes `cleanup::protection`, adds a cleanup
   entry or a build-artifact rule must come with tests (see `cleanup.rs` and `scan.rs`). When in
   doubt, protect more, not less.
2. **Recognize build artifacts only next to their project file** (`package.json`, `Cargo.toml`,
   `*.csproj`…), never by name alone, and never inside installed programs (see `SKIP_DIRS` /
   `SKIP_ABS` in `scan.rs`).
3. **Automatic cleanups never enter git repositories** and never follow symbolic links.
4. **Every user-visible string exists in Italian and English.** Use the macros from `i18n.rs`:

   ```rust
   tr!("Testo italiano", "English text")                 // &'static str
   trf!("{n} file trovati", "{n} files found")          // String, like format!
   ```

   Italian first, English second.
5. **Platform-specific code lives in `cleanup.rs`** behind `cfg` attributes; keep the rest portable.
6. Keep dependencies minimal.

## Adding a cleanup entry

Add a `def(...)` to `common_defs` (all systems) or to the `os_defs` of one system in `cleanup.rs`:
name, short description, path, `Safety` level and `Action`. Entries whose path does not exist are
hidden automatically. Choose the safety level honestly:

| Level | Use for |
|---|---|
| `Sicuro` (safe) | deleting has no consequences (temp files, logs, trash) |
| `Rigenerabile` (regenerable) | recreated automatically (caches, downloaded packages) |
| `Attenzione` (caution) | deletable, but something is lost or slow to get back |
| `Manuale` (manual) | must be managed by its own app: shown with `Action::None` |

## Reporting bugs

Please include your OS and terminal, the `disk-space-tui --version` output, what you did and what
happened. For wrong sizes, a `--report` excerpt helps a lot (remove anything private).
