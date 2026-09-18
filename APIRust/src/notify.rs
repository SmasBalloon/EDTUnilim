//! Messages privés Discord : emplois du temps publiés et changements.
//!
//! Quand le département republie un PDF, l'ancienne et la nouvelle version
//! sont comparées créneau par créneau, pour chaque classe suivie ; seules les
//! personnes dont un cours a bougé sont prévenues, avec le `.ics` à jour. Un
//! PDF régénéré sans changement de créneau ne déclenche rien.

use std::collections::BTreeMap;
use std::sync::Arc;

use tracing::{info, warn};

use crate::discord::{Attachment, Button, ButtonStyle, Discord, DiscordError};
use crate::homework::Homework;
use crate::ics;
use crate::model::{Course, CourseKind, Period, Timetable};
use crate::users::{User, UserStore};

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

/// Compare deux versions d'une semaine, pour les cours d'un demi-groupe.
pub fn diff<'a>(before: &'a Timetable, after: &'a Timetable, group: &str) -> Changes<'a> {
    let before: Vec<&Course> = before.courses.iter().filter(|course| course.concerns(group)).collect();
    let after: Vec<&Course> = after.courses.iter().filter(|course| course.concerns(group)).collect();

    let mut removed: Vec<&Course> =
        before.iter().copied().filter(|old| !after.iter().any(|new| same_course(old, new))).collect();
    let mut added: Vec<&Course> =
        after.iter().copied().filter(|new| !before.iter().any(|old| same_course(old, new))).collect();

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
fn short_label(item: &Homework) -> String {
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

/// Le `.ics` d'une semaine pour un demi-groupe.
fn calendar(timetable: &Timetable, group: &str) -> Attachment {
    Attachment {
        filename: format!("edt-A{}-S{}-{}.ics", timetable.year, timetable.week, group.to_uppercase()),
        content: ics::build(timetable, group).into_bytes(),
    }
}

/// Personnes abonnées à une année, regroupées par demi-groupe.
fn by_group(users: Vec<User>) -> BTreeMap<String, Vec<User>> {
    let mut groups: BTreeMap<String, Vec<User>> = BTreeMap::new();
    for user in users {
        if let Some(group) = user.group.clone() {
            groups.entry(group.to_lowercase()).or_default().push(user);
        }
    }
    groups
}

/// Envoie les messages privés aux personnes abonnées.
#[derive(Clone)]
pub struct Notifier {
    discord: Discord,
    users: Arc<UserStore>,
}

impl Notifier {
    pub fn new(discord: Discord, users: Arc<UserStore>) -> Self {
        Self { discord, users }
    }

    /// Une nouvelle semaine : chaque abonné de l'année reçoit sa semaine.
    pub async fn week_published(&self, timetable: &Timetable) {
        for (group, users) in by_group(self.users.subscribers(timetable.year).await) {
            if !timetable.courses.iter().any(|course| course.concerns(&group)) {
                continue;
            }
            let file = calendar(timetable, &group);
            let message = published_message(timetable, &group);
            for user in &users {
                self.deliver(user, std::slice::from_ref(&message), &file).await;
            }
        }
    }

    /// Une semaine republiée : seuls les groupes dont un cours a bougé sont prévenus.
    pub async fn week_changed(&self, before: &Timetable, after: &Timetable) {
        for (group, users) in by_group(self.users.subscribers(after.year).await) {
            let changes = diff(before, after, &group);
            if changes.is_empty() {
                continue;
            }
            info!(year = after.year, week = after.week, group, "changements signalés");
            let file = calendar(after, &group);
            let messages = change_messages(after, &group, &changes);
            for user in &users {
                self.deliver(user, &messages, &file).await;
            }
        }
    }

    /// Confirmation d'abonnement, avec la semaine en cours quand elle est connue.
    pub async fn welcome(&self, user: &User, current: Option<&Timetable>) -> Result<(), DiscordError> {
        let Some((year, group)) = user.class() else { return Ok(()) };
        let intro = format!(
            "👋 C'est noté : tu recevras ici les emplois du temps de BUT {year} {} et leurs changements.",
            group.to_uppercase()
        );
        match current {
            Some(timetable) => {
                let text = format!(
                    "{intro}\n{}",
                    published_message(timetable, group).replacen("publiée", "en cours", 1)
                );
                self.discord.send_dm(&user.id, &text, Some(&calendar(timetable, group))).await
            }
            None => self.discord.send_dm(&user.id, &intro, None).await,
        }
    }

    // --- Devoirs -----------------------------------------------------------

    /// Un devoir de classe vient d'être noté : la classe est invitée à le
    /// confirmer. Son auteur ne reçoit rien — il sait déjà.
    pub async fn homework_proposed(&self, item: &Homework) {
        let text = format!(
            "📝 **{}** propose un devoir {} :\n{}\nConfirme-le s'il est exact — il n'apparaîtra pour tout le monde qu'ensuite.",
            item.author_name,
            item.audience.label(item.year),
            describe_homework(item),
        );
        let buttons = vec![Button {
            id: format!("hw:confirm:{}", item.id),
            label: "Confirmer".into(),
            style: ButtonStyle::Success,
        }];
        self.to_class(item, &text, buttons).await;
    }

    /// Le devoir est confirmé : il devient officiel pour la classe.
    pub async fn homework_confirmed(&self, item: &Homework) {
        let confirmed = item.confirmed_name.as_deref().unwrap_or("quelqu'un");
        let text = format!(
            "✅ Devoir confirmé par **{confirmed}** {} :\n{}",
            item.audience.label(item.year),
            describe_homework(item),
        );
        let buttons = vec![Button {
            id: format!("hw:done:{}", item.id),
            label: "Marquer comme fait".into(),
            style: ButtonStyle::Primary,
        }];
        self.to_class(item, &text, buttons).await;
    }

    /// Rappel de la veille : ce qui est à rendre demain et qui n'est pas fait.
    pub async fn homework_due_tomorrow(&self, user: &User, items: &[&Homework]) {
        if items.is_empty() {
            return;
        }
        let list: Vec<String> = items.iter().map(|item| format!("• {}", describe_homework(item))).collect();
        let text = format!("⏰ Pour demain, il te reste :\n{}", list.join("\n"));
        let buttons = items
            .iter()
            .take(5)
            .map(|item| Button {
                id: format!("hw:done:{}", item.id),
                label: short_label(item),
                style: ButtonStyle::Primary,
            })
            .collect();
        if let Err(error) = self.discord.send_dm_with(&user.id, &text, None, buttons).await {
            warn!(%error, user = user.id, "rappel de devoir impossible");
        }
    }

    /// Écrit à la classe visée par un devoir, sauf à celui qui l'a noté.
    async fn to_class(&self, item: &Homework, text: &str, buttons: Vec<Button>) {
        for user in self.users.subscribers(item.year).await {
            if user.id == item.author || !item.visible_to(&user) {
                continue;
            }
            if let Err(error) = self.discord.send_dm_with(&user.id, text, None, buttons.clone()).await {
                warn!(%error, user = user.id, "message privé impossible");
            }
        }
    }

    /// Envoie les messages, le fichier joint au dernier.
    async fn deliver(&self, user: &User, messages: &[String], file: &Attachment) {
        for (index, message) in messages.iter().enumerate() {
            let attachment = (index + 1 == messages.len()).then_some(file);
            if let Err(error) = self.discord.send_dm(&user.id, message, attachment).await {
                warn!(%error, user = user.id, "message privé impossible");
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
            teacher_id: Some("dupont-m".into()),
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
        assert!(diff(&before, &after, "G1a").is_empty());
    }

    #[test]
    fn reports_a_room_change_as_a_modification() {
        let before = week(vec![course(600, "G1A", "104"), course(840, "G1", "105")]);
        let after = week(vec![course(600, "G1A", "208"), course(480, "G1", "103")]);
        let changes = diff(&before, &after, "G1a");
        assert!(diff(&before, &after, "G3a").is_empty(), "un autre groupe n'est pas concerné");

        // G1A : salle changée. G1B : cours retiré. G1 entier : cours ajouté.
        assert_eq!(changes.modified.len(), 1);
        assert_eq!(changes.added.len(), 1);
        assert_eq!(changes.removed.len(), 1);

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
}
