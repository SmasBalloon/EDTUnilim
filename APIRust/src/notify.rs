//! Signalement des changements d'emploi du temps sur Discord.
//!
//! Quand le département republie un PDF, l'ancienne et la nouvelle version
//! sont comparées créneau par créneau ; les différences partent dans un salon
//! Discord par webhook. Un PDF régénéré sans changement de créneau ne
//! déclenche rien.

use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use tracing::warn;

use crate::model::{Course, CourseKind, Period, Timetable};

/// Longueur maximale d'un message Discord.
const MESSAGE_LIMIT: usize = 2000;

/// Ce qui distingue deux versions d'une même semaine.
#[derive(Debug, Default)]
pub struct Changes<'a> {
    pub added: Vec<&'a Course>,
    pub removed: Vec<&'a Course>,
    /// Même créneau (jour, horaire, groupes), contenu modifié : `(avant, après)`.
    pub modified: Vec<(&'a Course, &'a Course)>,
}

impl Changes<'_> {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

/// Tout ce qui décrit un créneau : deux créneaux égaux ici sont identiques.
fn same_course(a: &Course, b: &Course) -> bool {
    same_slot(a, b)
        && a.kind == b.kind
        && a.resource.as_ref().map(|r| &r.code) == b.resource.as_ref().map(|r| &r.code)
        && a.title == b.title
        && a.teacher == b.teacher
        && a.room == b.room
}

/// Le même créneau pour les mêmes groupes, quel qu'en soit le contenu.
fn same_slot(a: &Course, b: &Course) -> bool {
    a.date == b.date && a.start == b.start && a.end == b.end && a.groups == b.groups
}

/// Compare deux versions d'une semaine.
pub fn diff<'a>(before: &'a Timetable, after: &'a Timetable) -> Changes<'a> {
    let mut removed: Vec<&Course> =
        before.courses.iter().filter(|old| !after.courses.iter().any(|new| same_course(old, new))).collect();
    let mut added: Vec<&Course> =
        after.courses.iter().filter(|new| !before.courses.iter().any(|old| same_course(old, new))).collect();

    // Un créneau retiré puis remis au même endroit a simplement changé de
    // salle, d'enseignant ou de matière.
    let mut modified = Vec::new();
    removed.retain(|old| match added.iter().position(|new| same_slot(old, new)) {
        Some(index) => {
            modified.push((*old, added.remove(index)));
            false
        }
        None => true,
    });

    let order = |c: &&Course| (c.date, c.start, c.groups.clone());
    added.sort_by_key(order);
    removed.sort_by_key(order);
    modified.sort_by_key(|(old, _)| (old.date, old.start, old.groups.clone()));
    Changes { added, removed, modified }
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
    let day: String = course.day.to_lowercase().chars().take(3).collect();
    format!("{day}. {} {}–{}", short_date(course.date), course.start, course.end)
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
pub fn change_messages(after: &Timetable, changes: &Changes) -> Vec<String> {
    let mut lines = vec![format!("📅 **{} modifiée**", heading(after.year, after.week, &after.period))];
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

/// Annonce d'une semaine qui vient d'apparaître dans l'index.
pub fn new_week_message(timetable: &Timetable) -> String {
    format!(
        "🆕 **{} publiée** — {} créneaux",
        heading(timetable.year, timetable.week, &timetable.period),
        timetable.courses.len()
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

/// Envoie des messages à un webhook Discord.
#[derive(Clone)]
pub struct Notifier {
    client: reqwest::Client,
    webhook: String,
}

impl std::fmt::Debug for Notifier {
    // L'URL du webhook vaut mot de passe : elle ne doit pas finir dans les journaux.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notifier").finish_non_exhaustive()
    }
}

impl Notifier {
    pub fn new(webhook: String) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?;
        Ok(Self { client, webhook })
    }

    pub async fn send(&self, messages: &[String]) {
        for content in messages {
            let body = serde_json::json!({ "content": content, "allowed_mentions": { "parse": [] } });
            let sent = self
                .client
                .post(&self.webhook)
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string())
                .send()
                .await
                .and_then(reqwest::Response::error_for_status);
            if let Err(error) = sent {
                // `without_url` : l'erreur reqwest embarquerait sinon le jeton du webhook.
                warn!(error = %error.without_url(), "notification Discord impossible");
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;
    use crate::model::{ClockTime, Resource};

    fn course(start: i64, group: &str, room: &str) -> Course {
        Course {
            day: "MARDI".into(),
            date: date!(2026 - 09 - 15),
            start: ClockTime::from_minutes(start),
            end: ClockTime::from_minutes(start + 120),
            duration_minutes: 120,
            kind: CourseKind::Tp,
            groups: vec![group.into()],
            resource: Some(Resource { code: "R1.05".into(), name: None }),
            title: None,
            teacher: Some("Dupont M.".into()),
            room: Some(room.into()),
        }
    }

    fn week(courses: Vec<Course>) -> Timetable {
        Timetable {
            year: 1,
            week: 3,
            period: Period {
                week_number: Some(38),
                start: date!(2026 - 09 - 14),
                end: date!(2026 - 09 - 19),
                label: String::new(),
            },
            courses,
        }
    }

    #[test]
    fn ignores_an_identical_republication() {
        let before = week(vec![course(600, "G1A", "104")]);
        let after = week(vec![course(600, "G1A", "104")]);
        assert!(diff(&before, &after).is_empty());
    }

    #[test]
    fn reports_a_room_change_as_a_modification() {
        let before = week(vec![course(600, "G1A", "104"), course(840, "G1B", "105")]);
        let after = week(vec![course(600, "G1A", "208"), course(480, "G2A", "103")]);
        let changes = diff(&before, &after);

        assert_eq!(changes.modified.len(), 1);
        assert_eq!(changes.added.len(), 1);
        assert_eq!(changes.removed.len(), 1);

        let text = change_messages(&after, &changes).join("\n");
        assert!(text.contains("BUT 1 · semaine 3 (14/09 → 19/09)"), "{text}");
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
}
