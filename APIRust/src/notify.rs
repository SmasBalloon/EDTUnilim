//! Messages privés Discord : emplois du temps publiés et changements.
//!
//! Quand le département republie un PDF, l'ancienne et la nouvelle version
//! sont comparées créneau par créneau, pour chaque classe suivie ; seules les
//! personnes dont un cours a bougé sont prévenues, avec le `.ics` à jour. Un
//! PDF régénéré sans changement de créneau ne déclenche rien.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

use crate::conflicts::{self, Booking, Conflict};
use crate::discord::{Attachment, Button, ButtonStyle, Discord, DiscordError};
use crate::homework::Homework;
use crate::ics;
use crate::model::{Course, CourseKind, Period, Timetable};
use crate::users::{User, UserStore};

/// Longueur maximale d'un message Discord.
const MESSAGE_LIMIT: usize = 2000;

/// Messages privés envoyés en même temps. Assez pour que toute une promo
/// reçoive son emploi du temps dans la même poignée de secondes, assez peu
/// pour rester sous la limite globale de Discord (50 requêtes par seconde,
/// deux par message) ; un 429 éventuel est de toute façon réessayé.
const PARALLEL_DELIVERIES: usize = 20;

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
    what_with(course, true)
}

/// Comme [`what`], la salle en option : un conflit la nomme une fois pour tous ses cours.
fn what_with(course: &Course, show_room: bool) -> String {
    let mut parts = vec![kind_label(course.kind).to_owned()];
    if let Some(label) = course.resource.as_ref().map(|r| r.code.clone()).or_else(|| course.title.clone()) {
        parts[0] = format!("{} {label}", parts[0]);
    }
    if !course.groups.is_empty() {
        parts.push(course.groups.join(", "));
    }
    if let Some(room) = course.room.as_ref().filter(|_| show_room) {
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

/// Les salles réservées en double, une ligne par conflit et un cours par ligne
/// (salle, cours, groupe, enseignant). Vide quand il n'y en a pas.
pub fn conflict_messages(conflicts: &[&Conflict]) -> Vec<String> {
    if conflicts.is_empty() {
        return Vec::new();
    }
    let mut lines = vec!["🚨 **Salle réservée par deux cours en même temps**".to_owned()];
    for conflict in conflicts {
        lines.push(format!(
            "⚠️ salle {} · {}\n   • {}\n   • {}",
            conflict.room,
            short_date_of(conflict.first.course),
            booking_line(&conflict.first),
            booking_line(&conflict.second),
        ));
    }
    pack(lines)
}

/// `mar. 15/09`
fn short_date_of(course: &Course) -> String {
    let day: String = course.day.to_lowercase().chars().take(3).collect();
    format!("{day}. {}", short_date(course.date))
}

/// `10:00–12:00 · BUT 1 · TP R1.05 · G1A · Dupont M.` : la salle est dans le titre du conflit.
fn booking_line(booking: &Booking) -> String {
    let course = booking.course;
    format!("{}–{} · BUT {} · {}", course.start, course.end, booking.year, what_with(course, false))
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

/// Les salles doubles réservées par `timetable` avec les autres années, sauf
/// celles que `previous` (la version d'avant) réservait déjà.
fn new_conflicts<'a>(
    timetable: &'a Timetable,
    others: &[&'a Timetable],
    previous: Option<&'a Timetable>,
) -> Vec<Conflict<'a>> {
    // Une semaine de la même année ne partage pas ses salles : c'est une autre version.
    let others: Vec<&Timetable> =
        others.iter().copied().filter(|other| other.year != timetable.year).collect();
    let with = |own: &'a Timetable| -> Vec<&'a Timetable> {
        [own].into_iter().chain(others.iter().copied()).collect()
    };
    let known: HashSet<String> = previous
        .map(|before| conflicts::find(&with(before)).iter().map(Conflict::key).collect())
        .unwrap_or_default();
    conflicts::find(&with(timetable))
        .into_iter()
        .filter(|c| c.involves_year(timetable.year) && !known.contains(&c.key()))
        .collect()
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

/// Les mêmes messages et le même `.ics` pour tout un demi-groupe.
struct Batch {
    messages: Vec<String>,
    /// Absent quand le message ne concerne pas l'emploi du temps de ces personnes.
    file: Option<Attachment>,
    users: Vec<User>,
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
    ///
    /// `others` : les semaines des autres années aux mêmes dates, dont les
    /// salles sont partagées avec celle-ci. Les salles réservées en double sont
    /// signalées à chaque groupe concerné, de n'importe quelle année.
    pub async fn week_published(&self, timetable: &Timetable, others: &[&Timetable]) {
        let found = new_conflicts(timetable, others, None);
        let mut batches = Vec::new();
        for (group, users) in by_group(self.users.subscribers(timetable.year).await) {
            if !timetable.courses.iter().any(|course| course.concerns(&group)) {
                continue;
            }
            let mut messages = vec![published_message(timetable, &group)];
            let concerned: Vec<&Conflict> =
                found.iter().filter(|c| c.concerns(timetable.year, &group)).collect();
            messages.extend(conflict_messages(&concerned));
            batches.push(Batch { file: Some(calendar(timetable, &group)), messages, users });
        }
        batches.extend(self.other_years(&found, timetable.year).await);
        self.deliver_all(batches).await;
    }

    /// Une semaine republiée : seuls les groupes dont un cours a bougé sont prévenus.
    ///
    /// Les salles doubles réservées par cette version et pas par la précédente
    /// partent dans le même envoi, y compris pour un groupe dont aucun cours n'a
    /// bougé mais qu'un cours déplacé chez un autre vient de mettre en conflit —
    /// dans la même année ou dans une autre (`others`).
    pub async fn week_changed(&self, before: &Timetable, after: &Timetable, others: &[&Timetable]) {
        let found = new_conflicts(after, others, Some(before));
        let mut batches = Vec::new();
        for (group, users) in by_group(self.users.subscribers(after.year).await) {
            let changes = diff(before, after, &group);
            let concerned: Vec<&Conflict> = found.iter().filter(|c| c.concerns(after.year, &group)).collect();
            if changes.is_empty() && concerned.is_empty() {
                continue;
            }
            info!(
                year = after.year,
                week = after.week,
                group,
                conflicts = concerned.len(),
                "changements signalés"
            );
            let mut messages =
                if changes.is_empty() { Vec::new() } else { change_messages(after, &group, &changes) };
            messages.extend(conflict_messages(&concerned));
            batches.push(Batch { file: Some(calendar(after, &group)), messages, users });
        }
        batches.extend(self.other_years(&found, after.year).await);
        self.deliver_all(batches).await;
    }

    /// Les groupes des autres années pris dans un conflit : ils ne reçoivent que
    /// celui-ci, leur propre emploi du temps n'a pas changé.
    async fn other_years(&self, found: &[Conflict<'_>], own_year: u8) -> Vec<Batch> {
        let mut years: Vec<u8> = found.iter().flat_map(|c| [c.first.year, c.second.year]).collect();
        years.sort_unstable();
        years.dedup();
        let mut batches = Vec::new();
        for year in years.into_iter().filter(|year| *year != own_year) {
            for (group, users) in by_group(self.users.subscribers(year).await) {
                let concerned: Vec<&Conflict> = found.iter().filter(|c| c.concerns(year, &group)).collect();
                if !concerned.is_empty() {
                    batches.push(Batch { file: None, messages: conflict_messages(&concerned), users });
                }
            }
        }
        batches
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

    /// Les mêmes annonces, mais sans faire attendre celui qui a cliqué.
    ///
    /// Prévenir une classe, c'est deux requêtes à Discord par personne — bien
    /// plus que les trois secondes qu'il accorde pour répondre à un bouton ou
    /// à une commande. L'annonce part donc de son côté : ce qui compte est
    /// déjà enregistré, la réponse n'a pas à l'attendre.
    pub fn announce_proposed(&self, item: Homework) {
        let notifier = self.clone();
        tokio::spawn(async move { notifier.homework_proposed(&item).await });
    }

    pub fn announce_confirmed(&self, item: Homework) {
        let notifier = self.clone();
        tokio::spawn(async move { notifier.homework_confirmed(&item).await });
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
        let (mut sent, mut failed) = (0, 0);
        for user in self.users.subscribers(item.year).await {
            if user.id == item.author || !item.visible_to(&user) {
                continue;
            }
            match self.discord.send_dm_with(&user.id, text, None, buttons.clone()).await {
                Ok(()) => sent += 1,
                Err(error) => {
                    failed += 1;
                    warn!(%error, user = user.id, "message privé impossible");
                }
            }
        }
        info!(id = item.id, sent, failed, "devoir annoncé à la classe");
    }

    /// Envoie chaque lot à tous ses destinataires, en parallèle : toute la
    /// promo reçoit l'emploi du temps en même temps, pas au fil d'une file
    /// d'attente où les derniers l'auraient plusieurs minutes après les premiers.
    async fn deliver_all(&self, batches: Vec<Batch>) {
        if batches.is_empty() {
            return;
        }
        let started = Instant::now();
        let groups = batches.len();
        let slots = Arc::new(Semaphore::new(PARALLEL_DELIVERIES));
        let mut deliveries = JoinSet::new();
        for Batch { messages, file, users } in batches {
            let content = Arc::new((messages, file));
            for user in users {
                let (notifier, content, slots) = (self.clone(), Arc::clone(&content), Arc::clone(&slots));
                deliveries.spawn(async move {
                    let _slot = slots.acquire_owned().await;
                    notifier.deliver(&user, &content.0, content.1.as_ref()).await
                });
            }
        }
        let (mut sent, mut failed) = (0, 0);
        while let Some(done) = deliveries.join_next().await {
            match done {
                Ok(true) => sent += 1,
                Ok(false) => failed += 1,
                Err(error) => {
                    failed += 1;
                    warn!(%error, "envoi d'un message privé interrompu");
                }
            }
        }
        info!(
            groups,
            sent,
            failed,
            ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "messages privés distribués"
        );
    }

    /// Envoie les messages, le fichier joint au dernier ; vrai si tout est parti.
    async fn deliver(&self, user: &User, messages: &[String], file: Option<&Attachment>) -> bool {
        for (index, message) in messages.iter().enumerate() {
            let attachment = file.filter(|_| index + 1 == messages.len());
            if let Err(error) = self.discord.send_dm(&user.id, message, attachment).await {
                warn!(%error, user = user.id, "message privé impossible");
                return false;
            }
        }
        debug!(user = user.id, messages = messages.len(), "message privé envoyé");
        true
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

    // --- Mise en forme -----------------------------------------------------

    fn homework(title: &str) -> Homework {
        Homework {
            id: "h1".into(),
            author: "42".into(),
            author_name: "Ada".into(),
            year: 1,
            audience: crate::homework::Audience::Class { group: Some("G1".into()) },
            title: title.into(),
            details: None,
            resource: Some("R1.05".into()),
            due: None,
            course: None,
            created_at: time::OffsetDateTime::now_utc(),
            confirmed_by: None,
            confirmed_name: None,
            done: Vec::new(),
        }
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

    // --- Envois ------------------------------------------------------------

    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::config::DiscordConfig;

    struct Bench {
        server: MockServer,
        notifier: Notifier,
        users: Arc<UserStore>,
        _dir: tempfile::TempDir,
    }

    /// Un Discord simulé qui accepte tout, et deux abonnés du même demi-groupe.
    async fn bench() -> Bench {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/users/@me/channels"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"900"}"#))
            .mount(&server)
            .await;
        let discord = Discord::new(DiscordConfig {
            client_id: "1".into(),
            client_secret: "secret".into(),
            bot_token: "jeton".into(),
            guild_id: "7".into(),
            known_guild_ids: Vec::new(),
            session_secret: "s".repeat(32),
            public_key: None,
            api_base: server.uri(),
        })
        .expect("client Discord");

        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let users = Arc::new(UserStore::open(dir.path()).await.expect("comptes"));
        for (id, name) in [("42", "Ada"), ("43", "Bob")] {
            users.sign_in(id, name, None).await.expect("compte");
            users
                .update(id, |user| {
                    user.year = Some(1);
                    user.group = Some("G1a".into());
                })
                .await
                .expect("classe");
        }
        Bench { notifier: Notifier::new(discord, Arc::clone(&users)), server, users, _dir: dir }
    }

    async fn messages(bench: &Bench, status: u16, expected: u64) {
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .respond_with(ResponseTemplate::new(status).set_body_string("{}"))
            .expect(expected)
            .mount(&bench.server)
            .await;
    }

    #[tokio::test]
    async fn tells_each_subscriber_about_a_new_week() {
        let bench = bench().await;
        messages(&bench, 200, 2).await;

        bench.notifier.week_published(&week(vec![course(600, "G1A", "104")]), &[]).await;
        // Un groupe sans cours cette semaine n'est pas dérangé.
        bench.notifier.week_published(&week(vec![course(600, "G3A", "104")]), &[]).await;
    }

    #[tokio::test]
    async fn only_tells_the_groups_whose_courses_moved() {
        let bench = bench().await;
        messages(&bench, 200, 2).await;

        let before = week(vec![course(600, "G1A", "104")]);
        bench.notifier.week_changed(&before, &before, &[]).await;
        bench.notifier.week_changed(&before, &week(vec![course(600, "G1A", "208")]), &[]).await;
    }

    #[tokio::test]
    async fn sends_to_everyone_at_the_same_time() {
        let bench = bench().await;
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("{}")
                    .set_delay(std::time::Duration::from_millis(400)),
            )
            .expect(2)
            .mount(&bench.server)
            .await;

        // L'un après l'autre, les deux envois prendraient au moins 800 ms.
        let started = std::time::Instant::now();
        bench.notifier.week_published(&week(vec![course(600, "G1A", "104")]), &[]).await;
        assert!(started.elapsed() < std::time::Duration::from_millis(750), "{:?}", started.elapsed());
    }

    #[tokio::test]
    async fn keeps_going_when_a_message_cannot_be_delivered() {
        let bench = bench().await;
        // Messages privés fermés : l'erreur arrête cette personne, pas les autres.
        messages(&bench, 403, 2).await;
        bench.notifier.week_published(&week(vec![course(600, "G1A", "104")]), &[]).await;
    }

    #[tokio::test]
    async fn welcomes_a_person_with_or_without_the_current_week() {
        let bench = bench().await;
        messages(&bench, 200, 2).await;
        let ada = bench.users.get("42").await.expect("compte");

        bench
            .notifier
            .welcome(&ada, Some(&week(vec![course(600, "G1A", "104")])))
            .await
            .expect("avec semaine");
        bench.notifier.welcome(&ada, None).await.expect("sans semaine");

        // Sans classe choisie, il n'y a rien à envoyer.
        let mut classless = ada.clone();
        classless.year = None;
        bench.notifier.welcome(&classless, None).await.expect("rien à envoyer");
    }

    #[tokio::test]
    async fn asks_the_class_to_confirm_but_not_the_author() {
        let bench = bench().await;
        // Bob seul reçoit : Ada a noté le devoir.
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .and(body_string_contains("hw:confirm:h1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&bench.server)
            .await;
        bench.notifier.homework_proposed(&homework("Exercices")).await;

        let mut item = homework("Exercices");
        item.confirmed_name = Some("Bob".into());
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .and(body_string_contains("hw:done:h1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&bench.server)
            .await;
        bench.notifier.homework_confirmed(&item).await;
    }

    // --- Salles doubles réservées ------------------------------------------

    /// Accepte tout message que les attentes plus précises n'ont pas pris.
    async fn accept_the_rest(bench: &Bench) {
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .mount(&bench.server)
            .await;
    }

    fn overlapping_week() -> Timetable {
        let mut other = course(660, "G2A", "104");
        other.teacher = Some("Martin P.".into());
        other.resource = Some(Resource { code: "R1.02".into(), name: None });
        week(vec![course(600, "G1A", "104"), other])
    }

    #[test]
    fn spells_out_room_course_group_and_teacher_of_a_conflict() {
        let timetable = overlapping_week();
        let found = conflicts::find(&[&timetable]);
        let text = conflict_messages(&found.iter().collect::<Vec<_>>()).join("\n");
        assert!(text.contains("salle 104 · mar. 15/09"), "{text}");
        assert!(text.contains("10:00–12:00 · BUT 1 · TP R1.05 · G1A · Dupont M."), "{text}");
        assert!(text.contains("11:00–13:00 · BUT 1 · TP R1.02 · G2A · Martin P."), "{text}");
        assert!(conflict_messages(&[]).is_empty());
    }

    #[tokio::test]
    async fn adds_conflicts_to_the_publication_message() {
        let bench = bench().await;
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .and(body_string_contains("Salle réservée par deux cours"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(2)
            .mount(&bench.server)
            .await;
        // Les autres messages (annonce, changements) passent sans condition.
        accept_the_rest(&bench).await;
        bench.notifier.week_published(&overlapping_week(), &[]).await;
    }

    #[tokio::test]
    async fn reports_a_new_conflict_even_when_the_group_courses_did_not_move() {
        let bench = bench().await;
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .and(body_string_contains("Salle réservée par deux cours"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(2)
            .mount(&bench.server)
            .await;
        // G2A déplace un cours dans la salle de G1A : G1A ne voit aucun changement
        // de son emploi du temps, mais sa salle est maintenant prise deux fois.
        let before = week(vec![course(600, "G1A", "104"), course(600, "G2A", "105")]);
        let mut moved = course(660, "G2A", "104");
        moved.teacher = Some("Martin P.".into());
        let after = week(vec![course(600, "G1A", "104"), moved]);
        accept_the_rest(&bench).await;
        bench.notifier.week_changed(&before, &after, &[]).await;
    }

    /// Une semaine de l'année `year`, avec un cours de `group` dans la salle 104.
    fn other_year(year: u8, group: &str) -> Timetable {
        let mut other = course(660, group, "104");
        other.teacher = Some("Martin P.".into());
        other.resource = Some(Resource { code: "R3.02".into(), name: None });
        Timetable { year, ..week(vec![other]) }
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

    /// Un abonné de l'année `year` et du demi-groupe `G1a`, de plus.
    async fn subscribe(bench: &Bench, id: &str, year: u8) {
        bench.users.sign_in(id, "Cléo", None).await.expect("compte");
        bench
            .users
            .update(id, |user| {
                user.year = Some(year);
                user.group = Some("G1a".into());
            })
            .await
            .expect("classe");
    }

    #[tokio::test]
    async fn tells_the_other_year_about_a_conflict_without_its_own_timetable() {
        let bench = bench().await;
        subscribe(&bench, "44", 2).await;
        // Cléo (A2) ne reçoit que le conflit : un message, sans l'annonce.
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .and(body_string_contains("BUT 2 · TP R3.02"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(3)
            .mount(&bench.server)
            .await;
        accept_the_rest(&bench).await;

        // Ada et Bob (A1 G1a) reçoivent l'annonce puis le conflit ; Cléo le conflit seul.
        let first = week(vec![course(600, "G1A", "104")]);
        bench.notifier.week_published(&first, &[&other_year(2, "G1A")]).await;
    }

    #[tokio::test]
    async fn ignores_a_week_of_the_same_year_among_the_others() {
        let bench = bench().await;
        messages(&bench, 200, 0).await;
        let mut same_year = other_year(1, "G2A");
        same_year.week = 4;
        // Même année : c'est une autre version, pas une autre promo — aucun conflit.
        let before = week(vec![course(600, "G1A", "104")]);
        bench.notifier.week_changed(&before, &before, &[&same_year]).await;
    }

    #[tokio::test]
    async fn does_not_repeat_a_conflict_already_known() {
        let bench = bench().await;
        messages(&bench, 200, 0).await;
        let timetable = overlapping_week();
        bench.notifier.week_changed(&timetable, &timetable, &[]).await;
    }

    #[tokio::test]
    async fn announces_in_the_background() {
        let bench = bench().await;
        messages(&bench, 200, 2).await;
        bench.notifier.announce_proposed(homework("Exercices"));
        bench.notifier.announce_confirmed(homework("Exercices"));
        // Le temps que les deux annonces partent avant la vérification des attentes.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    #[tokio::test]
    async fn reminds_what_is_due_tomorrow() {
        let bench = bench().await;
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .and(body_string_contains("Pour demain"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&bench.server)
            .await;
        let ada = bench.users.get("42").await.expect("compte");
        let (one, two) = (homework("Un"), homework("Deux"));

        bench.notifier.homework_due_tomorrow(&ada, &[]).await;
        bench.notifier.homework_due_tomorrow(&ada, &[&one, &two]).await;
    }

    #[tokio::test]
    async fn a_failed_reminder_is_only_logged() {
        let bench = bench().await;
        messages(&bench, 500, 1).await;
        let ada = bench.users.get("42").await.expect("compte");
        bench.notifier.homework_due_tomorrow(&ada, &[&homework("Un")]).await;
    }
}
