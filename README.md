# Disk Space TUI

[![CI](https://github.com/danielegabrovec/disk-space-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/danielegabrovec/disk-space-tui/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-informational)

**A fast terminal app to see what takes up your disk — and to free space without breaking anything.**

Point it at a drive or a folder: it scans in parallel (a 1.5 TB drive with 12 million files
takes about 20 seconds), shows the biggest folders and files, tells you **whether each item
is safe to delete and why**, and collects the usual space hogs (temporary files, caches,
the trash, `node_modules`, `target`, `.next`, `bin/obj`…) in one cleanup list.

Works on **Windows, Linux and macOS**, with keyboard and mouse. The interface is in
**English or Italian** (picked from the system language). 🇮🇹 [Leggimi in italiano](README.it.md)

```text
╭ Disk Space TUI ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│ C:\  ██████░░░░░░░░░░░░░░░░░░  24.4%  free   2.81 TB  of 3.72 TB  · NTFS                                                                 │
│ G:\  ███████░░░░░░░░░░░░░░░░░  28.2%  free   2.67 TB  of 3.72 TB  · FAT32                                                                │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
  1  Explore   2  Big files (13)   3  Cleanup (10.1 GB)
 Path: C:\demo › Projects   (click a part of the path to go back there)
╭ 2 items · sorted by size ──────────────────────────────────────────────────────────────────╮╭ Details ───────────────────────────────────╮
│▶    1.76 GB ███████░░░░░  54.5%  ▸ web-shop                                                ││web-shop  1.76 GB                           │
│     1.46 GB █████░░░░░░░  45.5%  ▸ rust-cli  ± git repository                              ││Folder · 3 files                            │
│                                                                                            ││Size           1.76 GB                      │
│                                                                                            ││Of the folder  54.5%                        │
│                                                                                            ││Of the drive   0.0%                         │
│                                                                                            ││ ? No information                           │
│                                                                                            ││I do not know what this is: check what it   │
│                                                                                            ││contains before deleting it. The Recycle Bin│
│                                                                                            ││lets you change your mind.                  │
│                                                                                            ││ Enter  open                                │
│                                                                                            ││ Space  all actions                         │
│                                                                                            ││ Del  Recycle Bin                           │
╰───────────────────────────────────────────────────────── in this folder: 3.22 GB · 5 files ╯╰────────────────────────────────────────────╯
 ✓ Analysis complete: 15.4 GB in 13 files · 0.0 s · 3.22 GB of build files in tab 3 Cleanup
  Enter  open   ←  back   Space  actions   Del  Recycle Bin   Shift+Del  delete   /  search   S  sort   F5  refresh   N  new analysis
```

## Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
- [Keyboard and mouse](#keyboard-and-mouse)
- [The three tabs](#the-three-tabs)
- [Safety model](#safety-model)
- [What gets cleaned, per platform](#what-gets-cleaned-per-platform)
- [Command-line mode](#command-line-mode)
- [How sizes are measured](#how-sizes-are-measured)
- [Platform notes and troubleshooting](#platform-notes-and-troubleshooting)
- [Building from source](#building-from-source)
- [Project layout](#project-layout)
- [Contributing](#contributing) · [License](#license)

## Features

- **Fast parallel scan** of a whole drive or any folder, with live progress, the folder being
  read and an estimated percentage when you scan an entire drive.
- **Explore** folders sorted by size, with bars, percentages, a clickable path, search
  (`/`) and sorting by name or size.
- **"Can I delete this?"** — a Details panel and colored tags (*system*, *regenerable*,
  *cleanable*, *git repository*, *caution*) explain every item before you touch it.
- **Big files**: the 500 largest files of the scan, with a jump to their folder.
- **Cleanup** grouped by risk: *safe* (temp files, trash, crash dumps…), *caches* (npm, pip,
  cargo, NuGet, Gradle, browsers…), *project build files* found by the scan, *with caution*,
  and *manage with their app* (Ollama, LM Studio, Docker… listed only to show their size).
- **Safe by design**: system folders and your main profile folders cannot be deleted from
  the app, git repositories are never touched by automatic cleanups, deleting goes to the
  trash by default, permanent deletions ask for confirmation with **Cancel** as the default.
- **Mouse support**: click, double-click, wheel, right-click for the action menu, clickable
  tabs, path, checkboxes and buttons.
- **Text report** and scripting commands (`--report`, `--artifacts`, `--delete-list`,
  `--clean-temp`) using the same engine and the same protections.
- Single self-contained binary, no runtime, no configuration.

## Installation

### Prebuilt binaries

Download the archive for your system from the
[Releases page](https://github.com/danielegabrovec/disk-space-tui/releases), extract it and run
`disk-space-tui` (`disk-space-tui.exe` on Windows).

| System | File |
|---|---|
| Windows 10/11 (x64) | `disk-space-tui-x86_64-pc-windows-msvc.zip` |
| Linux (x64) | `disk-space-tui-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Apple Silicon) | `disk-space-tui-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `disk-space-tui-x86_64-apple-darwin.tar.gz` |

On macOS, if Gatekeeper blocks the downloaded binary:
`xattr -d com.apple.quarantine ./disk-space-tui`.

### With Cargo

```sh
cargo install --git https://github.com/danielegabrovec/disk-space-tui
```

Requires Rust 1.88 or newer ([rustup.rs](https://rustup.rs)).

## Usage

```sh
disk-space-tui              # interactive interface
disk-space-tui --lang it    # force Italian (or: --lang en)
```

1. **Pick what to analyze** — a drive, your home folder, Desktop, Documents, Downloads… or press
   `G` and type/paste any path (`~` works on Linux/macOS).
2. **Explore** the biggest folders. The **Details** panel on the right tells you what an item
   is and whether it is safe to delete.
3. **Clean up** in tab 3: select entries with `Space` (or `A` for all safe ones) and press `Enter`.

Use a terminal at least 110 columns wide to get the Details panel on the side; narrower
terminals show it at the bottom. Windows Terminal, iTerm2, GNOME Terminal, Konsole, Alacritty,
WezTerm and kitty all work; the mouse needs a terminal with mouse reporting (all of the above).

The language is taken from the system (`it*` → Italian, anything else → English). Override it with
`--lang it|en` or the environment variable `DISK_SPACE_TUI_LANG=it|en`.

## Keyboard and mouse

The bottom bar always shows the keys that work right now; `?` opens the full help.

| Where | Keys |
|---|---|
| Everywhere | `Tab` / `1` `2` `3` switch tab · `↑` `↓` `PgUp` `PgDn` `Home` `End` move · `G` type a path · `?` help · `Q` quit |
| Explore | `Enter` / `→` open · `←` / `Backspace` / `Esc` back · `Space` **action menu** · `Del` move to trash · `Shift+Del` delete permanently · `/` search · `S` sort · `F5` rescan · `N` new analysis · `O` show in the file manager |
| Big files | `Enter` actions · `T` show in its folder · `Del` / `Shift+Del` · `O` file manager |
| Cleanup | `Space` select · `A` all safe entries · `X` clear selection · `Enter` clean · `M` actions · `N` analyze the folder · `F5` measure again |
| Dialogs | `Y`/`S` yes · `N` no · `←` `→` switch button · `Enter` the highlighted button · `Esc` cancel |
| Mouse | click selects · double-click opens · wheel scrolls · **right-click opens the action menu** · tabs, path parts, checkboxes and buttons are clickable |

`d` / `D` work as alternatives to `Del` / `Shift+Del` (handy on laptops and Macs without a
Delete key).

The **action menu** (`Space` or right-click) lists everything you can do with the selected item,
each with its shortcut: open, analyze only this folder, show in the file manager, copy the path,
move to the trash, delete permanently. Unavailable actions are greyed out with the reason.

```text
╭ web-shop ──────────────────────────────────────────────────────╮
│ What do you want to do?                                        │
│                                                                │
│ ▶ [Enter]  Open the folder                                     │
│   [N]      Analyze only this folder                            │
│   [O]      Show in File Explorer                               │
│   [C]      Copy the path                                       │
│   [D]      Move to the Recycle Bin                             │
│   [E]      Delete permanently                                  │
│                                                                │
╰─────────────────────────────────────────────────────── 1.76 GB ╯
```

## The three tabs

### 1 · Explore

The folder tree, sorted by size. Each row shows the size, a bar, the share of the parent folder,
the name and a tag when the item is recognized:

| Tag | Meaning |
|---|---|
| `✗ system` | protected: cannot be deleted from the app |
| `↻ regenerable` | build output, dependencies or cache: recreated automatically |
| `✓ cleanable` | a known cleanup entry (temp files, trash…) |
| `! caution` | can be deleted, but you lose something (downloaded models, app data…) |
| `± git repository` | a project under git: only what is committed and pushed is safe |

Folders with many small files show them grouped as `(N files under 1 MB)` to keep memory low.

### 2 · Big files

The 500 largest files found by the scan, with their folder. `T` jumps to the file in Explore.

### 3 · Cleanup

```text
╭ Disk Space TUI ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│ C:\  ██████░░░░░░░░░░░░░░░░░░  24.4%  free   2.81 TB  of 3.72 TB  · NTFS                                                                 │
│ G:\  ███████░░░░░░░░░░░░░░░░░  28.2%  free   2.67 TB  of 3.72 TB  · FAT32                                                                │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
  1  Explore   2  Big files (13)   3  Cleanup (10.1 GB)
╭ What can be cleaned ───────────────────────────────────────────────────────────────────────╮╭ Details ───────────────────────────────────╮
│   SAFE — can be deleted without consequences ─────────────────────────────────── 2.13 GB   ││Recycle Bin                                 │
│▶  [✓]    ≥29.6 MB  Recycle Bin                                                             ││                                            │
│   [✓]    ≥2.02 GB  Temporary files (user)                                                  ││Size           29.6 MB                      │
│   [ ]        ≥0 B  Temporary files (Windows)                                               ││                                            │
│   [✓]     54.2 MB  Windows Update downloads                                                ││ ✓ SAFE                                     │
│   [✓]    ≥2.79 MB  Windows Error Reporting                                                 ││Deleted files waiting: the space stays used │
│   [✓]     26.7 MB  Program crash dumps                                                     ││until you empty it                          │
│   CACHES — re-downloaded automatically when needed ───────────────────────────── 4.15 GB   ││                                            │
│   [ ]     6.49 KB  pip cache                                                               ││What it does: Empties the Recycle Bin       │
│   [ ]      447 MB  Chrome cache                                                            ││(permanent)                                 │
│   [ ]     53.6 MB  Edge cache                                                              ││                                            │
│   [ ]      156 MB  VS Code cache                                                           ││ Space  select                              │
│   [ ]      441 MB  Downloaded VS Code extensions                                           ││ Enter  clean                               │
│   [ ]      257 MB  Cargo cache (downloaded crates)                                         ││ N  analyze the folder                      │
│   [ ]     1.75 GB  Extracted crate sources                                                 ││ M  all actions                             │
│   [ ]     1.04 GB  Gradle cache                                                            ││                                            │
│   [ ]     37.5 MB  Local Maven repository                                                  ││C:\$Recycle.Bin                             │
│   PROJECT BUILD FILES — recreated by the build ───────────────────────────────── 3.22 GB   ││                                            │
│   [ ]     1.46 GB  Projects\rust-cli › target                                              ││                                            │
│   [ ]     1.17 GB  Projects\web-shop › .next                                               ││                                            │
│   [ ]      600 MB  Projects\web-shop › node_modules                                        ││                                            │
│   WITH CAUTION — you lose something downloaded ────────────────────────────────── 605 MB   ││                                            │
│   [ ]      605 MB  Hugging Face models                                                     ││                                            │
│   MANAGE WITH THEIR APP — listed here only to show their size ─────────────────── 178 GB   ││                                            │
│            720 MB  Google Drive cache                                                      ││                                            │
│           29.4 GB  Docker                                                                  ││                                            │
│           91.1 GB  Ollama models                                                           ││                                            │
│           44.9 GB  LM Studio models                                                        ││                                            │
│           11.5 GB  Downloads                                                               ││                                            │
╰───────────────────────────────────────────── ✓ 5 entries selected · 2.13 GB   Enter  clean ╯╰────────────────────────────────────────────╯
 ℹ Safe entries selected: 5 · Enter = clean
  Space  select   A  all safe   X  none   Enter  clean   M  actions   N  analyze   F5  re-measure   ?  help
```

Entries are grouped by risk, each group with its total:

- **Safe** — temporary files, trash, crash dumps, update downloads.
- **Caches** — package-manager and browser caches: re-downloaded when needed.
- **Project build files** — found by the scan: `node_modules`, `target`, `.next`, `.turbo`,
  `bin`/`obj`, CMake build folders… Each is recognized **only if the project file is next to it**
  (`package.json`, `Cargo.toml`, `*.csproj`, `*.sln`, `CMakeCache.txt`, `build.gradle`).
- **With caution** — e.g. downloaded AI models: can be deleted but take long to re-download.
- **Manage with their app** — Ollama, LM Studio, Docker, Downloads… shown only so you know their
  size; the Details panel tells you the right command (`ollama rm`, `docker system prune`…).

## Safety model

Deleting files is the dangerous part of a disk cleaner, so the rules are strict and covered by tests.

**Never deletable from the app** (the action is refused with the reason):

- the root of any drive and any mount point of another drive (Linux/macOS);
- **Windows**: `Windows` (except `Windows\Temp` and the Windows Update download cache),
  `Program Files`, `Program Files (x86)`, `ProgramData`, `Users`, `System Volume Information`,
  `$Recycle.Bin`, `Recovery`, `pagefile.sys`, `hiberfil.sys`, `swapfile.sys`;
- **Linux/macOS**: `/bin`, `/boot`, `/dev`, `/etc`, `/lib*`, `/proc`, `/run`, `/sbin`, `/snap`,
  `/srv`, `/sys`, `/usr`, `/var` (except `/var/tmp`), `/nix`, `/System`, `/Library`, `/private`
  and everything inside them; the folders `/home`, `/Users`, `/root`, `/opt`, `/Applications`,
  `/mnt`, `/media`, `/Volumes`, `/tmp` themselves;
- your **home folder** and its main folders (Desktop, Documents, Downloads, Pictures, Music,
  Videos, AppData / Library / `.config` / `.local`), and anything that contains them;
- anything inside `~/.ssh` and `~/.gnupg` (private keys).

**Warnings** in the confirmation dialog for git repositories, Obsidian vaults, installed programs,
application data and AI models managed by an app.

**Automatic cleanups** (the Cleanup tab and `--clean-temp`):

- never descend into a **git repository** (a folder with `.git`) — a worktree left in the temp
  folder may contain unsaved work; they are counted and reported as "left untouched";
- temp folders: only files **older than 24 hours**; on Linux/macOS also skip hidden top-level
  entries (`/tmp/.X11-unix`, …), `systemd-private-*` folders, sockets, pipes and devices;
- files in use are skipped and counted, never forced.

**Deleting** from Explore or Big files moves to the **trash / Recycle Bin by default** (`Del`).
Permanent deletion (`Shift+Del`) has **Cancel** as the default button, except for recognized
build folders. Symbolic links and junctions are removed as links: **their target is never
followed**. Moving to the trash does not free space until the trash is emptied — the app says so
and the trash is an entry in the Cleanup tab.

## What gets cleaned, per platform

Only entries that exist on your computer are shown. Sizes are measured in the background.

| Group | Windows | Linux | macOS |
|---|---|---|---|
| Safe | Recycle Bin · `%TEMP%` · `Windows\Temp` · Windows Update downloads · Delivery Optimization cache · Windows Error Reporting · crash dumps · NVIDIA installers | Trash (`~/.local/share/Trash`) · `/tmp` · thumbnails | Trash (`~/.Trash`) · `$TMPDIR` · `~/Library/Logs` |
| Caches | npm · Yarn · pip · Chrome · Edge · VS Code | npm · Yarn · pip · Go · Chrome · Chromium · Firefox · VS Code | `~/Library/Caches` · Xcode DerivedData · iOS DeviceSupport · simulator caches · npm |
| Caches (all) | cargo registry · NuGet · Gradle · Maven | ← same | ← same |
| Caution | Hugging Face models | ← same | ← same · Xcode archives |
| Manual | Google Drive cache · Docker · Windows.old · Ollama · LM Studio · Downloads | Docker (`/var/lib/docker`) · Ollama · LM Studio · Downloads | iPhone/iPad backups · Docker · Ollama · LM Studio · Downloads |

Some Windows entries (`Windows\Temp`, Windows Update, WER) need the app to be **run as
administrator**; otherwise they are measured partially (shown as `≥`) and cleaned as far as
permissions allow.

## Command-line mode

The same engine and the same protections, without the interface — useful for scripts.

```sh
# Text report: biggest folders (tree), biggest files, build files, known cleanup entries
disk-space-tui --report C:\                # or: --report /  ·  --report ~/projects
disk-space-tui --report ~ --top 15 --depth 3

# List build artifacts as TSV: <bytes> <TAB> <path> <TAB> <description>
disk-space-tui --artifacts ~/projects > artifacts.tsv

# Permanently delete the paths listed in a file (one per line); protected paths are refused
cut -f2 artifacts.tsv > to-delete.txt
disk-space-tui --delete-list to-delete.txt

# Empty the temp folder (files older than 24 h, git repositories left untouched)
disk-space-tui --clean-temp

disk-space-tui --help · --version · --lang it|en
```

`--delete-list` prints one line per path (`OK`, `PARTIAL`, `PROTECTED`, `MISSING`) and a total.
Deletion is **permanent**: review the list first.

## How sizes are measured

- Sizes are **logical file sizes** (what `ls -l` / Explorer "Size" show), summed per folder.
- Files with several **hard links** are counted once per link: the pnpm store, `WinSxS` and some
  backup tools therefore look bigger than the space they really use. `node_modules` installed by
  pnpm are flagged in their description.
- **Symbolic links and junctions are not followed**, so nothing is counted twice.
- On Linux/macOS the scan **stays on the starting file system** (like `du -x`): `/proc`, `/sys` and
  other mounted drives are skipped when you scan `/`.
- Folders that cannot be read are marked *unreadable* and counted in the summary.

## Platform notes and troubleshooting

- **Windows**: run from Windows Terminal for the best result. To clean `Windows\Temp`,
  Windows Update and WER, start the terminal *as administrator*. The Recycle Bin is emptied with
  PowerShell's `Clear-RecycleBin`.
- **Linux**: "Copy the path" needs `wl-copy` (Wayland), `xclip` or `xsel`; "Show in file manager"
  uses `xdg-open`. Use `sudo` only if you really need to scan system folders.
- **macOS**: emptying `~/.Trash` and reading some folders requires **Full Disk Access** for your
  terminal (System Settings → Privacy & Security → Full Disk Access).
- **The mouse does not work**: your terminal must support mouse reporting; the keyboard always works.
- **Box characters look wrong**: use a font with box-drawing characters (Cascadia, JetBrains Mono,
  Fira Code, Menlo…) and a UTF-8 terminal.
- **The scan is slow on a network drive**: that is the network, not the app — scan a subfolder.

## Building from source

```sh
git clone https://github.com/danielegabrovec/disk-space-tui
cd disk-space-tui
cargo build --release          # binary in target/release/
cargo test                     # unit and UI tests (the UI is rendered to an in-memory terminal)
cargo clippy --all-targets -- -D warnings
```

Print every screen as text, without a real terminal (handy to review UI changes):

```sh
DUMP_ROOT=/some/folder DUMP_LANG=en cargo test --release dump_screens -- --ignored --nocapture
```

## Project layout

```text
src/main.rs      startup, mouse capture, command-line options
src/app.rs       state, keys, mouse, background operations (threads + channel)
src/ui.rs        drawing (ratatui)
src/scan.rs      parallel scan (rayon), tree, big files, build-artifact detection
src/cleanup.rs   cleanup entries per OS, protections, deletion, trash, clipboard
src/assess.rs    "can I delete this?" verdicts for the Details panel and the tags
src/report.rs    text report
src/i18n.rs      Italian/English strings (tr!/trf! macros) and language detection
src/util.rs      size/number formatting
```

Built with [ratatui](https://ratatui.rs), [crossterm](https://github.com/crossterm-rs/crossterm),
[rayon](https://github.com/rayon-rs/rayon), [sysinfo](https://github.com/GuillaumeGomez/sysinfo),
[trash](https://github.com/Byron/trash-rs) and [sys-locale](https://github.com/1Password/sys-locale).

## Contributing

Issues and pull requests are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). Changes that touch
deletion or protections must come with tests.

## License

[MIT](LICENSE) © 2026 Daniele Gabrovec
