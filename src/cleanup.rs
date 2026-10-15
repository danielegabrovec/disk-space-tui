//! Voci di pulizia (temporanei, cache, cestino…), protezioni ed eliminazione.
//! Windows, Linux e macOS: le differenze sono tutte qui dentro.

use crate::{tr, trf};
use ratatui::style::Color;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Safety {
    /// Si può cancellare senza conseguenze.
    Sicuro,
    /// Si ricrea da solo (cache, dipendenze): costa solo un nuovo download/build.
    Rigenerabile,
    /// Si può cancellare ma si perde qualcosa di scaricato (es. modelli).
    Attenzione,
    /// Va gestito a mano o dall'app che lo possiede.
    Manuale,
}

impl Safety {
    pub fn label(self) -> &'static str {
        match self {
            Safety::Sicuro => tr!("SICURO", "SAFE"),
            Safety::Rigenerabile => tr!("RIGENERABILE", "REGENERABLE"),
            Safety::Attenzione => tr!("ATTENZIONE", "CAUTION"),
            Safety::Manuale => tr!("A MANO", "MANUAL"),
        }
    }
    pub fn color(self) -> Color {
        match self {
            Safety::Sicuro => Color::Green,
            Safety::Rigenerabile => Color::Cyan,
            Safety::Attenzione => Color::Yellow,
            Safety::Manuale => Color::DarkGray,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    /// Svuota la cartella lasciandola al suo posto. Con `min_age` tocca solo i file più vecchi;
    /// con `skip_hidden` lascia le voci nascoste di primo livello (es. `/tmp/.X11-unix`).
    ClearContents {
        min_age: Option<Duration>,
        skip_hidden: bool,
    },
    /// Cestino di Windows (via PowerShell). Su Linux/macOS il cestino è una cartella normale.
    #[cfg_attr(not(windows), allow(dead_code))]
    EmptyRecycleBin,
    /// Elimina la cartella intera (artefatti di sviluppo).
    DeleteDir,
    None,
}

impl Action {
    pub fn clear() -> Action {
        Action::ClearContents {
            min_age: None,
            skip_hidden: false,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Action::ClearContents { min_age: Some(_), .. } => tr!(
                "Svuota la cartella: solo file più vecchi di 24 ore, quelli in uso vengono saltati",
                "Empties the folder: only files older than 24 hours, files in use are skipped"
            )
            .into(),
            Action::ClearContents { min_age: None, .. } => tr!(
                "Svuota la cartella (la cartella resta); i file in uso vengono saltati",
                "Empties the folder (the folder stays); files in use are skipped"
            )
            .into(),
            Action::EmptyRecycleBin => trf!("Svuota il {} (definitivo)", "Empties the {} (permanent)", crate::i18n::trash_name()),
            Action::DeleteDir => tr!(
                "Elimina la cartella definitivamente: si ricrea con il comando di build/installazione",
                "Deletes the folder permanently: it is recreated by the build/install command"
            )
            .into(),
            Action::None => trf!(
                "Da gestire a mano: N = analizzala, O = aprila in {}",
                "Handle it yourself: N = analyze it, O = open it in {}",
                crate::i18n::file_manager()
            ),
        }
    }
}

pub struct Target {
    pub id: usize,
    pub name: String,
    pub detail: String,
    pub path: PathBuf,
    pub safety: Safety,
    pub action: Action,
    pub size: Option<u64>,
    /// Misura incompleta (alcune sottocartelle non leggibili).
    pub partial: bool,
    pub selected: bool,
    /// Trovata dalla scansione (artefatti di sviluppo) invece che dalla lista fissa.
    pub from_scan: bool,
}

impl Target {
    pub fn actionable(&self) -> bool {
        !matches!(self.action, Action::None)
    }
}

fn env_path(k: &str) -> Option<PathBuf> {
    std::env::var_os(k)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        env_path("USERPROFILE").or_else(|| env_path("HOME"))
    } else {
        env_path("HOME")
    }
}

#[cfg(windows)]
fn system_drive() -> String {
    std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
}

// ---------------------------------------------------------------- voci di pulizia

struct Def {
    name: &'static str,
    detail: &'static str,
    path: Option<PathBuf>,
    safety: Safety,
    action: Action,
}

fn def(
    name: &'static str,
    detail: &'static str,
    path: Option<PathBuf>,
    safety: Safety,
    action: Action,
) -> Def {
    Def {
        name,
        detail,
        path,
        safety,
        action,
    }
}

fn join(base: &Option<PathBuf>, rel: &str) -> Option<PathBuf> {
    base.as_ref().map(|b| {
        rel.split(['/', '\\'])
            .fold(b.clone(), |p, part| p.join(part))
    })
}

/// Voci comuni a tutti i sistemi (cache degli strumenti di sviluppo, modelli AI…).
fn common_defs(home: &Option<PathBuf>) -> Vec<Def> {
    use Safety::*;
    vec![
        def(
            tr!(
                "Cache cargo (crate scaricati)",
                "Cargo cache (downloaded crates)"
            ),
            "~/.cargo/registry/cache",
            join(home, ".cargo/registry/cache"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Sorgenti crate estratti", "Extracted crate sources"),
            "~/.cargo/registry/src",
            join(home, ".cargo/registry/src"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Pacchetti NuGet", "NuGet packages"),
            tr!(
                "~/.nuget/packages: si riscaricano al prossimo restore",
                "~/.nuget/packages: re-downloaded on the next restore"
            ),
            join(home, ".nuget/packages"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache Gradle", "Gradle cache"),
            "~/.gradle/caches",
            join(home, ".gradle/caches"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Repository Maven locale", "Local Maven repository"),
            "~/.m2/repository",
            join(home, ".m2/repository"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Modelli Hugging Face", "Hugging Face models"),
            tr!(
                "Modelli AI scaricati: si riscaricano, ma pesano",
                "Downloaded AI models: they can be re-downloaded, but they are big"
            ),
            join(home, ".cache/huggingface"),
            Attenzione,
            Action::clear(),
        ),
        def(
            tr!("Modelli Ollama", "Ollama models"),
            tr!(
                "Rimuovili con `ollama list` / `ollama rm <nome>`, non a mano",
                "Remove them with `ollama list` / `ollama rm <name>`, not by hand"
            ),
            join(home, ".ollama/models"),
            Manuale,
            Action::None,
        ),
        def(
            tr!("Modelli LM Studio", "LM Studio models"),
            tr!(
                "Rimuovili dall'app LM Studio (My Models)",
                "Remove them from the LM Studio app (My Models)"
            ),
            join(home, ".lmstudio/models"),
            Manuale,
            Action::None,
        ),
        def(
            tr!("Download", "Downloads"),
            tr!(
                "Controlla a mano: spesso ci sono installer e archivi vecchi",
                "Check by hand: there are often old installers and archives"
            ),
            join(home, "Downloads"),
            Manuale,
            Action::None,
        ),
    ]
}

#[cfg(windows)]
fn os_defs(_home: &Option<PathBuf>) -> Vec<Def> {
    use Safety::*;
    let local = env_path("LOCALAPPDATA");
    let roaming = env_path("APPDATA");
    let win = env_path("WINDIR");
    let sys = Some(PathBuf::from(format!("{}\\", system_drive())));
    let programdata = env_path("ProgramData");
    let day = Some(Duration::from_secs(24 * 3600));
    vec![
        def(
            tr!("Cestino", "Recycle Bin"),
            tr!(
                "File eliminati in attesa: finché non lo svuoti lo spazio resta occupato",
                "Deleted files waiting: the space stays used until you empty it"
            ),
            join(&sys, "$Recycle.Bin"),
            Sicuro,
            Action::EmptyRecycleBin,
        ),
        def(
            tr!("File temporanei (utente)", "Temporary files (user)"),
            tr!(
                "%TEMP%: file lasciati da installazioni e programmi",
                "%TEMP%: files left by installers and programs"
            ),
            Some(std::env::temp_dir()),
            Sicuro,
            Action::ClearContents {
                min_age: day,
                skip_hidden: false,
            },
        ),
        def(
            tr!("File temporanei (Windows)", "Temporary files (Windows)"),
            tr!(
                "C:\\Windows\\Temp — serve avviare come amministratore",
                "C:\\Windows\\Temp — requires running as administrator"
            ),
            join(&win, "Temp"),
            Sicuro,
            Action::ClearContents {
                min_age: day,
                skip_hidden: false,
            },
        ),
        def(
            tr!("Download di Windows Update", "Windows Update downloads"),
            tr!(
                "Aggiornamenti già installati — serve amministratore",
                "Updates already installed — requires administrator"
            ),
            join(&win, "SoftwareDistribution\\Download"),
            Sicuro,
            Action::clear(),
        ),
        def(
            tr!(
                "Cache Ottimizzazione recapito",
                "Delivery Optimization cache"
            ),
            tr!(
                "Pezzi di aggiornamenti condivisi — serve amministratore",
                "Shared update pieces — requires administrator"
            ),
            join(
                &win,
                "ServiceProfiles\\NetworkService\\AppData\\Local\\Microsoft\\Windows\\DeliveryOptimization\\Cache",
            ),
            Sicuro,
            Action::clear(),
        ),
        def(
            tr!("Segnalazioni errori di Windows", "Windows Error Reporting"),
            tr!(
                "Report di crash (WER) — serve amministratore",
                "Crash reports (WER) — requires administrator"
            ),
            join(&programdata, "Microsoft\\Windows\\WER"),
            Sicuro,
            Action::clear(),
        ),
        def(
            tr!("Dump dei crash dei programmi", "Program crash dumps"),
            "%LOCALAPPDATA%\\CrashDumps",
            join(&local, "CrashDumps"),
            Sicuro,
            Action::clear(),
        ),
        def(
            tr!(
                "Installer driver NVIDIA scaricati",
                "Downloaded NVIDIA driver installers"
            ),
            tr!("Pacchetti già installati", "Packages already installed"),
            join(&programdata, "NVIDIA Corporation\\Downloader"),
            Sicuro,
            Action::clear(),
        ),
        def(
            tr!("Cache npm", "npm cache"),
            tr!(
                "Pacchetti scaricati da npm: si riscaricano quando servono",
                "Packages downloaded by npm: re-downloaded when needed"
            ),
            join(&local, "npm-cache\\_cacache"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache Yarn", "Yarn cache"),
            "%LOCALAPPDATA%\\Yarn\\Cache",
            join(&local, "Yarn\\Cache"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache pip", "pip cache"),
            "%LOCALAPPDATA%\\pip\\cache",
            join(&local, "pip\\cache"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache Chrome", "Chrome cache"),
            tr!(
                "Chiudi Chrome prima di pulire",
                "Close Chrome before cleaning"
            ),
            join(&local, "Google\\Chrome\\User Data\\Default\\Cache"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache Edge", "Edge cache"),
            tr!("Chiudi Edge prima di pulire", "Close Edge before cleaning"),
            join(&local, "Microsoft\\Edge\\User Data\\Default\\Cache"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache VS Code", "VS Code cache"),
            tr!(
                "Dati in cache di VS Code: chiudilo prima",
                "VS Code cached data: close it first"
            ),
            join(&roaming, "Code\\CachedData"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!(
                "Estensioni VS Code scaricate",
                "Downloaded VS Code extensions"
            ),
            tr!(
                "Installer .vsix già installati",
                ".vsix installers already installed"
            ),
            join(&roaming, "Code\\CachedExtensionVSIXs"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache Google Drive", "Google Drive cache"),
            tr!(
                "Riducila dall'app Drive: Impostazioni > Cartella cache",
                "Shrink it from the Drive app: Settings > Cache folder"
            ),
            join(&local, "Google\\DriveFS"),
            Manuale,
            Action::None,
        ),
        def(
            "Docker",
            tr!(
                "Usa `docker system prune` o Docker Desktop > Clean up",
                "Use `docker system prune` or Docker Desktop > Clean up"
            ),
            join(&local, "Docker"),
            Manuale,
            Action::None,
        ),
        def(
            tr!(
                "Vecchia installazione di Windows",
                "Previous Windows installation"
            ),
            tr!(
                "Windows.old: rimuovila da Impostazioni > Sistema > Archiviazione",
                "Windows.old: remove it from Settings > System > Storage"
            ),
            join(&sys, "Windows.old"),
            Manuale,
            Action::None,
        ),
    ]
}

#[cfg(target_os = "linux")]
fn os_defs(home: &Option<PathBuf>) -> Vec<Def> {
    use Safety::*;
    let day = Some(Duration::from_secs(24 * 3600));
    vec![
        def(
            tr!("Cestino", "Trash"),
            tr!(
                "~/.local/share/Trash: finché non lo svuoti lo spazio resta occupato",
                "~/.local/share/Trash: the space stays used until you empty it"
            ),
            join(home, ".local/share/Trash"),
            Sicuro,
            Action::clear(),
        ),
        def(
            tr!("File temporanei", "Temporary files"),
            tr!(
                "/tmp: solo file più vecchi di 24 ore, senza toccare socket e voci di sistema",
                "/tmp: only files older than 24 hours, leaving sockets and system entries alone"
            ),
            Some(std::env::temp_dir()),
            Sicuro,
            Action::ClearContents {
                min_age: day,
                skip_hidden: true,
            },
        ),
        def(
            tr!("Miniature", "Thumbnails"),
            "~/.cache/thumbnails",
            join(home, ".cache/thumbnails"),
            Sicuro,
            Action::clear(),
        ),
        def(
            tr!("Cache npm", "npm cache"),
            tr!(
                "Pacchetti scaricati da npm: si riscaricano quando servono",
                "Packages downloaded by npm: re-downloaded when needed"
            ),
            join(home, ".npm/_cacache"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache Yarn", "Yarn cache"),
            "~/.cache/yarn",
            join(home, ".cache/yarn"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache pip", "pip cache"),
            "~/.cache/pip",
            join(home, ".cache/pip"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache di Go", "Go build cache"),
            "~/.cache/go-build",
            join(home, ".cache/go-build"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache Chrome", "Chrome cache"),
            tr!(
                "Chiudi Chrome prima di pulire",
                "Close Chrome before cleaning"
            ),
            join(home, ".cache/google-chrome"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache Chromium", "Chromium cache"),
            tr!(
                "Chiudi Chromium prima di pulire",
                "Close Chromium before cleaning"
            ),
            join(home, ".cache/chromium"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache Firefox", "Firefox cache"),
            tr!(
                "Chiudi Firefox prima di pulire",
                "Close Firefox before cleaning"
            ),
            join(home, ".cache/mozilla"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache VS Code", "VS Code cache"),
            tr!(
                "Dati in cache di VS Code: chiudilo prima",
                "VS Code cached data: close it first"
            ),
            join(home, ".config/Code/CachedData"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            "Docker",
            tr!(
                "Usa `docker system prune` (i dati sono in /var/lib/docker)",
                "Use `docker system prune` (data lives in /var/lib/docker)"
            ),
            Some(PathBuf::from("/var/lib/docker")),
            Manuale,
            Action::None,
        ),
    ]
}

#[cfg(target_os = "macos")]
fn os_defs(home: &Option<PathBuf>) -> Vec<Def> {
    use Safety::*;
    let day = Some(Duration::from_secs(24 * 3600));
    vec![
        def(
            tr!("Cestino", "Trash"),
            tr!(
                "~/.Trash: finché non lo svuoti lo spazio resta occupato (serve l'accesso completo al disco per il Terminale)",
                "~/.Trash: the space stays used until you empty it (Terminal needs Full Disk Access)"
            ),
            join(home, ".Trash"),
            Sicuro,
            Action::clear(),
        ),
        def(
            tr!("File temporanei", "Temporary files"),
            tr!(
                "$TMPDIR: solo file più vecchi di 24 ore",
                "$TMPDIR: only files older than 24 hours"
            ),
            Some(std::env::temp_dir()),
            Sicuro,
            Action::ClearContents {
                min_age: day,
                skip_hidden: true,
            },
        ),
        def(
            tr!("Log delle app", "App logs"),
            "~/Library/Logs",
            join(home, "Library/Logs"),
            Sicuro,
            Action::clear(),
        ),
        def(
            tr!("Cache delle app", "App caches"),
            tr!(
                "~/Library/Caches: si ricrea da sola; chiudi le app prima di pulire",
                "~/Library/Caches: recreated automatically; close apps before cleaning"
            ),
            join(home, "Library/Caches"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            "Xcode DerivedData",
            tr!(
                "Compilati di Xcode: si rifanno alla prossima build",
                "Xcode build products: rebuilt on the next build"
            ),
            join(home, "Library/Developer/Xcode/DerivedData"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            "Xcode iOS DeviceSupport",
            tr!(
                "Simboli dei dispositivi: si riscaricano collegandoli",
                "Device symbols: re-downloaded when you connect the device"
            ),
            join(home, "Library/Developer/Xcode/iOS DeviceSupport"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Cache dei simulatori", "Simulator caches"),
            "~/Library/Developer/CoreSimulator/Caches",
            join(home, "Library/Developer/CoreSimulator/Caches"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Archivi di Xcode", "Xcode archives"),
            tr!(
                "Build archiviate per la pubblicazione: servono a simbolicare i crash",
                "Archived release builds: needed to symbolicate crashes"
            ),
            join(home, "Library/Developer/Xcode/Archives"),
            Attenzione,
            Action::clear(),
        ),
        def(
            tr!("Cache npm", "npm cache"),
            tr!(
                "Pacchetti scaricati da npm: si riscaricano quando servono",
                "Packages downloaded by npm: re-downloaded when needed"
            ),
            join(home, ".npm/_cacache"),
            Rigenerabile,
            Action::clear(),
        ),
        def(
            tr!("Backup di iPhone/iPad", "iPhone/iPad backups"),
            tr!(
                "Gestiscili dal Finder (dispositivo > Gestisci backup)",
                "Manage them from Finder (device > Manage Backups)"
            ),
            join(home, "Library/Application Support/MobileSync/Backup"),
            Manuale,
            Action::None,
        ),
        def(
            "Docker",
            tr!(
                "Usa `docker system prune` o Docker Desktop > Clean up",
                "Use `docker system prune` or Docker Desktop > Clean up"
            ),
            join(home, "Library/Containers/com.docker.docker"),
            Manuale,
            Action::None,
        ),
    ]
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn os_defs(_home: &Option<PathBuf>) -> Vec<Def> {
    Vec::new()
}

/// Le voci di pulizia note che esistono su questo computer, nell'ordine in cui compaiono.
pub fn fixed_targets() -> Vec<Target> {
    let home = home_dir();
    let mut defs = os_defs(&home);
    defs.extend(common_defs(&home));
    let mut seen: Vec<PathBuf> = Vec::new();
    defs.into_iter()
        .filter_map(|d| {
            let p = d.path?;
            if !p.exists() || seen.contains(&p) {
                return None;
            }
            seen.push(p.clone());
            Some((d.name, d.detail, p, d.safety, d.action))
        })
        .enumerate()
        .map(|(id, (name, detail, path, safety, action))| Target {
            id,
            name: name.into(),
            detail: detail.into(),
            path,
            safety,
            action,
            size: None,
            partial: false,
            selected: false,
            from_scan: false,
        })
        .collect()
}

// ---------------------------------------------------------------- misura

/// Dimensione di una cartella (parallela). Il bool dice se qualcosa non era leggibile.
pub fn dir_size(p: &Path) -> (u64, bool) {
    fn rec(p: &Path, partial: &AtomicBool) -> u64 {
        let Ok(rd) = fs::read_dir(p) else {
            partial.store(true, Relaxed);
            return 0;
        };
        let mut total = 0;
        let mut dirs = Vec::new();
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                dirs.push(e.path());
            } else {
                total += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        total + dirs.par_iter().map(|d| rec(d, partial)).sum::<u64>()
    }
    if p.is_file() {
        return (fs::metadata(p).map(|m| m.len()).unwrap_or(0), false);
    }
    let partial = AtomicBool::new(false);
    let s = rec(p, &partial);
    (s, partial.into_inner())
}

// ---------------------------------------------------------------- protezioni

/// Forma confrontabile di un percorso: su Windows minuscolo (il file system ignora le
/// maiuscole), senza separatore finale.
fn norm(p: &Path) -> String {
    let s = p.to_string_lossy();
    let s = if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.into_owned()
    };
    let t = s.trim_end_matches(['\\', '/']);
    if t.is_empty() { s } else { t.to_string() }
}

fn sep() -> char {
    if cfg!(windows) { '\\' } else { '/' }
}

/// `s` è `base` o sta dentro `base`.
fn within(s: &str, base: &str) -> bool {
    s == base || s.starts_with(&format!("{base}{}", sep()))
}

/// Cartelle del profilo che non si eliminano mai in blocco.
fn home_crit(home: &str) -> Vec<String> {
    let s = sep();
    let subs: &[&str] = if cfg!(windows) {
        &[
            "",
            "appdata",
            "appdata\\local",
            "appdata\\roaming",
            "documents",
            "desktop",
            "downloads",
            "pictures",
            "music",
            "videos",
            "onedrive",
            ".ssh",
            ".gnupg",
        ]
    } else {
        &[
            "",
            "Desktop",
            "Documents",
            "Downloads",
            "Pictures",
            "Music",
            "Videos",
            "Movies",
            "Public",
            "Library",
            ".config",
            ".local",
            ".local/share",
            ".ssh",
            ".gnupg",
        ]
    };
    subs.iter()
        .map(|sub| {
            if sub.is_empty() {
                home.to_string()
            } else {
                format!("{home}{s}{sub}")
            }
        })
        .collect()
}

/// Motivo per cui un percorso NON si può eliminare da qui (cartelle di sistema e simili).
pub fn protection(p: &Path) -> Option<&'static str> {
    if p.parent().is_none() || p.file_name().is_none() {
        return Some(tr!("è la radice di un disco", "it is the root of a drive"));
    }
    let s = norm(p);
    let home = home_dir().map(|h| norm(&h));
    if let Some(h) = &home {
        // Chiavi SSH/GPG: mai, nemmeno dentro.
        for secret in [".ssh", ".gnupg"] {
            if within(&s, &format!("{h}{}{secret}", sep())) {
                return Some(tr!(
                    "contiene chiavi private (SSH/GPG)",
                    "it contains private keys (SSH/GPG)"
                ));
            }
        }
    }
    let mut crit = os_crit();
    if let Some(h) = &home {
        crit.extend(home_crit(h));
    }
    // Il percorso è una cartella critica, oppure la contiene.
    if crit
        .iter()
        .any(|c| *c == s || c.starts_with(&format!("{s}{}", sep())))
    {
        return Some(tr!(
            "è una cartella di sistema o del profilo utente",
            "it is a system or user-profile folder"
        ));
    }
    os_protection(p, &s)
}

#[cfg(windows)]
fn os_crit() -> Vec<String> {
    let sys = system_drive().to_lowercase();
    let win = env_path("WINDIR")
        .map(|w| norm(&w))
        .unwrap_or_else(|| format!("{sys}\\windows"));
    vec![
        win,
        format!("{sys}\\program files"),
        format!("{sys}\\program files (x86)"),
        format!("{sys}\\programdata"),
        format!("{sys}\\users"),
        format!("{sys}\\system volume information"),
        format!("{sys}\\$recycle.bin"),
        format!("{sys}\\recovery"),
        format!("{sys}\\boot"),
    ]
}

#[cfg(windows)]
fn os_protection(p: &Path, s: &str) -> Option<&'static str> {
    let sys = system_drive().to_lowercase();
    let win = env_path("WINDIR")
        .map(|w| norm(&w))
        .unwrap_or_else(|| format!("{sys}\\windows"));
    let allowed = [
        format!("{win}\\temp"),
        format!("{win}\\softwaredistribution\\download"),
    ];
    if s.starts_with(&format!("{win}\\")) && !allowed.iter().any(|a| within(s, a)) {
        return Some(tr!(
            "è dentro Windows: usa Impostazioni > Sistema > Archiviazione",
            "it is inside Windows: use Settings > System > Storage"
        ));
    }
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if [
        "pagefile.sys",
        "hiberfil.sys",
        "swapfile.sys",
        "dumpstack.log.tmp",
    ]
    .contains(&name.as_str())
    {
        return Some(tr!(
            "è un file di sistema gestito da Windows (memoria virtuale / ibernazione)",
            "it is a system file managed by Windows (virtual memory / hibernation)"
        ));
    }
    None
}

/// Alberi di sistema di Linux e macOS: protetti loro e tutto ciò che contengono.
#[cfg(unix)]
const UNIX_SYSTEM_TREES: &[&str] = &[
    "/bin", "/boot", "/dev", "/etc", "/lib", "/lib32", "/lib64", "/libx32", "/proc", "/run",
    "/sbin", "/snap", "/srv", "/sys", "/usr", "/var", "/nix", "/System", "/Library", "/private",
    "/cores",
];

/// Dentro gli alberi di sistema, queste si possono pulire.
#[cfg(unix)]
const UNIX_SYSTEM_EXCEPT: &[&str] = &[
    "/var/tmp",
    "/private/tmp",
    "/private/var/tmp",
    "/var/folders",
    "/private/var/folders",
];

#[cfg(unix)]
fn os_crit() -> Vec<String> {
    let mut v: Vec<String> = UNIX_SYSTEM_TREES.iter().map(|s| s.to_string()).collect();
    // Radici che contengono dati di tutti: protette solo loro stesse.
    v.extend(
        [
            "/home",
            "/Users",
            "/root",
            "/mnt",
            "/media",
            "/Volumes",
            "/opt",
            "/Applications",
            "/tmp",
        ]
        .map(String::from),
    );
    v
}

#[cfg(unix)]
fn os_protection(p: &Path, s: &str) -> Option<&'static str> {
    use std::os::unix::fs::MetadataExt;
    if UNIX_SYSTEM_TREES.iter().any(|t| within(s, t))
        && !UNIX_SYSTEM_EXCEPT.iter().any(|e| within(s, e))
    {
        return Some(tr!("è una cartella di sistema", "it is a system folder"));
    }
    // Un disco montato non si elimina in blocco.
    if let (Ok(m), Some(parent)) = (fs::symlink_metadata(p), p.parent())
        && let Ok(pm) = fs::metadata(parent)
        && m.is_dir()
        && m.dev() != pm.dev()
    {
        return Some(tr!(
            "è il punto di montaggio di un altro disco",
            "it is the mount point of another drive"
        ));
    }
    None
}

#[cfg(not(any(windows, unix)))]
fn os_crit() -> Vec<String> {
    Vec::new()
}

#[cfg(not(any(windows, unix)))]
fn os_protection(_p: &Path, _s: &str) -> Option<&'static str> {
    None
}

/// Avviso da mostrare nella conferma (si può procedere, ma meglio saperlo).
pub fn warning(p: &Path) -> Option<&'static str> {
    let s = norm(p);
    if p.join(".git").exists() {
        return Some(tr!(
            "È un repository git: le modifiche non committate/pushate andrebbero perse.",
            "It is a git repository: uncommitted/unpushed changes would be lost."
        ));
    }
    if p.join(".obsidian").exists() {
        return Some(tr!(
            "Contiene un vault Obsidian: sono note personali.",
            "It contains an Obsidian vault: these are personal notes."
        ));
    }
    let lower = s.to_lowercase();
    if lower.contains(".ollama") || lower.contains(".lmstudio") {
        return Some(tr!(
            "Sono modelli gestiti da un'app: meglio rimuoverli dall'app stessa.",
            "These models are managed by an app: better remove them from the app itself."
        ));
    }
    if cfg!(windows) {
        if lower.contains("\\program files") {
            return Some(tr!(
                "È un programma installato: meglio disinstallarlo da Impostazioni > App.",
                "It is an installed program: better uninstall it from Settings > Apps."
            ));
        }
        if lower.contains("\\appdata\\") {
            return Some(tr!(
                "Sono dati di un'applicazione: chiudila prima, e assicurati che non servano.",
                "This is application data: close the app first and make sure you do not need it."
            ));
        }
    } else {
        if s.starts_with("/opt/") || s.starts_with("/Applications/") {
            return Some(tr!(
                "È un programma installato: meglio disinstallarlo con il suo gestore (pacchetti, Finder).",
                "It is an installed program: better uninstall it with its manager (package manager, Finder)."
            ));
        }
        if lower.contains("/.config/")
            || lower.contains("/.local/share/")
            || lower.contains("/library/")
        {
            return Some(tr!(
                "Sono dati di un'applicazione: chiudila prima, e assicurati che non servano.",
                "This is application data: close the app first and make sure you do not need it."
            ));
        }
    }
    None
}

// ---------------------------------------------------------------- eliminazione

#[derive(Default)]
pub struct Errs {
    pub count: u64,
    pub first: Option<String>,
    /// Cartelle lasciate al loro posto perché sono repository git.
    pub kept_repos: u64,
}

impl Errs {
    fn add(&mut self, p: &Path, e: &io::Error) {
        self.count += 1;
        if self.first.is_none() {
            self.first = Some(format!("{}: {e}", p.display()));
        }
    }
}

pub struct Outcome {
    pub freed: u64,
    pub errs: Errs,
    /// Il percorso non esiste più.
    pub gone: bool,
}

fn remove_file_forced(p: &Path) -> io::Result<()> {
    match fs::remove_file(p) {
        Err(e) if cfg!(windows) && e.kind() == io::ErrorKind::PermissionDenied => {
            // File in sola lettura: togli l'attributo e riprova.
            let mut perm = fs::metadata(p)?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            fs::set_permissions(p, perm)?;
            fs::remove_file(p)
        }
        r => r,
    }
}

/// Socket, pipe e dispositivi: non si toccano mai (es. `/tmp/.X11-unix/X0`).
#[cfg(unix)]
fn is_special(ft: &fs::FileType) -> bool {
    use std::os::unix::fs::FileTypeExt;
    ft.is_socket() || ft.is_fifo() || ft.is_block_device() || ft.is_char_device()
}

#[cfg(not(unix))]
fn is_special(_ft: &fs::FileType) -> bool {
    false
}

#[derive(Clone, Copy)]
struct Opts {
    min_age: Option<Duration>,
    keep_repos: bool,
    skip_hidden_top: bool,
}

/// Cancella il contenuto di `dir` continuando anche se qualcosa fallisce (file in uso).
fn remove_contents(dir: &Path, o: Opts, top: bool, errs: &mut Errs) {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => return errs.add(dir, &e),
    };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if is_special(&ft) {
            continue;
        }
        if top && o.skip_hidden_top {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.')
                || name.starts_with("systemd-private-")
                || name.starts_with("snap-private-tmp")
            {
                continue;
            }
        }
        if ft.is_symlink() {
            // Si toglie il link, mai il suo bersaglio.
            if let Err(er) = fs::remove_file(&p).or_else(|_| fs::remove_dir(&p)) {
                errs.add(&p, &er);
            }
        } else if ft.is_dir() {
            // Un repository git (es. un worktree finito nei temporanei) può contenere lavoro non salvato.
            if o.keep_repos && p.join(".git").exists() {
                errs.kept_repos += 1;
                continue;
            }
            remove_contents(&p, o, false, errs);
            // Riesce solo se è rimasta vuota: con min_age è normale che non lo sia.
            if let Err(er) = fs::remove_dir(&p)
                && o.min_age.is_none()
            {
                errs.add(&p, &er);
            }
        } else {
            if let Some(age) = o.min_age {
                let recent = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|t| t.elapsed().map(|el| el < age).unwrap_or(true))
                    .unwrap_or(true);
                if recent {
                    continue;
                }
            }
            if let Err(er) = remove_file_forced(&p) {
                errs.add(&p, &er);
            }
        }
    }
}

/// Elimina un file o una cartella: nel cestino oppure definitivamente.
pub fn delete_path(p: &Path, size: u64, permanent: bool) -> Outcome {
    let mut errs = Errs::default();
    if !permanent {
        return match trash::delete(p) {
            Ok(()) => Outcome {
                freed: size,
                errs,
                gone: !p.exists(),
            },
            Err(e) => {
                errs.count = 1;
                errs.first = Some(format!("{}: {e}", crate::i18n::trash_name()));
                Outcome {
                    freed: 0,
                    errs,
                    gone: false,
                }
            }
        };
    }
    if p.is_dir() && !fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
        remove_contents(
            p,
            Opts {
                min_age: None,
                keep_repos: false,
                skip_hidden_top: false,
            },
            true,
            &mut errs,
        );
        if let Err(e) = fs::remove_dir(p) {
            errs.add(p, &e);
        }
    } else if let Err(e) = remove_file_forced(p).or_else(|_| fs::remove_dir(p)) {
        errs.add(p, &e);
    }
    let gone = !p.exists();
    let freed = if gone {
        size
    } else {
        size.saturating_sub(dir_size(p).0)
    };
    Outcome { freed, errs, gone }
}

/// Esegue la pulizia di una voce.
pub fn run(path: &Path, action: &Action, known_size: u64) -> Outcome {
    let mut errs = Errs::default();
    match action {
        Action::ClearContents {
            min_age,
            skip_hidden,
        } => {
            let before = dir_size(path).0;
            let o = Opts {
                min_age: *min_age,
                keep_repos: true,
                skip_hidden_top: *skip_hidden,
            };
            remove_contents(path, o, true, &mut errs);
            let after = dir_size(path).0;
            Outcome {
                freed: before.saturating_sub(after),
                errs,
                gone: false,
            }
        }
        Action::DeleteDir => delete_path(path, known_size, true),
        Action::EmptyRecycleBin => {
            let before = dir_size(path).0;
            let status = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Clear-RecycleBin -Force -ErrorAction SilentlyContinue",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if let Err(e) = status {
                errs.add(path, &e);
            }
            let after = dir_size(path).0;
            Outcome {
                freed: before.saturating_sub(after),
                errs,
                gone: false,
            }
        }
        Action::None => Outcome {
            freed: 0,
            errs,
            gone: false,
        },
    }
}

// ---------------------------------------------------------------- integrazione col sistema

/// Mostra un file o una cartella nel gestore di file del sistema.
pub fn open_in_file_manager(p: &Path) {
    let quiet = |c: &mut Command| {
        c.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("explorer.exe");
        if p.is_dir() {
            c.arg(p);
        } else {
            c.raw_arg(format!("/select,\"{}\"", p.display()));
        }
        quiet(&mut c);
        let _ = c.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let mut c = Command::new("open");
        if p.is_dir() {
            c.arg(p);
        } else {
            c.arg("-R").arg(p);
        }
        quiet(&mut c);
        let _ = c.spawn();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let target = if p.is_dir() {
            p
        } else {
            p.parent().unwrap_or(p)
        };
        let mut c = Command::new("xdg-open");
        c.arg(target);
        quiet(&mut c);
        let _ = c.spawn();
    }
}

/// Copia un testo negli appunti con lo strumento del sistema.
pub fn copy_to_clipboard(text: &str) -> bool {
    use std::io::Write;
    let pipe = |cmd: &str, args: &[&str]| -> bool {
        let Ok(mut child) = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        let wrote = child
            .stdin
            .take()
            .is_some_and(|mut s| s.write_all(text.as_bytes()).is_ok());
        wrote && child.wait().is_ok_and(|s| s.success())
    };
    if cfg!(windows) {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!("Set-Clipboard -Value '{}'", text.replace('\'', "''")),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    } else if cfg!(target_os = "macos") {
        pipe("pbcopy", &[])
    } else {
        pipe("wl-copy", &[])
            || pipe("xclip", &["-selection", "clipboard"])
            || pipe("xsel", &["--clipboard", "--input"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn protects_system_paths() {
        assert!(protection(Path::new("C:\\")).is_some());
        assert!(protection(Path::new("C:\\Windows")).is_some());
        assert!(protection(Path::new("C:\\Windows\\System32")).is_some());
        assert!(protection(Path::new("C:\\Windows\\Temp")).is_none());
        assert!(protection(Path::new("C:\\Users")).is_some());
        assert!(protection(Path::new("C:\\pagefile.sys")).is_some());
        if let Some(h) = home_dir() {
            assert!(protection(&h).is_some());
            assert!(protection(&h.join("Documents")).is_some());
            assert!(protection(&h.join(".ssh").join("id_ed25519")).is_some());
            assert!(protection(&h.join("Documents").join("vecchio-progetto")).is_none());
        }
    }

    #[test]
    #[cfg(unix)]
    fn protects_system_paths() {
        for p in [
            "/",
            "/usr",
            "/usr/lib/x",
            "/etc/hosts",
            "/home",
            "/var/lib/docker",
            "/System/Library",
            "/Applications",
            "/proc/1",
        ] {
            assert!(
                protection(Path::new(p)).is_some(),
                "{p} dovrebbe essere protetto"
            );
        }
        assert!(protection(Path::new("/var/tmp/x")).is_none());
        assert!(protection(Path::new("/opt/vecchia-app")).is_none());
        if let Some(h) = home_dir() {
            assert!(protection(&h).is_some());
            assert!(protection(&h.join("Documents")).is_some());
            assert!(protection(&h.join(".ssh").join("id_ed25519")).is_some());
            assert!(protection(&h.join("progetti").join("app").join("node_modules")).is_none());
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("disk-space-tui-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn clear_contents_respects_age() {
        let base = tmp("clear");
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("nuovo.txt"), b"ciao").unwrap();
        fs::write(base.join("sub").join("nuovo2.txt"), b"ciao").unwrap();
        // Con min_age i file appena creati restano.
        let o = run(
            &base,
            &Action::ClearContents {
                min_age: Some(Duration::from_secs(3600)),
                skip_hidden: false,
            },
            0,
        );
        assert_eq!(o.freed, 0);
        assert!(base.join("nuovo.txt").exists());
        // Senza min_age si svuota tutto ma la cartella resta.
        let o = run(&base, &Action::clear(), 0);
        assert_eq!(o.freed, 8);
        assert!(base.exists());
        assert_eq!(fs::read_dir(&base).unwrap().count(), 0);
        fs::remove_dir(&base).unwrap();
    }

    #[test]
    fn clear_contents_keeps_git_repos_and_hidden() {
        let base = tmp("git");
        fs::create_dir_all(base.join("worktree").join("src")).unwrap();
        fs::write(base.join("worktree").join(".git"), b"gitdir: altrove").unwrap();
        fs::write(
            base.join("worktree").join("src").join("lavoro.rs"),
            b"fn main() {}",
        )
        .unwrap();
        fs::create_dir_all(base.join(".X11-unix")).unwrap();
        fs::write(base.join(".X11-unix").join("X0"), b"x").unwrap();
        fs::write(base.join("vecchio.tmp"), b"x").unwrap();
        let o = run(
            &base,
            &Action::ClearContents {
                min_age: None,
                skip_hidden: true,
            },
            0,
        );
        assert_eq!(o.errs.kept_repos, 1);
        assert!(base.join("worktree").join("src").join("lavoro.rs").exists());
        assert!(base.join(".X11-unix").join("X0").exists());
        assert!(!base.join("vecchio.tmp").exists());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn delete_does_not_follow_symlinks() {
        let base = tmp("link");
        let target = base.join("bersaglio");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("importante.txt"), b"x").unwrap();
        let dir = base.join("da-eliminare");
        fs::create_dir_all(&dir).unwrap();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&target, dir.join("link")).is_ok();
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_dir(&target, dir.join("link")).is_ok();
        let o = delete_path(&dir, 0, true);
        assert!(o.gone);
        assert!(
            target.join("importante.txt").exists(),
            "il bersaglio del link non va toccato (link creato: {linked})"
        );
        fs::remove_dir_all(&base).unwrap();
    }
}
