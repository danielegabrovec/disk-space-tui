//! Formattazione di dimensioni, numeri e barre.

use ratatui::style::Color;

/// Separatore decimale e delle migliaia della lingua attiva.
fn seps() -> (char, char) {
    if crate::i18n::en() {
        ('.', ',')
    } else {
        (',', '.')
    }
}

/// Dimensione leggibile in base 1024: "12,3 GB" (italiano) o "12.3 GB" (inglese).
pub fn fmt_size(b: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if b < 1024 {
        return format!("{b} B");
    }
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    let s = if v >= 100.0 {
        format!("{v:.0}")
    } else if v >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    };
    format!("{} {}", s.replace('.', &seps().0.to_string()), UNITS[i])
}

/// Numero con il separatore delle migliaia: "1.234.567" (italiano) o "1,234,567" (inglese).
pub fn fmt_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(seps().1);
        }
        out.push(c);
    }
    out
}

/// «1 voce», «3 voci».
pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!(
        "{} {}",
        fmt_count(n as u64),
        if n == 1 { one } else { many }
    )
}

/// Percentuale da un rapporto 0..1: "12,3%".
pub fn fmt_pct(r: f64) -> String {
    format!("{:.1}%", r * 100.0).replace('.', &seps().0.to_string())
}

/// Barra testuale piena/vuota larga `width` caratteri.
pub fn bar(ratio: f64, width: usize) -> String {
    let full = ((ratio.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    format!("{}{}", "█".repeat(full), "░".repeat(width - full))
}

/// Verde fino al 70%, giallo fino al 90%, poi rosso.
pub fn ratio_color(r: f64) -> Color {
    if r < 0.7 {
        Color::Green
    } else if r < 0.9 {
        Color::Yellow
    } else {
        Color::Red
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(fmt_size(512), "512 B");
        assert_eq!(fmt_size(1536), "1,50 KB");
        assert_eq!(fmt_size(15 * 1024 * 1024), "15,0 MB");
        assert_eq!(fmt_size(250 * 1024 * 1024 * 1024), "250 GB");
    }

    #[test]
    fn counts() {
        assert_eq!(fmt_count(0), "0");
        assert_eq!(fmt_count(999), "999");
        assert_eq!(fmt_count(1000), "1.000");
        assert_eq!(fmt_count(1234567), "1.234.567");
    }
}
