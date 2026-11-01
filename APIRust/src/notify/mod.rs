//! Messages privés Discord : emplois du temps publiés et changements.
//!
//! Quand le département republie un PDF, l'ancienne et la nouvelle version
//! sont comparées créneau par créneau, pour chaque classe suivie ; seules les
//! personnes dont un cours a bougé sont prévenues, avec le `.ics` à jour. Un
//! PDF régénéré sans changement de créneau ne déclenche rien.

mod diff;
mod messages;

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

pub use self::diff::{diff, Changes};
use self::messages::short_label;
pub use self::messages::{
    change_messages, conflict_messages, conflict_report, describe_homework, published_message,
};
use crate::conflicts::{self, Conflict};
use crate::discord::{Attachment, Button, ButtonStyle, Discord, DiscordError};
use crate::homework::Homework;
use crate::ics;
use crate::model::Timetable;
use crate::users::{User, UserStore};

/// Messages privés envoyés en même temps. Assez pour que toute une promo
/// reçoive son emploi du temps dans la même poignée de secondes, assez peu
/// pour rester sous la limite globale de Discord (50 requêtes par seconde,
/// deux par message) ; un 429 éventuel est de toute façon réessayé.
const PARALLEL_DELIVERIES: usize = 20;

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
    file: Attachment,
    users: Vec<User>,
}

/// Envoie les messages privés aux personnes abonnées.
#[derive(Clone)]
pub struct Notifier {
    discord: Discord,
    users: Arc<UserStore>,
    /// Comptes Discord prévenus des salles réservées en double (enseignants,
    /// administration) ; vide, rien n'est cherché ni envoyé.
    conflict_recipients: Arc<[String]>,
}

impl Notifier {
    pub fn new(discord: Discord, users: Arc<UserStore>) -> Self {
        Self { discord, users, conflict_recipients: Arc::from([]) }
    }

    /// Prévient ces comptes Discord des salles réservées en double.
    pub fn with_conflict_recipients(mut self, ids: Vec<String>) -> Self {
        self.conflict_recipients = ids.into();
        self
    }

    /// Une nouvelle semaine : chaque abonné de l'année reçoit sa semaine.
    ///
    /// `others` : les semaines des autres années aux mêmes dates, dont les
    /// salles sont partagées avec celle-ci. Les salles réservées en double sont
    /// signalées aux destinataires des conflits, pas aux abonnés.
    pub async fn week_published(&self, timetable: &Timetable, others: &[&Timetable]) {
        let mut batches = Vec::new();
        for (group, users) in by_group(self.users.subscribers(timetable.year).await) {
            if !timetable.courses.iter().any(|course| course.concerns(&group)) {
                continue;
            }
            let messages = vec![published_message(timetable, &group)];
            batches.push(Batch { file: calendar(timetable, &group), messages, users });
        }
        self.deliver_all(batches).await;
        self.report_conflicts(timetable, others, None).await;
    }

    /// Une semaine republiée : seuls les groupes dont un cours a bougé sont prévenus.
    ///
    /// Les salles doubles réservées par cette version et pas par la précédente
    /// sont signalées aux destinataires des conflits, avec les autres années
    /// (`others`).
    pub async fn week_changed(&self, before: &Timetable, after: &Timetable, others: &[&Timetable]) {
        let mut batches = Vec::new();
        for (group, users) in by_group(self.users.subscribers(after.year).await) {
            let changes = diff(before, after, &group);
            if changes.is_empty() {
                continue;
            }
            info!(year = after.year, week = after.week, group, "changements signalés");
            let messages = change_messages(after, &group, &changes);
            batches.push(Batch { file: calendar(after, &group), messages, users });
        }
        self.deliver_all(batches).await;
        self.report_conflicts(after, others, Some(before)).await;
    }

    /// Envoie aux destinataires des conflits les salles doubles que `timetable`
    /// réserve avec `others`, sauf celles que `previous` réservait déjà.
    async fn report_conflicts(
        &self,
        timetable: &Timetable,
        others: &[&Timetable],
        previous: Option<&Timetable>,
    ) {
        if self.conflict_recipients.is_empty() {
            return;
        }
        let found = new_conflicts(timetable, others, previous);
        if found.is_empty() {
            return;
        }
        info!(
            year = timetable.year,
            week = timetable.week,
            conflicts = found.len(),
            "salles doubles signalées"
        );
        self.send_to_recipients(conflict_messages(&found.iter().collect::<Vec<_>>())).await;
    }

    /// Envoie aux destinataires des conflits toutes les salles doubles de ces
    /// semaines, qu'elles soient nouvelles ou non : un conflit déjà présent au
    /// démarrage n'est jamais « nouveau » pour [`Self::week_changed`].
    pub async fn report_existing_conflicts(&self, timetables: &[&Timetable]) {
        if self.conflict_recipients.is_empty() {
            return;
        }
        let found = conflicts::find(timetables);
        info!(weeks = timetables.len(), conflicts = found.len(), "état des salles doubles");
        self.send_to_recipients(conflict_report(&found.iter().collect::<Vec<_>>())).await;
    }

    async fn send_to_recipients(&self, messages: Vec<String>) {
        if messages.is_empty() {
            return;
        }
        for id in self.conflict_recipients.iter() {
            self.deliver(id, &messages, None).await;
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
                    notifier.deliver(&user.id, &content.0, Some(&content.1)).await
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
    async fn deliver(&self, user: &str, messages: &[String], file: Option<&Attachment>) -> bool {
        for (index, message) in messages.iter().enumerate() {
            let attachment = file.filter(|_| index + 1 == messages.len());
            if let Err(error) = self.discord.send_dm(user, message, attachment).await {
                warn!(%error, user, "message privé impossible");
                return false;
            }
        }
        debug!(user, messages = messages.len(), "message privé envoyé");
        true
    }
}

/// Cours, semaines et devoirs partagés par les tests des sous-modules.
#[cfg(test)]
mod fixtures {
    use time::macros::date;

    use crate::homework::Homework;
    use crate::model::{ClockTime, Course, CourseKind, Period, Resource, Timetable};

    pub fn course(start: i64, group: &str, room: &str) -> Course {
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

    pub fn week(courses: Vec<Course>) -> Timetable {
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

    pub fn homework(title: &str) -> Homework {
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

    pub fn overlapping_week() -> Timetable {
        let mut other = course(660, "G2A", "104");
        other.teacher = Some("Martin P.".into());
        other.resource = Some(Resource { code: "R1.02".into(), name: None });
        week(vec![course(600, "G1A", "104"), other])
    }

    /// Une semaine de l'année `year`, avec un cours de `group` dans la salle 104.
    pub fn other_year(year: u8, group: &str) -> Timetable {
        let mut other = course(660, group, "104");
        other.teacher = Some("Martin P.".into());
        other.resource = Some(Resource { code: "R3.02".into(), name: None });
        Timetable { year, ..week(vec![other]) }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{course, homework, other_year, overlapping_week, week};
    use super::*;

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
            conflict_recipients: Vec::new(),
            admins: Vec::new(),
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

    /// Le banc d'essai, avec un destinataire des conflits (`99`) en plus des abonnés.
    fn with_recipients(mut bench: Bench, ids: &[&str]) -> Bench {
        bench.notifier =
            bench.notifier.with_conflict_recipients(ids.iter().map(|id| (*id).to_owned()).collect());
        bench
    }

    /// La conversation privée de `id` porte le numéro `channel`, distinct de
    /// celle des abonnés (`900`) ; `times` messages y sont attendus.
    async fn recipient(bench: &Bench, id: &str, channel: &str, status: u16, times: u64) {
        Mock::given(method("POST"))
            .and(path("/users/@me/channels"))
            .and(body_string_contains(format!("\"recipient_id\":\"{id}\"")))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(r#"{{"id":"{channel}"}}"#)))
            .with_priority(1)
            .mount(&bench.server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("/channels/{channel}/messages")))
            .and(body_string_contains("Salle réservée par plusieurs cours"))
            .respond_with(ResponseTemplate::new(status).set_body_string("{}"))
            .expect(times)
            .mount(&bench.server)
            .await;
    }

    /// Les abonnés reçoivent `times` messages, aucun ne parle de conflit.
    async fn subscribers_without_conflicts(bench: &Bench, times: u64) {
        Mock::given(method("POST"))
            .and(path("/channels/900/messages"))
            .and(body_string_contains("Salle réservée"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(0)
            .mount(&bench.server)
            .await;
        messages(bench, 200, times).await;
    }

    #[tokio::test]
    async fn sends_conflicts_to_the_recipients_only() {
        let bench = with_recipients(bench().await, &["99"]);
        recipient(&bench, "99", "990", 200, 1).await;
        // Ada et Bob reçoivent l'annonce de leur semaine, sans le conflit.
        subscribers_without_conflicts(&bench, 2).await;
        bench.notifier.week_published(&overlapping_week(), &[]).await;
    }

    /// Le destinataire `99` reçoit `times` états des lieux.
    async fn report_recipient(bench: &Bench, times: u64) {
        Mock::given(method("POST"))
            .and(path("/users/@me/channels"))
            .and(body_string_contains("\"recipient_id\":\"99\""))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"990"}"#))
            .with_priority(1)
            .mount(&bench.server)
            .await;
        Mock::given(method("POST"))
            .and(path("/channels/990/messages"))
            .and(body_string_contains("état actuel"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(times)
            .mount(&bench.server)
            .await;
    }

    #[tokio::test]
    async fn reports_the_conflicts_already_there_once() {
        let bench = with_recipients(bench().await, &["99"]);
        report_recipient(&bench, 1).await;
        // Ni les abonnés ni personne d'autre n'est dérangé.
        subscribers_without_conflicts(&bench, 0).await;
        // Le conflit existe dans cette version : `week_changed` ne le dirait pas.
        let timetable = overlapping_week();
        bench.notifier.report_existing_conflicts(&[&timetable]).await;
    }

    #[tokio::test]
    async fn reports_the_conflicts_of_several_weeks_in_one_message() {
        let bench = with_recipients(bench().await, &["99"]);
        report_recipient(&bench, 1).await;
        let mut next = overlapping_week();
        next.week = 4;
        for course in &mut next.courses {
            course.date += time::Duration::days(7);
        }
        bench.notifier.report_existing_conflicts(&[&overlapping_week(), &next]).await;
    }

    #[tokio::test]
    async fn reports_nothing_when_no_room_is_booked_twice() {
        let bench = with_recipients(bench().await, &["99"]);
        report_recipient(&bench, 0).await;
        let timetable = week(vec![course(600, "G1A", "104"), course(600, "G2A", "105")]);
        bench.notifier.report_existing_conflicts(&[&timetable]).await;
    }

    #[tokio::test]
    async fn reports_no_existing_conflict_without_recipients() {
        let bench = bench().await;
        messages(&bench, 200, 0).await;
        bench.notifier.report_existing_conflicts(&[&overlapping_week()]).await;
    }

    #[tokio::test]
    async fn reports_nothing_without_recipients() {
        let bench = bench().await;
        subscribers_without_conflicts(&bench, 2).await;
        bench.notifier.week_published(&overlapping_week(), &[]).await;
    }

    #[tokio::test]
    async fn reports_a_new_conflict_even_when_no_subscriber_course_moved() {
        let bench = with_recipients(bench().await, &["99"]);
        recipient(&bench, "99", "990", 200, 1).await;
        // G2A, sans abonné, déplace un cours dans la salle de G1A : Ada et Bob
        // n'ont rien à apprendre, la salle est pourtant prise deux fois.
        subscribers_without_conflicts(&bench, 0).await;
        let before = week(vec![course(600, "G1A", "104"), course(600, "G2A", "105")]);
        let mut moved = course(660, "G2A", "104");
        moved.teacher = Some("Martin P.".into());
        let after = week(vec![course(600, "G1A", "104"), moved]);
        bench.notifier.week_changed(&before, &after, &[]).await;
    }

    #[tokio::test]
    async fn keeps_sending_to_the_other_recipients_after_a_failure() {
        let bench = with_recipients(bench().await, &["98", "99"]);
        // Messages privés fermés chez 98 : 99 est prévenu quand même.
        recipient(&bench, "98", "980", 403, 1).await;
        recipient(&bench, "99", "990", 200, 1).await;
        accept_the_rest(&bench).await;
        bench.notifier.week_published(&overlapping_week(), &[]).await;
    }

    #[tokio::test]
    async fn reports_a_conflict_with_another_year() {
        let bench = with_recipients(bench().await, &["99"]);
        Mock::given(method("POST"))
            .and(path("/users/@me/channels"))
            .and(body_string_contains("\"recipient_id\":\"99\""))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"990"}"#))
            .with_priority(1)
            .mount(&bench.server)
            .await;
        Mock::given(method("POST"))
            .and(path("/channels/990/messages"))
            .and(body_string_contains("BUT 1 · TP R1.05 · G1A"))
            .and(body_string_contains("BUT 2 · TP R3.02 · G1A"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&bench.server)
            .await;
        subscribers_without_conflicts(&bench, 2).await;

        let first = week(vec![course(600, "G1A", "104")]);
        bench.notifier.week_published(&first, &[&other_year(2, "G1A")]).await;
    }

    #[tokio::test]
    async fn ignores_a_week_of_the_same_year_among_the_others() {
        let bench = with_recipients(bench().await, &["99"]);
        messages(&bench, 200, 0).await;
        let mut same_year = other_year(1, "G2A");
        same_year.week = 4;
        // Même année : c'est une autre version, pas une autre promo — aucun conflit.
        let before = week(vec![course(600, "G1A", "104")]);
        bench.notifier.week_changed(&before, &before, &[&same_year]).await;
    }

    #[tokio::test]
    async fn does_not_repeat_a_conflict_already_known() {
        let bench = with_recipients(bench().await, &["99"]);
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
