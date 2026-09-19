//! Export d'une semaine au format iCalendar (RFC 5545), joint aux messages privés.
//!
//! Les horaires du PDF sont des heures de Limoges : ils sont écrits tels quels
//! avec `TZID=Europe/Paris`, et le fuseau est décrit dans le fichier pour que
//! chaque agenda les replace correctement, heure d'été comprise.

use time::OffsetDateTime;

use crate::model::{Course, CourseKind, Timetable};

const VTIMEZONE: &str = "BEGIN:VTIMEZONE\r\n\
TZID:Europe/Paris\r\n\
BEGIN:DAYLIGHT\r\n\
TZOFFSETFROM:+0100\r\n\
TZOFFSETTO:+0200\r\n\
TZNAME:CEST\r\n\
DTSTART:19700329T020000\r\n\
RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\n\
END:DAYLIGHT\r\n\
BEGIN:STANDARD\r\n\
TZOFFSETFROM:+0200\r\n\
TZOFFSETTO:+0100\r\n\
TZNAME:CET\r\n\
DTSTART:19701025T030000\r\n\
RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\n\
END:STANDARD\r\n\
END:VTIMEZONE";

fn kind_label(kind: CourseKind) -> &'static str {
    match kind {
        CourseKind::Amphi => "CM",
        CourseKind::Td => "TD",
        CourseKind::Tp => "TP",
        CourseKind::Sae => "SAÉ",
        CourseKind::Controle => "Contrôle",
    }
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace('\n', "\\n")
}

/// Replie une ligne à 75 octets sans couper un caractère UTF-8.
fn fold(line: &str) -> String {
    let mut folded = String::with_capacity(line.len() + 8);
    let mut width = 0;
    for character in line.chars() {
        let size = character.len_utf8();
        // 74 octets utiles : les lignes suivantes commencent par une espace.
        if width + size > 74 {
            folded.push_str("\r\n ");
            width = 0;
        }
        folded.push(character);
        width += size;
    }
    folded
}

/// `20260915T100000`
fn local(course: &Course, minutes: u16) -> String {
    let date = course.date;
    format!(
        "{:04}{:02}{:02}T{:02}{:02}00",
        date.year(),
        u8::from(date.month()),
        date.day(),
        minutes / 60,
        minutes % 60
    )
}

/// `R3.01 - Développement web / TD / Monédière T.`
fn summary(course: &Course) -> String {
    let code = course.resource.as_ref().map(|r| r.code.as_str());
    let label = course.resource.as_ref().and_then(|r| r.name.as_deref()).or(course.title.as_deref());
    let resource = [code, label].into_iter().flatten().collect::<Vec<_>>().join(" - ");
    let resource = if resource.is_empty() { "Cours".to_owned() } else { resource };
    [Some(resource.as_str()), Some(kind_label(course.kind)), course.teacher.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Calendrier des cours d'une semaine, restreint à un demi-groupe.
pub fn build(timetable: &Timetable, group: &str) -> String {
    let now = OffsetDateTime::now_utc();
    let stamp = format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );

    let mut lines = vec![
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".to_owned(),
        "PRODID:-//edt-unilim//fr".to_owned(),
        "CALSCALE:GREGORIAN".to_owned(),
        "METHOD:PUBLISH".to_owned(),
        fold(&format!("X-WR-CALNAME:EDT A{} S{} {}", timetable.year, timetable.week, group.to_uppercase())),
        VTIMEZONE.to_owned(),
    ];

    for course in timetable.courses.iter().filter(|course| course.concerns(group)) {
        let start = course.start.minutes();
        let description = [
            course.resource.as_ref().map(|r| format!("Ressource : {}", r.code)),
            course.resource.as_ref().and_then(|r| r.name.as_ref()).map(|name| format!("Intitulé : {name}")),
            Some(format!("Groupes : {}", course.groups.join(", "))),
            course.teacher.as_ref().map(|teacher| format!("Enseignant : {teacher}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");

        lines.push("BEGIN:VEVENT".to_owned());
        // Stable d'une version à l'autre : un agenda qui réimporte le fichier
        // met à jour le créneau au lieu de le dupliquer.
        lines.push(fold(&format!(
            "UID:A{}-{}-{}-{}@edt-unilim",
            timetable.year,
            local(course, start),
            course.groups.join("-"),
            course.kind_code()
        )));
        lines.push(format!("DTSTAMP:{stamp}"));
        lines.push(format!("DTSTART;TZID=Europe/Paris:{}", local(course, start)));
        lines.push(format!("DTEND;TZID=Europe/Paris:{}", local(course, course.end.minutes())));
        lines.push(fold(&format!("SUMMARY:{}", escape(&summary(course)))));
        if let Some(room) = &course.room {
            lines.push(fold(&format!("LOCATION:{}", escape(room))));
        }
        lines.push(fold(&format!("DESCRIPTION:{}", escape(&description))));
        lines.push("END:VEVENT".to_owned());
    }

    lines.push("END:VCALENDAR".to_owned());
    // BOM : sans lui, Outlook lit le fichier en Latin-1 et perd les accents.
    format!("\u{feff}{}\r\n", lines.join("\r\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A1_S3: &[u8] = include_bytes!("../tests/fixtures/A1_S3.pdf");

    #[test]
    fn exports_only_the_courses_of_the_group() {
        let timetable = crate::pdf::parse(A1_S3, 1, 3).expect("PDF analysable");
        let calendar = build(&timetable, "G1a");

        let expected = timetable.courses.iter().filter(|c| c.concerns("G1a")).count();
        assert!(expected > 0);
        assert_eq!(calendar.matches("BEGIN:VEVENT").count(), expected);
        assert!(calendar.contains("DTSTART;TZID=Europe/Paris:2026"));
        assert!(calendar.contains("BEGIN:VTIMEZONE"));
        assert!(calendar.ends_with("END:VCALENDAR\r\n"));
    }

    #[test]
    fn folds_long_lines_on_character_boundaries() {
        let line = format!("SUMMARY:{}", "é".repeat(80));
        let folded = fold(&line);
        assert!(folded.split("\r\n").all(|part| part.len() <= 75));
        assert_eq!(folded.replace("\r\n ", ""), line);
    }

    #[test]
    fn labels_every_kind_of_course() {
        assert_eq!(kind_label(CourseKind::Amphi), "CM");
        assert_eq!(kind_label(CourseKind::Td), "TD");
        assert_eq!(kind_label(CourseKind::Tp), "TP");
        assert_eq!(kind_label(CourseKind::Sae), "SAÉ");
        assert_eq!(kind_label(CourseKind::Controle), "Contrôle");
    }

    #[test]
    fn escapes_what_ical_reserves() {
        assert_eq!(escape("a;b,c\\d\ne"), "a\\;b\\,c\\\\d\\ne");
    }
}
