//! «Si può eliminare?»: valutazione di un file o di una cartella, mostrata nel pannello
//! di dettaglio e come etichetta nelle liste.

use crate::cleanup::{self, Safety, Target};
use crate::{scan, tr, trf};
use ratatui::style::Color;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    /// Non si elimina da qui (sistema, profilo utente).
    Protetto,
    /// Si ricrea da solo: build, dipendenze, cache.
    Rigenerabile,
    /// Temporanei e simili: si cancella senza conseguenze.
    Sicuro,
    /// Si può, ma si perde qualcosa o va fatto dall'app che lo possiede.
    Attenzione,
    /// Repository git: il codice è al sicuro solo se è committato e pushato.
    Repo,
    /// Nessuna informazione: decide l'utente.
    Ignoto,
}

impl Level {
    pub fn tag(self) -> Option<&'static str> {
        match self {
            Level::Protetto => Some(tr!("sistema", "system")),
            Level::Rigenerabile => Some(tr!("rigenerabile", "regenerable")),
            Level::Sicuro => Some(tr!("pulibile", "cleanable")),
            Level::Attenzione => Some(tr!("attenzione", "caution")),
            Level::Repo => Some(tr!("repository git", "git repository")),
            Level::Ignoto => None,
        }
    }
    pub fn color(self) -> Color {
        match self {
            Level::Protetto => Color::Red,
            Level::Rigenerabile => Color::Cyan,
            Level::Sicuro => Color::Green,
            Level::Attenzione => Color::Yellow,
            Level::Repo => Color::Magenta,
            Level::Ignoto => Color::DarkGray,
        }
    }
    /// Simboli stretti (una colonna) anche nella console classica di Windows.
    pub fn icon(self) -> &'static str {
        match self {
            Level::Protetto => "✗",
            Level::Rigenerabile => "↻",
            Level::Sicuro => "✓",
            Level::Attenzione => "!",
            Level::Repo => "±",
            Level::Ignoto => "?",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Verdict {
    pub level: Level,
    pub title: String,
    pub text: String,
}

pub fn from_safety(s: Safety) -> Level {
    match s {
        Safety::Sicuro => Level::Sicuro,
        Safety::Rigenerabile => Level::Rigenerabile,
        Safety::Attenzione | Safety::Manuale => Level::Attenzione,
    }
}

/// Valutazione veloce per le etichette delle liste: niente risalita negli antenati.
pub fn quick(path: &Path, is_dir: bool, targets: &[Target]) -> Level {
    if cleanup::protection(path).is_some() {
        return Level::Protetto;
    }
    if let Some(t) = targets.iter().find(|t| !t.from_scan && t.path == path) {
        return from_safety(t.safety);
    }
    if is_dir
        && !scan::is_skip_path(path)
        && let (Some(parent), Some(name)) = (path.parent(), path.file_name())
        && scan::artifact_kind(parent, &name.to_string_lossy()).is_some()
    {
        return Level::Rigenerabile;
    }
    if is_dir && path.join(".git").exists() {
        return Level::Repo;
    }
    Level::Ignoto
}

/// Valutazione completa per il pannello di dettaglio.
pub fn full(path: &Path, is_dir: bool, targets: &[Target]) -> Verdict {
    if let Some(why) = cleanup::protection(path) {
        return Verdict {
            level: Level::Protetto,
            title: tr!("Protetto", "Protected").into(),
            text: trf!(
                "Non si elimina da qui: {why}.",
                "It cannot be deleted from here: {why}."
            ),
        };
    }
    // Una voce di pulizia nota, o qualcosa al suo interno.
    let mut best: Option<&Target> = None;
    for t in targets
        .iter()
        .filter(|t| !t.from_scan && path.starts_with(&t.path))
    {
        if best.is_none_or(|b| t.path.components().count() > b.path.components().count()) {
            best = Some(t);
        }
    }
    if let Some(t) = best {
        let inside = t.path != path;
        return Verdict {
            level: from_safety(t.safety),
            title: if inside {
                trf!("Dentro «{}»", "Inside “{}”", t.name)
            } else {
                t.name.clone()
            },
            text: format!(
                "{}. {}",
                t.detail,
                if inside {
                    tr!(
                        "Puoi anche pulire tutta la voce dalla scheda Pulizia.",
                        "You can also clean the whole entry from the Cleanup tab."
                    )
                } else {
                    tr!(
                        "Si pulisce dalla scheda Pulizia.",
                        "Clean it from the Cleanup tab."
                    )
                }
            ),
        };
    }
    // Un artefatto di compilazione, o qualcosa al suo interno.
    let mut cur = if is_dir { Some(path) } else { path.parent() };
    let mut is_self = is_dir;
    while let Some(p) = cur {
        let (Some(parent), Some(name)) = (p.parent(), p.file_name()) else {
            break;
        };
        if scan::is_skip_path(p) {
            break;
        }
        let name = name.to_string_lossy();
        if let Some(what) = scan::artifact_kind(parent, &name) {
            return if is_self {
                Verdict {
                    level: Level::Rigenerabile,
                    title: tr!("File di compilazione", "Build files").into(),
                    text: trf!(
                        "{what}. Si può eliminare: il progetto resta, si ricrea alla prossima build.",
                        "{what}. Safe to delete: the project stays, it is recreated on the next build."
                    ),
                }
            } else {
                Verdict {
                    level: Level::Rigenerabile,
                    title: trf!("Dentro «{name}»", "Inside “{name}”"),
                    text: trf!(
                        "{what}. Conviene eliminare tutta la cartella «{name}».",
                        "{what}. Better delete the whole “{name}” folder."
                    ),
                }
            };
        }
        is_self = false;
        cur = p.parent();
    }
    if is_dir && path.join(".git").exists() {
        return Verdict {
            level: Level::Repo,
            title: tr!("Repository git", "Git repository").into(),
            text: tr!(
                "È un progetto con git: eliminandolo si perde tutto ciò che non è committato e pushato. \
                 Per liberare spazio di solito basta togliere i file di compilazione al suo interno (scheda 3).",
                "It is a git project: deleting it loses everything that is not committed and pushed. \
                 To free space it is usually enough to remove the build files inside it (tab 3)."
            )
            .into(),
        };
    }
    if let Some(w) = cleanup::warning(path) {
        return Verdict {
            level: Level::Attenzione,
            title: tr!("Attenzione", "Caution").into(),
            text: w.into(),
        };
    }
    Verdict {
        level: Level::Ignoto,
        title: tr!("Nessuna informazione", "No information").into(),
        text: trf!(
            "Non so cosa sia: controlla tu cosa contiene prima di eliminarlo. Il {} ti permette di ripensarci.",
            "I do not know what this is: check what it contains before deleting it. The {} lets you change your mind.",
            crate::i18n::trash_name()
        ),
    }
}
