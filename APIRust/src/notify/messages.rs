//! Mise en forme des messages Discord.

use crate::conflicts::{self, Booking, Conflict};
use crate::homework::Homework;
use crate::model::{Course, CourseKind, Period, Timetable};

use super::Changes;

/// Longueur maximale d'un message Discord.
const MESSAGE_LIMIT: usize = 2000;

/// « R1.05 — Exercices 3 à 7 (pour le 21/09) »
pub fn describe_homework(item: &Homework) -> String {
    let mut text = format!("**{}**", item.summary());
    if let Some(due) = item.due {
        text.push_str(&format!(" — pour le {}", french_date(due)));
    }
    if let Some(details) = &item.details {
        text.push_str(&format!("\n> {details}"));
    }
    text
}

/// Un intitulé assez court pour tenir sur un bouton Discord.
pub(super) fn short_label(item: &Homework) -> String {
    let summary = item.summary();
    match summary.char_indices().nth(72) {
        Some((cut, _)) => format!("Fait : {}…", &summary[..cut]),
        None => format!("Fait : {summary}"),
    }
}

/// `21/09`
fn french_date(day: time::Date) -> String {
    format!("{:02}/{:02}", day.day(), u8::from(day.month()))
}

fn kind_label(kind: CourseKind) -> &'static str {
    match kind {
        CourseKind::Amphi => "CM",
        CourseKind::Td => "TD",
        CourseKind::Tp => "TP",
        CourseKind::Sae => "SAÉ",
        CourseKind::Controle => "Contrôle",
    }
}

/// `15/09`
fn short_date(date: time::Date) -> String {
    format!("{:02}/{:02}", date.day(), u8::from(date.month()))
}

/// `mar. 15/09 10:00–12:00`
fn when(course: &Course) -> String {
    format!("{} {}–{}", short_date_of(course), course.start, course.end)
}

/// `mar. 15/09`
fn short_date_of(course: &Course) -> String {
    let day: String = course.day.to_lowercase().chars().take(3).collect();
    format!("{day}. {}", short_date(course.date))
}

/// `TP R1.05 · G1A · salle 104 · Dupont M.`
fn what(course: &Course) -> String {
    let mut parts = vec![kind_label(course.kind).to_owned()];
    if let Some(label) = course.resource.as_ref().map(|r| r.code.clone()).or_else(|| course.title.clone()) {
        parts[0] = format!("{} {label}", parts[0]);
    }
    if !course.groups.is_empty() {
        parts.push(course.groups.join(", "));
    }
    if let Some(room) = &course.room {
        parts.push(format!("salle {room}"));
    }
    if let Some(teacher) = &course.teacher {
        parts.push(teacher.clone());
    }
    parts.join(" · ")
}

/// Ce qui a bougé entre deux versions d'un même créneau.
fn what_changed(before: &Course, after: &Course) -> String {
    let field = |name: &str, old: Option<&str>, new: Option<&str>| {
        (old != new).then(|| format!("{name} {} → {}", old.unwrap_or("—"), new.unwrap_or("—")))
    };
    let changes: Vec<String> = [
        (before.kind != after.kind)
            .then(|| format!("{} → {}", kind_label(before.kind), kind_label(after.kind))),
        field(
            "matière",
            before.resource.as_ref().map(|r| r.code.as_str()),
            after.resource.as_ref().map(|r| r.code.as_str()),
        ),
        field("intitulé", before.title.as_deref(), after.title.as_deref()),
        field("salle", before.room.as_deref(), after.room.as_deref()),
        field("enseignant", before.teacher.as_deref(), after.teacher.as_deref()),
    ]
    .into_iter()
    .flatten()
    .collect();
    changes.join(", ")
}

/// `BUT 1 · semaine 3 (14/09 → 19/09)`
fn heading(year: u8, week: u8, period: &Period) -> String {
    format!("BUT {year} · semaine {week} ({} → {})", short_date(period.start), short_date(period.end))
}

/// Le compte rendu des changements, découpé en messages Discord.
pub fn change_messages(after: &Timetable, group: &str, changes: &Changes) -> Vec<String> {
    let mut lines = vec![format!(
        "📅 **{} modifiée pour {}**",
        heading(after.year, after.week, &after.period),
        group.to_uppercase()
    )];
    for (before, now) in &changes.modified {
        lines.push(format!("✏️ {} · {} — {}", when(now), what(now), what_changed(before, now)));
    }
    for course in &changes.added {
        lines.push(format!("➕ {} · {}", when(course), what(course)));
    }
    for course in &changes.removed {
        lines.push(format!("➖ ~~{} · {}~~", when(course), what(course)));
    }
    pack(lines)
}

/// Les salles réservées en double, une ligne par salle disputée et un cours
/// par ligne (cours, groupe, enseignant). Vide quand il n'y en a pas.
pub fn conflict_messages(conflicts: &[&Conflict]) -> Vec<String> {
    conflict_lines("🚨 **Salle réservée par plusieurs cours en même temps**", conflicts)
}

/// Comme [`conflict_messages`], pour l'état des lieux de toutes les semaines à venir.
pub fn conflict_report(conflicts: &[&Conflict]) -> Vec<String> {
    conflict_lines("📋 **Salles réservées par plusieurs cours en même temps — état actuel**", conflicts)
}

fn conflict_lines(title: &str, conflicts: &[&Conflict]) -> Vec<String> {
    if conflicts.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![title.to_owned()];
    for overlap in conflicts::group(conflicts) {
        let mut line = format!("⚠️ salle {} · {}", overlap.room, short_date_of(overlap.first().course));
        for booking in &overlap.bookings {
            line.push_str(&format!("\n   • {}", booking_line(booking)));
        }
        lines.push(line);
    }
    pack(lines)
}

/// `10:00–12:00 · BUT 1 · TP R1.05 · G1A · salle 108-9 · Dupont M.` : chaque cours dit
/// sa salle telle que le PDF l'écrit, et `108` face à `108-9` montre d'où vient le conflit.
fn booking_line(booking: &Booking) -> String {
    let course = booking.course;
    format!("{}–{} · BUT {} · {}", course.start, course.end, booking.year, what(course))
}

/// Annonce d'une semaine qui vient d'apparaître dans l'index.
pub fn published_message(timetable: &Timetable, group: &str) -> String {
    let count = timetable.courses.iter().filter(|course| course.concerns(group)).count();
    format!(
        "🆕 **{} publiée** — {} cours pour {}. Le fichier .ics s'importe dans ton agenda.",
        heading(timetable.year, timetable.week, &timetable.period),
        count,
        group.to_uppercase()
    )
}

/// Regroupe des lignes en messages qui tiennent chacun dans la limite Discord.
fn pack(lines: Vec<String>) -> Vec<String> {
    let mut messages = Vec::new();
    let mut current = String::new();
    for line in lines {
        let line: String = line.chars().take(MESSAGE_LIMIT).collect();
        // Discord compte en caractères, pas en octets.
        if !current.is_empty() && current.chars().count() + 1 + line.chars().count() > MESSAGE_LIMIT {
            messages.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(&line);
    }
    if !current.is_empty() {
        messages.push(current);
    }
    messages
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;
    use crate::model::{ClockTime, Resource};
    use crate::notify::diff;
    use crate::notify::fixtures::{course, homework, other_year, overlapping_week, week};

    #[test]
    fn reports_a_room_change_as_a_modification() {
        let before = week(vec![course(600, "G1A", "104"), course(840, "G1", "105")]);
        let after = week(vec![course(600, "G1A", "208"), course(480, "G1", "103")]);
        let changes = diff(&before, &after, "G1a");

        let text = change_messages(&after, "G1a", &changes).join("\n");
        assert!(text.contains("BUT 1 · semaine 3 (14/09 → 19/09) modifiée pour G1A"), "{text}");
        assert!(text.contains("✏️ mar. 15/09 10:00–12:00 · TP R1.05 · G1A · salle 208"), "{text}");
        assert!(text.contains("salle 104 → 208"), "{text}");
        assert!(text.contains("➕ mar. 15/09 08:00–10:00"), "{text}");
        assert!(text.contains("➖ ~~mar. 15/09 14:00–16:00"), "{text}");
    }

    #[test]
    fn splits_long_reports_under_the_discord_limit() {
        let lines: Vec<String> = (0..100).map(|i| format!("{i:03} {}", "x".repeat(60))).collect();
        let messages = pack(lines);
        assert!(messages.len() > 1);
        assert!(messages.iter().all(|m| m.chars().count() <= MESSAGE_LIMIT));
        assert_eq!(messages.iter().map(|m| m.lines().count()).sum::<usize>(), 100);
    }

    #[test]
    fn describes_a_homework_with_its_due_date_and_details() {
        let mut item = homework("Exercices");
        assert_eq!(describe_homework(&item), "**R1.05 — Exercices**");
        item.due = Some(date!(2026 - 09 - 21));
        item.details = Some("pages 3 à 7".into());
        let text = describe_homework(&item);
        assert!(text.contains("pour le 21/09"), "{text}");
        assert!(text.ends_with("\n> pages 3 à 7"), "{text}");
        assert_eq!(french_date(date!(2026 - 01 - 05)), "05/01");
    }

    #[test]
    fn shortens_a_label_to_fit_on_a_button() {
        assert_eq!(short_label(&homework("Court")), "Fait : R1.05 — Court");
        let long = short_label(&homework(&"é".repeat(200)));
        assert!(long.starts_with("Fait : R1.05"), "{long}");
        assert!(long.ends_with('…'), "{long}");
        assert!(long.chars().count() < 90, "{long}");
    }

    #[test]
    fn says_what_a_course_is_and_what_changed_in_it() {
        let mut bare = course(600, "G1A", "104");
        bare.resource = None;
        bare.title = Some("Projet".into());
        bare.groups.clear();
        bare.room = None;
        bare.teacher = None;
        assert_eq!(what(&bare), "TP Projet");

        let before = course(600, "G1A", "104");
        let mut after = course(600, "G1A", "104");
        after.kind = CourseKind::Td;
        after.resource = Some(Resource { code: "R1.06".into(), name: None });
        after.title = Some("Autre".into());
        after.teacher = None;
        let text = what_changed(&before, &after);
        for expected in ["TP → TD", "matière R1.05 → R1.06", "intitulé — → Autre", "enseignant Dupont M. → —"]
        {
            assert!(text.contains(expected), "{expected} manque dans {text}");
        }
        for kind in [CourseKind::Amphi, CourseKind::Sae, CourseKind::Controle] {
            assert!(!kind_label(kind).is_empty());
        }
    }

    #[test]
    fn announces_a_publication_with_the_number_of_courses() {
        let timetable = week(vec![course(600, "G1A", "104"), course(840, "G2A", "105")]);
        let text = published_message(&timetable, "g1a");
        assert!(text.contains("BUT 1 · semaine 3 (14/09 → 19/09) publiée"), "{text}");
        assert!(text.contains("1 cours pour G1A"), "{text}");
    }

    #[test]
    fn spells_out_room_course_group_and_teacher_of_a_conflict() {
        let timetable = overlapping_week();
        let found = conflicts::find(&[&timetable]);
        let text = conflict_messages(&found.iter().collect::<Vec<_>>()).join("\n");
        assert!(text.contains("salle 104 · mar. 15/09"), "{text}");
        assert!(text.contains("10:00–12:00 · BUT 1 · TP R1.05 · G1A · salle 104 · Dupont M."), "{text}");
        assert!(text.contains("11:00–13:00 · BUT 1 · TP R1.02 · G2A · salle 104 · Martin P."), "{text}");
        assert!(conflict_messages(&[]).is_empty());
    }

    #[test]
    fn titles_the_report_as_the_current_state() {
        let timetable = overlapping_week();
        let found = conflicts::find(&[&timetable]);
        let text = conflict_report(&found.iter().collect::<Vec<_>>()).join("\n");
        assert!(text.contains("état actuel") && text.contains("salle 104 · mar. 15/09"), "{text}");
        assert!(conflict_report(&[]).is_empty());
    }

    #[test]
    fn names_each_course_once_when_a_long_course_covers_several() {
        let mut long = course(600, "G1A", "104");
        long.end = ClockTime::from_minutes(840);
        let mut morning = course(600, "G2A", "104");
        morning.teacher = Some("Martin P.".into());
        morning.resource = Some(Resource { code: "R1.02".into(), name: None });
        let mut noon = course(720, "G3A", "104");
        noon.teacher = Some("Durand J.".into());
        noon.resource = Some(Resource { code: "R1.03".into(), name: None });
        let timetable = week(vec![long, morning, noon]);

        let found = conflicts::find(&[&timetable]);
        assert_eq!(found.len(), 2);
        let text = conflict_messages(&found.iter().collect::<Vec<_>>()).join("\n");
        assert_eq!(text.matches("⚠️").count(), 1, "{text}");
        assert_eq!(text.matches("Dupont M.").count(), 1, "{text}");
        assert!(text.contains("Martin P.") && text.contains("Durand J."), "{text}");
    }

    #[test]
    fn names_the_combined_room_of_a_course() {
        let mut other = course(660, "G2A", "112");
        other.teacher = Some("Martin P.".into());
        let timetable = week(vec![course(600, "G1A", "111-2"), other]);
        let found = conflicts::find(&[&timetable]);
        let text = conflict_messages(&found.iter().collect::<Vec<_>>()).join("\n");
        assert!(text.contains("⚠️ salle 112 · mar. 15/09"), "{text}");
        assert!(text.contains("TP R1.05 · G1A · salle 111-2 · Dupont M."), "{text}");
        // Chaque cours dit sa salle, même celle du titre.
        assert!(text.contains("TP R1.05 · G2A · salle 112 · Martin P."), "{text}");
    }

    #[test]
    fn names_the_year_of_each_course_in_a_cross_year_conflict() {
        let first = week(vec![course(600, "G1A", "104")]);
        let second = other_year(2, "G1A");
        let found = conflicts::find(&[&first, &second]);
        let text = conflict_messages(&found.iter().collect::<Vec<_>>()).join("\n");
        assert!(text.contains("BUT 1 · TP R1.05 · G1A"), "{text}");
        assert!(text.contains("BUT 2 · TP R3.02 · G1A"), "{text}");
    }
}
