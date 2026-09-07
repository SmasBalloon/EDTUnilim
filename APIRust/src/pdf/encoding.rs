//! Décodage des chaînes PDF simples (`WinAnsiEncoding`).
//!
//! Les emplois du temps n'embarquent que des polices Type1 standard en
//! `WinAnsiEncoding` : un simple Latin-1 suffit, à l'exception de la plage
//! `0x80..=0x9F` que Windows-1252 réaffecte.

/// Table des caractères Windows-1252 pour `0x80..=0x9F` (`\u{FFFD}` = non défini).
const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}', '\u{017D}', '\u{FFFD}',
    '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
];

pub fn decode_win_ansi(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => CP1252_HIGH[(b - 0x80) as usize],
            _ => b as char,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_latin1_accents() {
        assert_eq!(decode_win_ansi(b"Mon\xe9di\xe8re T."), "Monédière T.");
    }

    #[test]
    fn decodes_cp1252_specifics() {
        assert_eq!(decode_win_ansi(b"l\x92ann\xe9e"), "l’année");
    }
}
