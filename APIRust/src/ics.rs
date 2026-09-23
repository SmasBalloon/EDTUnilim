//! Export au format iCalendar (RFC 5545) : une semaine, jointe aux messages
//! privés, ou toutes les semaines publiées, servies en abonnement (webcal).
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

/// En-têtes propres à l'abonnement.
const SUBSCRIPTION: &[&str] = &[
    "X-WR-CALDESC:Emploi du temps de l'IUT du Limousin\\, département informatique",
    "X-WR-TIMEZONE:Europe/Paris",
    // Rythme de relecture suggéré. Apple et Thunderbird le suivent ; Google
    // relit à son propre rythme, de quelques heures à une journée.
    "REFRESH-INTERVAL;VALUE=DURATION:PT3H",
    "X-PUBLISHED-TTL:PT3H",
];

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

/// `20260915T080509Z`
fn stamp(now: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
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
    let name = format!("EDT A{} S{} {}", timetable.year, timetable.week, group.to_uppercase());
    calendar(&name, &[], [timetable], group)
}

/// Calendrier d'abonnement : toutes les semaines publiées d'une année, pour un
/// demi-groupe. L'agenda qui s'y abonne le relit de lui-même : un cours
/// déplacé dans le PDF l'est aussi chez lui, sans rien réimporter.
pub fn subscription<'a>(
    year: u8,
    timetables: impl IntoIterator<Item = &'a Timetable>,
    group: &str,
) -> String {
    let name = format!("EDT A{year} {}", group.to_uppercase());
    calendar(&name, SUBSCRIPTION, timetables, group)
}

fn calendar<'a>(
    name: &str,
    extra: &[&str],
    timetables: impl IntoIterator<Item = &'a Timetable>,
    group: &str,
) -> String {
    let stamp = stamp(OffsetDateTime::now_utc());

    let mut lines = vec![
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".to_owned(),
        "PRODID:-//edt-unilim//fr".to_owned(),
        "CALSCALE:GREGORIAN".to_owned(),
        "METHOD:PUBLISH".to_owned(),
        fold(&format!("X-WR-CALNAME:{name}")),
    ];
    lines.extend(extra.iter().map(|line| fold(line)));
    lines.push(VTIMEZONE.to_owned());

    for timetable in timetables {
        for course in timetable.courses.iter().filter(|course| course.concerns(group)) {
            event(&mut lines, timetable.year, course, &stamp);
        }
    }

    lines.push("END:VCALENDAR".to_owned());
    // BOM : sans lui, Outlook lit le fichier en Latin-1 et perd les accents.
    format!("\u{feff}{}\r\n", lines.join("\r\n"))
}

fn event(lines: &mut Vec<String>, year: u8, course: &Course, stamp: &str) {
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
    // Stable d'une version à l'autre : un agenda qui relit le fichier met à
    // jour le créneau au lieu de le dupliquer.
    lines.push(fold(&format!(
        "UID:A{}-{}-{}-{}@edt-unilim",
        year,
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
        assert!(calendar.contains("X-WR-CALNAME:EDT A1 S3 G1A\r\n"));
        assert!(calendar.contains("DTSTART;TZID=Europe/Paris:2026"));
        assert!(calendar.contains("BEGIN:VTIMEZONE"));
        assert!(!calendar.contains("REFRESH-INTERVAL"));
        assert!(calendar.ends_with("END:VCALENDAR\r\n"));
    }

    #[test]
    fn subscription_gathers_every_week_under_one_calendar() {
        let timetable = crate::pdf::parse(A1_S3, 1, 3).expect("PDF analysable");
        let calendar = subscription(1, [&timetable, &timetable], "G1a");

        let per_week = timetable.courses.iter().filter(|c| c.concerns("G1a")).count();
        assert_eq!(calendar.matches("BEGIN:VEVENT").count(), 2 * per_week);
        assert_eq!(calendar.matches("BEGIN:VCALENDAR").count(), 1);
        assert!(calendar.contains("X-WR-CALNAME:EDT A1 G1A\r\n"));
        assert!(calendar.contains("REFRESH-INTERVAL;VALUE=DURATION:PT3H\r\n"));
    }

    #[test]
    fn subscription_without_weeks_is_still_a_valid_calendar() {
        let calendar = subscription(2, [], "G1a");

        assert!(calendar.contains("BEGIN:VCALENDAR"));
        assert!(calendar.contains("END:VTIMEZONE\r\nEND:VCALENDAR\r\n"));
        assert!(!calendar.contains("BEGIN:VEVENT"));
    }

    #[test]
    fn stamps_in_utc_with_the_ical_layout() {
        let now = time::macros::datetime!(2026-09-15 08:05:09 UTC);
        assert_eq!(stamp(now), "20260915T080509Z");
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
