//! Scansione parallela del disco in un albero di cartelle con le dimensioni.
//!
//! Per tenere bassa la memoria anche su dischi con milioni di file, ogni cartella
//! conserva tutti i file solo se ne ha pochi; altrimenti tiene i file da 1 MB in su
//! e raggruppa gli altri in un'unica voce "(N file sotto 1 MB)".

use crate::util::fmt_count;
use rayon::prelude::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Dir,
    File,
    /// Gruppo di file piccoli: non si apre e non si elimina.
    Group,
}

#[derive(Debug)]
pub struct Node {
    pub name: String,
    pub size: u64,
    /// Numero di file contenuti (ricorsivo).
    pub files: u64,
    pub kind: Kind,
    /// Cartella non leggibile (accesso negato).
    pub denied: bool,
    /// Ordinati per dimensione decrescente.
    pub children: Vec<Node>,
}

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub cancel: AtomicBool,
    /// Una cartella letta di recente, da mostrare durante l'analisi.
    pub current: Mutex<String>,
}

const KEEP_ALL_FILES: usize = 64;
const MIN_KEPT_FILE: u64 = 1 << 20;

/// Identificativo del disco che contiene un file (solo Linux/macOS; altrove 0).
#[cfg(unix)]
fn dev_of(m: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    m.dev()
}

#[cfg(not(unix))]
fn dev_of(_m: &fs::Metadata) -> u64 {
    0
}

pub fn scan(root: &Path, p: &Progress) -> Node {
    // Come `du -x`: non si esce dal disco di partenza (salta /proc, /sys, dischi montati…).
    let root_dev = fs::metadata(root).map(|m| dev_of(&m)).unwrap_or(0);
    scan_dir(root, root.display().to_string(), root_dev, p)
}

fn file_node((name, size): (String, u64)) -> Node {
    Node {
        name,
        size,
        files: 1,
        kind: Kind::File,
        denied: false,
        children: Vec::new(),
    }
}

fn scan_dir(path: &Path, name: String, root_dev: u64, p: &Progress) -> Node {
    if p.dirs.fetch_add(1, Relaxed).is_multiple_of(128)
        && let Ok(mut cur) = p.current.try_lock()
    {
        *cur = path.display().to_string();
    }
    let mut node = Node {
        name,
        size: 0,
        files: 0,
        kind: Kind::Dir,
        denied: false,
        children: Vec::new(),
    };
    if p.cancel.load(Relaxed) {
        return node;
    }
    let entries = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => {
            node.denied = true;
            p.errors.fetch_add(1, Relaxed);
            return node;
        }
    };
    let mut subdirs = Vec::new();
    let mut files: Vec<(String, u64)> = Vec::new();
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        // Link simbolici e junction: non seguirli (si conterebbe due volte lo stesso spazio).
        if ft.is_symlink() {
            continue;
        }
        let n = e.file_name().to_string_lossy().into_owned();
        if ft.is_dir() {
            if cfg!(unix) && e.metadata().is_ok_and(|m| dev_of(&m) != root_dev) {
                continue;
            }
            subdirs.push((e.path(), n));
        } else {
            files.push((n, e.metadata().map(|m| m.len()).unwrap_or(0)));
        }
    }
    let fbytes: u64 = files.iter().map(|f| f.1).sum();
    p.files.fetch_add(files.len() as u64, Relaxed);
    p.bytes.fetch_add(fbytes, Relaxed);

    let dirs: Vec<Node> = subdirs
        .into_par_iter()
        .map(|(sp, n)| scan_dir(&sp, n, root_dev, p))
        .collect();
    node.size = fbytes + dirs.iter().map(|d| d.size).sum::<u64>();
    node.files = files.len() as u64 + dirs.iter().map(|d| d.files).sum::<u64>();
    node.children = dirs;

    if files.len() <= KEEP_ALL_FILES {
        node.children.extend(files.into_iter().map(file_node));
    } else {
        let (big, small): (Vec<_>, Vec<_>) = files.into_iter().partition(|f| f.1 >= MIN_KEPT_FILE);
        node.children.extend(big.into_iter().map(file_node));
        if !small.is_empty() {
            node.children.push(Node {
                name: crate::trf!(
                    "({} file sotto 1 MB)",
                    "({} files under 1 MB)",
                    fmt_count(small.len() as u64)
                ),
                size: small.iter().map(|f| f.1).sum(),
                files: small.len() as u64,
                kind: Kind::Group,
                denied: false,
                children: Vec::new(),
            });
        }
    }
    node.children.sort_unstable_by_key(|n| Reverse(n.size));
    node
}

// ---------------------------------------------------------------- navigazione

pub fn node_at<'a>(root: &'a Node, path: &[usize]) -> &'a Node {
    let mut n = root;
    for &i in path {
        n = &n.children[i];
    }
    n
}

/// Indici dei nodi che corrispondono ai nomi (confronto senza maiuscole, come Windows).
/// Restituisce il prefisso più lungo trovato.
pub fn resolve_names(root: &Node, names: &[String]) -> Vec<usize> {
    let mut n = root;
    let mut out = Vec::new();
    for name in names {
        let Some(i) = n
            .children
            .iter()
            .position(|c| c.kind != Kind::Group && c.name.eq_ignore_ascii_case(name))
        else {
            break;
        };
        out.push(i);
        n = &n.children[i];
    }
    out
}

/// Nomi dei componenti di `full` relativi alla radice della scansione.
pub fn rel_names(root: &Path, full: &Path) -> Option<Vec<String>> {
    let rel = full.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect(),
    )
}

/// Toglie dall'albero il nodo in `path` e aggiorna le dimensioni dei genitori.
pub fn remove_at(root: &mut Node, path: &[usize]) -> bool {
    let Some((&last, parent_path)) = path.split_last() else {
        return false;
    };
    let removed = {
        let mut n = &mut *root;
        for &i in parent_path {
            match n.children.get_mut(i) {
                Some(c) => n = c,
                None => return false,
            }
        }
        if last >= n.children.len() {
            return false;
        }
        n.children.remove(last)
    };
    let mut n = root;
    n.size = n.size.saturating_sub(removed.size);
    n.files = n.files.saturating_sub(removed.files);
    for &i in parent_path {
        n = &mut n.children[i];
        n.size = n.size.saturating_sub(removed.size);
        n.files = n.files.saturating_sub(removed.files);
    }
    true
}

// ---------------------------------------------------------------- estrazioni

/// I `k` file più grandi dell'albero, dal più grande.
pub fn top_files(root: &Node, root_path: &Path, k: usize) -> Vec<(PathBuf, u64)> {
    fn walk(
        n: &Node,
        path: &mut PathBuf,
        heap: &mut BinaryHeap<Reverse<(u64, PathBuf)>>,
        k: usize,
    ) {
        for c in &n.children {
            match c.kind {
                Kind::File => {
                    if heap.len() < k {
                        heap.push(Reverse((c.size, path.join(&c.name))));
                    } else if heap.peek().is_some_and(|m| c.size > m.0.0) {
                        heap.pop();
                        heap.push(Reverse((c.size, path.join(&c.name))));
                    }
                }
                Kind::Dir => {
                    path.push(&c.name);
                    walk(c, path, heap, k);
                    path.pop();
                }
                Kind::Group => {}
            }
        }
    }
    let mut heap = BinaryHeap::with_capacity(k + 1);
    walk(root, &mut root_path.to_path_buf(), &mut heap, k);
    let mut v: Vec<(PathBuf, u64)> = heap.into_iter().map(|Reverse((s, p))| (p, s)).collect();
    v.sort_unstable_by_key(|f| Reverse(f.1));
    v
}

pub struct DevArtifact {
    pub path: PathBuf,
    pub size: u64,
    pub what: &'static str,
}

/// Cartelle che non si scende mai a cercare artefatti: lì dentro `node_modules`
/// e simili appartengono a programmi installati, non a progetti.
/// Contano anche le cartelle-punto del profilo: le estensioni di VS Code/Cursor hanno
/// `package.json` + `node_modules` ma sono programmi installati, non progetti.
const SKIP_DIRS: &[&str] = &[
    // Windows
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "appdata",
    "$recycle.bin",
    "msys64",
    // macOS
    "library",
    "applications",
    // profilo: strumenti ed editor installati
    ".cargo",
    ".rustup",
    ".vscode",
    ".vscode-insiders",
    ".vscode-server",
    ".cursor",
    ".cursor-server",
    ".windsurf",
    ".antigravity-ide",
    ".lmstudio",
    ".ollama",
    ".cache",
    ".config",
    ".local",
    ".npm",
    ".nvm",
    ".pyenv",
    ".sdkman",
    ".nuget",
    ".gradle",
    ".m2",
    ".android",
    ".trash",
    "snap",
    "qt",
];

/// Alberi di sistema di Linux/macOS dove non si cercano artefatti.
const SKIP_ABS: &[&str] = &[
    "/usr", "/opt", "/var", "/etc", "/proc", "/sys", "/dev", "/run", "/boot", "/snap", "/nix",
    "/lib", "/bin", "/sbin", "/System", "/private",
];

/// La cartella contiene almeno un file con una di queste estensioni (es. un .csproj)?
fn has_ext(dir: &Path, exts: &[&str]) -> bool {
    fs::read_dir(dir).is_ok_and(|rd| {
        rd.flatten().any(|e| {
            Path::new(&e.file_name())
                .extension()
                .is_some_and(|x| exts.iter().any(|w| x.eq_ignore_ascii_case(w)))
        })
    })
}

/// Se la cartella `name` dentro `parent` è un artefatto di compilazione, dice che cos'è.
/// Si riconosce solo con il file di progetto accanto (package.json, Cargo.toml, .csproj…).
pub fn artifact_kind(parent: &Path, name: &str) -> Option<&'static str> {
    use crate::tr;
    let lname = name.to_ascii_lowercase();
    let has = |file: &str| parent.join(file).is_file();
    match lname.as_str() {
        "node_modules"
            if has("package.json") && parent.join(name).join(".modules.yaml").is_file() =>
        {
            Some(tr!(
                "Dipendenze pnpm: `pnpm install` le rimette. Sono collegamenti allo store, lo spazio liberato reale può essere molto minore",
                "pnpm dependencies: `pnpm install` restores them. They are links into the store, the space actually freed may be much smaller"
            ))
        }
        "node_modules" if has("package.json") => Some(tr!(
            "Dipendenze Node: `npm install` le rimette",
            "Node dependencies: `npm install` restores them"
        )),
        "target" if has("Cargo.toml") => Some(tr!(
            "Compilati Rust: `cargo build` li rifà",
            "Rust build output: `cargo build` recreates it"
        )),
        ".next" if has("package.json") => Some(tr!(
            "Build di Next.js: si rifà con `npm run build`/`dev`",
            "Next.js build: recreated by `npm run build`/`dev`"
        )),
        "__pycache__" | ".pytest_cache" | ".mypy_cache" | ".ruff_cache" => {
            Some(tr!("Cache Python", "Python cache"))
        }
        ".turbo" | ".parcel-cache" | ".vite" => {
            Some(tr!("Cache di build JavaScript", "JavaScript build cache"))
        }
        "bin" | "obj" if has_ext(parent, &["csproj", "fsproj", "vbproj", "vcxproj"]) => Some(tr!(
            "Compilati .NET / Visual Studio: `dotnet build` o Compila li rifà",
            ".NET / Visual Studio build output: `dotnet build` or Build recreates it"
        )),
        "debug" | "release" | "x64" | "x86" | "arm64"
            if has_ext(parent, &["sln", "slnx", "vcxproj"]) =>
        {
            Some(tr!(
                "Compilati C++ di Visual Studio: Compila li rifà",
                "Visual Studio C++ build output: Build recreates it"
            ))
        }
        ".vs" if has_ext(parent, &["sln", "slnx"]) => Some(tr!(
            "Cache di Visual Studio (IntelliSense): si ricrea all'apertura",
            "Visual Studio cache (IntelliSense): recreated when the solution is opened"
        )),
        ".gradle" | "build" if has("build.gradle") || has("build.gradle.kts") => Some(tr!(
            "Build Gradle: `gradle build` la rifà",
            "Gradle build: `gradle build` recreates it"
        )),
        "deriveddata" if parent.ends_with("Xcode") => {
            Some(tr!("Compilati di Xcode", "Xcode build products"))
        }
        n if (n == "build" || n == "out" || n.starts_with("cmake-build"))
            && parent.join(name).join("CMakeCache.txt").is_file() =>
        {
            Some(tr!(
                "Cartella di build CMake: si rigenera con cmake",
                "CMake build folder: regenerated by cmake"
            ))
        }
        _ => None,
    }
}

/// Le cartelle dove non si cercano artefatti (appartengono a programmi installati).
pub fn is_skip_dir(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    SKIP_DIRS.contains(&l.as_str()) || l.ends_with(".app")
}

/// Come `is_skip_dir`, guardando anche il percorso completo (alberi di sistema di Linux/macOS).
pub fn is_skip_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if is_skip_dir(&name) {
        return true;
    }
    if cfg!(unix) {
        let s = path.to_string_lossy();
        return SKIP_ABS
            .iter()
            .any(|a| s == *a || s.starts_with(&format!("{a}/")));
    }
    false
}
/// Artefatti di sviluppo rigenerabili (node_modules, target di Rust, .next, cache).
pub fn dev_artifacts(root: &Node, root_path: &Path) -> Vec<DevArtifact> {
    fn walk(n: &Node, path: &mut PathBuf, out: &mut Vec<DevArtifact>) {
        for c in &n.children {
            if c.kind != Kind::Dir {
                continue;
            }
            path.push(&c.name);
            if is_skip_path(path) {
                path.pop();
                continue;
            }
            let what = path.parent().and_then(|p| artifact_kind(p, &c.name));
            match what {
                Some(what) => {
                    if c.size >= MIN_KEPT_FILE {
                        out.push(DevArtifact {
                            path: path.clone(),
                            size: c.size,
                            what,
                        });
                    }
                }
                None => walk(c, path, out),
            }
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(root, &mut root_path.to_path_buf(), &mut out);
    out.sort_unstable_by_key(|a| Reverse(a.size));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str, children: Vec<Node>) -> Node {
        let mut children = children;
        children.sort_unstable_by_key(|n| Reverse(n.size));
        Node {
            name: name.into(),
            size: children.iter().map(|c| c.size).sum(),
            files: children.iter().map(|c| c.files).sum(),
            kind: Kind::Dir,
            denied: false,
            children,
        }
    }
    fn file(name: &str, size: u64) -> Node {
        file_node((name.into(), size))
    }

    #[test]
    fn remove_updates_parents() {
        let mut root = dir(
            "R",
            vec![dir("a", vec![file("x", 100), file("y", 50)]), file("z", 10)],
        );
        assert_eq!(root.size, 160);
        let path = resolve_names(&root, &["a".into(), "Y".into()]);
        assert_eq!(path.len(), 2);
        assert!(remove_at(&mut root, &path));
        assert_eq!(root.size, 110);
        assert_eq!(root.files, 2);
        assert_eq!(root.children[0].size, 100);
    }

    #[test]
    fn top_files_sorted() {
        let root = dir(
            "R",
            vec![dir("a", vec![file("x", 100), file("y", 50)]), file("z", 70)],
        );
        let top = top_files(&root, Path::new("R"), 2);
        assert_eq!(top.iter().map(|t| t.1).collect::<Vec<_>>(), vec![100, 70]);
        assert_eq!(top[0].0, Path::new("R").join("a").join("x"));
    }

    #[test]
    fn scan_real_dir() {
        let base = std::env::temp_dir().join(format!("spazio-disco-test-{}", std::process::id()));
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("a.bin"), vec![0u8; 3000]).unwrap();
        fs::write(base.join("sub").join("b.bin"), vec![0u8; 1000]).unwrap();
        let tree = scan(&base, &Progress::default());
        fs::remove_dir_all(&base).unwrap();
        assert_eq!(tree.size, 4000);
        assert_eq!(tree.files, 2);
        assert_eq!(tree.children[0].name, "a.bin");
    }
}
