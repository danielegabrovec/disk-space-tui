//! Lingua dell'interfaccia: italiano o inglese.
//!
//! Si sceglie, in ordine: `--lang it|en`, variabile `DISK_SPACE_TUI_LANG`, lingua del
//! sistema (italiano se inizia con «it», altrimenti inglese).
//!
//! Nel codice i testi si scrivono in coppia con le macro `tr!` (testo fisso) e `trf!`
//! (testo con `format!`): prima l'italiano, poi l'inglese.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    It,
    En,
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    LANG.store(if lang == Lang::En { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn get() -> Lang {
    if LANG.load(Ordering::Relaxed) == 1 {
        Lang::En
    } else {
        Lang::It
    }
}

pub fn en() -> bool {
    get() == Lang::En
}

pub fn parse(s: &str) -> Option<Lang> {
    let s = s.trim().to_ascii_lowercase();
    if s.starts_with("it") {
        Some(Lang::It)
    } else if s.starts_with("en") {
        Some(Lang::En)
    } else {
        None
    }
}

/// Lingua da usare se l'utente non la indica.
pub fn detect() -> Lang {
    if let Some(l) = std::env::var("DISK_SPACE_TUI_LANG")
        .ok()
        .as_deref()
        .and_then(parse)
    {
        return l;
    }
    match sys_locale::get_locale() {
        Some(loc) if loc.to_ascii_lowercase().starts_with("it") => Lang::It,
        _ => Lang::En,
    }
}

/// Nome del gestore di file del sistema, per le etichette («Mostra in …»).
pub fn file_manager() -> &'static str {
    if cfg!(windows) {
        if en() {
            "File Explorer"
        } else {
            "Esplora risorse"
        }
    } else if cfg!(target_os = "macos") {
        "Finder"
    } else if en() {
        "file manager"
    } else {
        "gestore file"
    }
}

/// Nome del tasto Canc/Del e di Maiusc/Shift.
pub fn key_del() -> &'static str {
    if en() { "Del" } else { "Canc" }
}

pub fn key_shift_del() -> &'static str {
    if en() { "Shift+Del" } else { "Maiusc+Canc" }
}

/// Nome del cestino del sistema.
pub fn trash_name() -> &'static str {
    match (cfg!(windows), en()) {
        (true, false) => "Cestino",
        (true, true) => "Recycle Bin",
        (false, false) => "Cestino",
        (false, true) => "Trash",
    }
}

/// Testo fisso nelle due lingue: `tr!("italiano", "english")`.
#[macro_export]
macro_rules! tr {
    ($it:expr, $en:expr $(,)?) => {
        if $crate::i18n::en() { $en } else { $it }
    };
}

/// Testo formattato nelle due lingue: `trf!("ciao {nome}", "hello {nome}")`.
#[macro_export]
macro_rules! trf {
    ($it:literal, $en:literal $(, $args:expr)* $(,)?) => {
        if $crate::i18n::en() { format!($en $(, $args)*) } else { format!($it $(, $args)*) }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lang() {
        assert_eq!(parse("it_IT.UTF-8"), Some(Lang::It));
        assert_eq!(parse("EN"), Some(Lang::En));
        assert_eq!(parse("de"), None);
    }
}
