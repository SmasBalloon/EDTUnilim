//! Le modèle métier exposé par l'API.

use std::fmt;

use serde::{Serialize, Serializer};
use time::Date;

/// Nature d'un créneau, déduite de la couleur de la case dans le PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CourseKind {
    /// Travaux pratiques (demi-groupe).
    Tp,
    /// Situation d'apprentissage et d'évaluation.
    Sae,
    /// Travaux dirigés.
    Td,
    /// Cours magistral en amphithéâtre.
    Amphi,
    /// Contrôle / évaluation.
    Controle,
}

impl CourseKind {
    /// Couleurs de fond utilisées par le générateur d'emplois du temps.
    const PALETTE: [(&'static str, CourseKind); 5] = [
        ("#b3ffff", CourseKind::Tp),
        ("#9fff9f", CourseKind::Sae),
        ("#ffbab3", CourseKind::Td),
        ("#ffff0c", CourseKind::Amphi),
        ("#f23ea7", CourseKind::Controle),
    ];

    pub fn from_hex(hex: &str) -> Option<Self> {
        Self::PALETTE.iter().find(|(code, _)| *code == hex).map(|(_, kind)| *kind)
    }

    /// Un TP ne concerne qu'une moitié de groupe.
    pub fn splits_group(self) -> bool {
        matches!(self, CourseKind::Tp)
    }
}

/// Un horaire de la journée, sérialisé en `HH:MM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClockTime {
    minutes: u16,
}

impl ClockTime {
    pub fn from_minutes(minutes: i64) -> Self {
        Self { minutes: minutes.clamp(0, 24 * 60) as u16 }
    }

    /// Arrondit au multiple de 5 minutes le plus proche : les bords des cases
    /// sont géométriques, donc entachés d'une erreur de quelques secondes.
    pub fn rounded_to_5(minutes: f64) -> Self {
        Self::from_minutes((minutes / 5.0).round() as i64 * 5)
    }

    pub fn minutes(self) -> u16 {
        self.minutes
    }
}

impl fmt::Display for ClockTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.minutes / 60, self.minutes % 60)
    }
}

impl Serialize for ClockTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Une ressource du programme national (`R1.01`, `S3.01A`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resource {
    pub code: String,
    /// Intitulé officiel, repris du référentiel quand le code y figure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Un créneau de l'emploi du temps.
#[derive(Debug, Clone, Serialize)]
pub struct Course {
    pub day: String,
    #[serde(with = "crate::model::iso_date")]
    pub date: Date,
    pub start: ClockTime,
    pub end: ClockTime,
    pub duration_minutes: u16,
    pub kind: CourseKind,
    /// Groupes concernés (`G1`, `G2A`, …), dans l'ordre de la grille.
    pub groups: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Resource>,
    /// Intitulé tel qu'imprimé, présent surtout sur les grandes cases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Enseignant, ramené au même nom quelle que soit l'écriture du PDF.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teacher: Option<String>,
    /// Identifiant de cet enseignant, stable d'une semaine et d'une année à
    /// l'autre : c'est par lui que se filtre son emploi du temps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teacher_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
}

impl Course {
    /// Nature du créneau en toutes lettres minuscules, comme en JSON.
    pub fn kind_code(&self) -> &'static str {
        match self.kind {
            CourseKind::Tp => "tp",
            CourseKind::Sae => "sae",
            CourseKind::Td => "td",
            CourseKind::Amphi => "amphi",
            CourseKind::Controle => "controle",
        }
    }

    /// Un créneau de `G1` concerne aussi bien `G1a` que `G1b`, et
    /// réciproquement : demander `G1a` rend les cours du groupe entier.
    pub fn concerns(&self, wanted: &str) -> bool {
        self.groups.iter().any(|group| matches_group(group, wanted))
    }
}

fn matches_group(group: &str, wanted: &str) -> bool {
    let (group, wanted) = (group.trim(), wanted.trim());
    let common = group.len().min(wanted.len());
    common > 0
        && group.is_char_boundary(common)
        && wanted.is_char_boundary(common)
        && group[..common].eq_ignore_ascii_case(&wanted[..common])
}

/// L'en-tête du document : semaine couverte et date de génération.
#[derive(Debug, Clone, Serialize)]
pub struct Period {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub week_number: Option<u8>,
    #[serde(with = "crate::model::iso_date")]
    pub start: Date,
    #[serde(with = "crate::model::iso_date")]
    pub end: Date,
    /// Ligne de titre brute, conservée telle quelle.
    pub label: String,
}

/// Un emploi du temps complet, tel que renvoyé par l'API.
#[derive(Debug, Clone, Serialize)]
pub struct Timetable {
    pub year: u8,
    pub week: u8,
    pub period: Period,
    pub courses: Vec<Course>,
}

/// Sérialisation des dates au format ISO 8601 (`2026-08-31`).
pub mod iso_date {
    use serde::Serializer;
    use time::macros::format_description;
    use time::Date;

    pub fn serialize<S: Serializer>(date: &Date, serializer: S) -> Result<S::Ok, S::Error> {
        let format = format_description!("[year]-[month]-[day]");
        match date.format(&format) {
            Ok(text) => serializer.serialize_str(&text),
            Err(_) => serializer.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_clock_time() {
        assert_eq!(ClockTime::from_minutes(510).to_string(), "08:30");
        assert_eq!(ClockTime::rounded_to_5(779.4).to_string(), "13:00");
    }

    #[test]
    fn group_filter_spans_half_groups() {
        assert!(matches_group("G1", "G1a"));
        assert!(matches_group("G1a", "G1"));
        assert!(matches_group("G1a", "g1a"));
        assert!(!matches_group("G1a", "G1b"));
        assert!(!matches_group("G2", "G1"));
    }

    #[test]
    fn maps_palette_to_kinds() {
        assert_eq!(CourseKind::from_hex("#b3ffff"), Some(CourseKind::Tp));
        assert_eq!(CourseKind::from_hex("#123456"), None);
    }
}
