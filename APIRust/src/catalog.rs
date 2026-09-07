//! Référentiel des ressources et SAÉ du BUT informatique.
//!
//! Le fichier est embarqué dans le binaire : il change une fois par réforme du
//! programme, pas au fil des semaines.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

const RESOURCES_CSV: &str = include_str!("../data/resources.csv");

/// Une entrée du référentiel.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogEntry {
    pub code: String,
    pub name: String,
    pub semester: u8,
}

/// Renvoie le référentiel, analysé une seule fois pour toute la durée du process.
pub fn entries() -> &'static [CatalogEntry] {
    static ENTRIES: OnceLock<Vec<CatalogEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| RESOURCES_CSV.lines().skip(1).filter_map(parse_line).collect())
}

fn parse_line(line: &str) -> Option<CatalogEntry> {
    let mut fields = line.trim().split(';');
    let code = fields.next()?.trim();
    let name = fields.next()?.trim();
    let semester = fields.next()?.trim().parse().ok()?;
    (!code.is_empty() && !name.is_empty()).then(|| CatalogEntry {
        code: code.to_owned(),
        name: name.to_owned(),
        semester,
    })
}

fn index() -> &'static HashMap<&'static str, &'static CatalogEntry> {
    static INDEX: OnceLock<HashMap<&'static str, &'static CatalogEntry>> = OnceLock::new();
    INDEX.get_or_init(|| entries().iter().map(|entry| (entry.code.as_str(), entry)).collect())
}

/// Intitulé officiel d'un code ressource, s'il figure au référentiel.
pub fn name_of(code: &str) -> Option<&'static str> {
    index().get(code).map(|entry| entry.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_loaded() {
        assert!(entries().len() > 50);
        assert_eq!(name_of("R1.01"), Some("Initiation au développement"));
        assert_eq!(name_of("R9.99"), None);
    }
}
