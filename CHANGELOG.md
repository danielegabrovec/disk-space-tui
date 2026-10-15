# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [0.2.0] — 2026-10-15

First public release.

### Added
- Interactive terminal interface with three tabs: Explore, Big files, Cleanup.
- Parallel scan of a drive or folder with live progress and estimated percentage.
- Details panel and list tags answering "can I delete this?" (system, regenerable, cleanable,
  caution, git repository).
- Action menu (Space / right-click), full mouse support, search, sorting, clickable path.
- Cleanup grouped by risk, with per-platform entries for Windows, Linux and macOS and
  detection of project build files (`node_modules`, `target`, `.next`, `.turbo`, `bin`/`obj`,
  CMake and Gradle builds, Python caches).
- Protections for system and profile folders on all three platforms, mount points, SSH/GPG keys.
- Automatic cleanups skip git repositories, recent temp files, sockets and hidden system entries.
- Italian and English interface, chosen from the system language (`--lang it|en`).
- Command-line modes: `--report`, `--artifacts`, `--delete-list`, `--clean-temp`.
