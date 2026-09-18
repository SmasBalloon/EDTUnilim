//! Les devoirs : ce qu'il y a à faire, pour soi ou pour sa classe.
//!
//! Deux idées gouvernent ce module.
//!
//! **La portée se choisit.** Un devoir peut n'être que pour soi, ou pour son
//! demi-groupe (le TP), son groupe (le TD), ou toute l'année (l'amphi) — c'est
//! la personne qui le note qui sait de quoi il retourne.
//!
//! **Un devoir de classe engage les autres**, donc il n'est officiel qu'une
//! fois qu'une deuxième personne l'a confirmé. En attendant, la classe le voit
//! marqué « à confirmer » : il faut bien que quelqu'un puisse le lire pour
//! l'accepter, mais personne ne doit le prendre pour argent comptant.
//!
//! Comme les comptes, tout tient dans un fichier JSON réécrit en entier :
//! quelques centaines de devoirs par promotion, ce n'est pas une base.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use tokio::sync::RwLock;

use crate::model::{matches_group, ClockTime};
use crate::session::nonce;
use crate::users::User;

/// Qui voit un devoir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Audience {
    /// Personne d'autre que son auteur.
    Personal,
    /// La classe : un groupe (`G1`), un demi-groupe (`G1a`), ou toute l'année.
    Class {
        #[serde(skip_serializing_if = "Option::is_none")]
        group: Option<String>,
    },
}

impl Audience {
    pub fn is_personal(&self) -> bool {
        matches!(self, Audience::Personal)
    }

    /// « pour toi », « pour G1A », « pour toute l'année ».
    pub fn label(&self, year: u8) -> String {
        match self {
            Audience::Personal => "pour toi".to_owned(),
            Audience::Class { group: None } => format!("pour tout le BUT {year}"),
            Audience::Class { group: Some(group) } => format!("pour {}", group.to_uppercase()),
        }
    }
}

/// Le créneau auquel un devoir est rattaché.
///
/// Un devoir peut naître avant que la semaine correspondante soit publiée : il
/// se rattache alors plus tard, quand le cours paraît enfin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseRef {
    pub year: u8,
    pub week: u8,
    #[serde(with = "crate::model::iso_date")]
    pub date: Date,
    pub start: ClockTime,
}

/// Un devoir à faire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Homework {
    pub id: String,
    /// Identifiant Discord de la personne qui l'a noté.
    pub author: String,
    /// Son nom au moment où elle l'a noté, pour l'afficher sans relire les comptes.
    pub author_name: String,
    /// Année du BUT concernée.
    pub year: u8,
    pub audience: Audience,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// Code de la ressource (`R1.05`), quand le devoir en relève.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// Date de rendu.
    #[serde(skip_serializing_if = "Option::is_none", with = "crate::model::opt_iso_date", default)]
    pub due: Option<Date>,
    /// Créneau de rattachement, s'il est connu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course: Option<CourseRef>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    /// Qui a confirmé ce devoir de classe — tant que c'est vide, il est en attente.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_name: Option<String>,
    /// Les personnes qui l'ont coché « fait ». Chacune pour soi.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub done: Vec<String>,
}

impl Homework {
    /// Un devoir de classe n'est officiel qu'une fois confirmé par quelqu'un
    /// d'autre que son auteur ; un devoir personnel n'a personne à convaincre.
    pub fn is_pending(&self) -> bool {
        !self.audience.is_personal() && self.confirmed_by.is_none()
    }

    pub fn is_done_by(&self, user_id: &str) -> bool {
        self.done.iter().any(|id| id == user_id)
    }

    /// Cette personne peut-elle voir ce devoir ?
    pub fn visible_to(&self, user: &User) -> bool {
        match &self.audience {
            Audience::Personal => self.author == user.id,
            Audience::Class { group } => {
                user.year == Some(self.year)
                    && match (group, user.group.as_deref()) {
                        // Sans groupe, le devoir s'adresse à toute l'année.
                        (None, _) => true,
                        (Some(wanted), Some(theirs)) => matches_group(wanted, theirs),
                        (Some(_), None) => false,
                    }
            }
        }
    }

    /// Cette personne peut-elle confirmer ce devoir ? Pas son auteur : c'est
    /// tout l'intérêt d'une confirmation.
    pub fn confirmable_by(&self, user: &User) -> bool {
        self.is_pending() && self.author != user.id && self.visible_to(user)
    }

    /// Seule la personne qui l'a noté peut le corriger ou l'effacer.
    pub fn editable_by(&self, user: &User) -> bool {
        self.author == user.id
    }

    /// `R1.05 — Exercices 3 à 7`
    pub fn summary(&self) -> String {
        match &self.resource {
            Some(code) => format!("{code} — {}", self.title),
            None => self.title.clone(),
        }
    }
}

/// Ce qu'il faut pour noter un devoir.
#[derive(Debug, Clone)]
pub struct NewHomework {
    pub year: u8,
    pub audience: Audience,
    pub title: String,
    pub details: Option<String>,
    pub resource: Option<String>,
    pub due: Option<Date>,
    pub course: Option<CourseRef>,
}

/// Les retouches possibles. `None` laisse le champ tel quel ; pour vider une
/// date ou un rattachement, on passe `Some(None)`.
#[derive(Debug, Clone, Default)]
pub struct Edit {
    pub title: Option<String>,
    pub details: Option<Option<String>>,
    pub resource: Option<Option<String>>,
    pub due: Option<Option<Date>>,
    pub course: Option<Option<CourseRef>>,
    pub audience: Option<Audience>,
}

pub struct HomeworkStore {
    path: PathBuf,
    items: RwLock<BTreeMap<String, Homework>>,
}

impl HomeworkStore {
    /// Ouvre le fichier des devoirs, créé au premier enregistrement.
    pub async fn open(dir: &Path) -> io::Result<Self> {
        let path = dir.join("homework.json");
        let items = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice::<Vec<Homework>>(&bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
                .into_iter()
                .map(|item| (item.id.clone(), item))
                .collect(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error),
        };
        Ok(Self { path, items: RwLock::new(items) })
    }

    pub async fn get(&self, id: &str) -> Option<Homework> {
        self.items.read().await.get(id).cloned()
    }

    /// Les devoirs qu'une personne voit, du plus proche au plus lointain.
    pub async fn visible_to(&self, user: &User) -> Vec<Homework> {
        let mut found: Vec<Homework> =
            self.items.read().await.values().filter(|item| item.visible_to(user)).cloned().collect();
        found.sort_by(|a, b| {
            // Sans date de rendu, le devoir passe après ceux qui en ont une.
            let key = |item: &Homework| (item.due.is_none(), item.due, item.created_at);
            key(a).cmp(&key(b))
        });
        found
    }

    /// Tous les devoirs, pour les rappels.
    pub async fn all(&self) -> Vec<Homework> {
        self.items.read().await.values().cloned().collect()
    }

    pub async fn create(&self, author: &User, new: NewHomework) -> io::Result<Homework> {
        let item = Homework {
            id: nonce(),
            author: author.id.clone(),
            author_name: author.name.clone(),
            year: new.year,
            audience: new.audience,
            title: new.title,
            details: new.details,
            resource: new.resource,
            due: new.due,
            course: new.course,
            created_at: OffsetDateTime::now_utc(),
            confirmed_by: None,
            confirmed_name: None,
            done: Vec::new(),
        };
        let mut items = self.items.write().await;
        items.insert(item.id.clone(), item.clone());
        self.save(&items).await?;
        Ok(item)
    }

    /// Applique une modification, si le devoir existe.
    pub async fn update(
        &self,
        id: &str,
        change: impl FnOnce(&mut Homework),
    ) -> io::Result<Option<Homework>> {
        let mut items = self.items.write().await;
        let Some(item) = items.get_mut(id) else { return Ok(None) };
        change(item);
        let item = item.clone();
        self.save(&items).await?;
        Ok(Some(item))
    }

    pub async fn delete(&self, id: &str) -> io::Result<bool> {
        let mut items = self.items.write().await;
        let removed = items.remove(id).is_some();
        if removed {
            self.save(&items).await?;
        }
        Ok(removed)
    }

    /// Oublie les devoirs dont la date de rendu est passée depuis longtemps :
    /// la liste sert à travailler, pas à archiver l'année.
    pub async fn forget_before(&self, day: Date) -> io::Result<usize> {
        let mut items = self.items.write().await;
        let before = items.len();
        items.retain(|_, item| item.due.is_none_or(|due| due >= day));
        let removed = before - items.len();
        if removed > 0 {
            self.save(&items).await?;
        }
        Ok(removed)
    }

    async fn save(&self, items: &BTreeMap<String, Homework>) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let list: Vec<&Homework> = items.values().collect();
        let bytes = serde_json::to_vec_pretty(&list).map_err(io::Error::other)?;
        let temporary = self.path.with_extension("json.tmp");
        tokio::fs::write(&temporary, bytes).await?;
        tokio::fs::rename(&temporary, &self.path).await
    }
}

/// Applique une retouche à un devoir.
pub fn apply(item: &mut Homework, edit: Edit) {
    if let Some(title) = edit.title {
        item.title = title;
    }
    if let Some(details) = edit.details {
        item.details = details;
    }
    if let Some(resource) = edit.resource {
        item.resource = resource;
    }
    if let Some(due) = edit.due {
        item.due = due;
    }
    if let Some(course) = edit.course {
        item.course = course;
    }
    if let Some(audience) = edit.audience {
        // Un devoir personnel qui s'ouvre à la classe redevient à confirmer :
        // ce qui n'engageait que soi engage maintenant les autres.
        if audience != item.audience && !audience.is_personal() {
            item.confirmed_by = None;
            item.confirmed_name = None;
        }
        item.audience = audience;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    fn user(id: &str, year: u8, group: &str) -> User {
        User {
            id: id.into(),
            name: format!("Personne {id}"),
            avatar: None,
            year: Some(year),
            group: Some(group.into()),
            notify: true,
            created_at: OffsetDateTime::now_utc(),
        }
    }

    fn new(audience: Audience) -> NewHomework {
        NewHomework {
            year: 1,
            audience,
            title: "Exercices 3 à 7".into(),
            details: None,
            resource: Some("R1.05".into()),
            due: Some(date!(2026 - 09 - 21)),
            course: None,
        }
    }

    #[tokio::test]
    async fn a_personal_note_stays_personal() {
        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let store = HomeworkStore::open(dir.path()).await.expect("fichier absent accepté");
        let ada = user("1", 1, "G1a");
        let bob = user("2", 1, "G1a");

        store.create(&ada, new(Audience::Personal)).await.expect("devoir noté");

        assert_eq!(store.visible_to(&ada).await.len(), 1);
        assert!(store.visible_to(&bob).await.is_empty(), "un devoir perso ne se partage pas");
    }

    #[tokio::test]
    async fn a_class_note_waits_for_a_second_pair_of_eyes() {
        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let store = HomeworkStore::open(dir.path()).await.expect("fichier absent accepté");
        let ada = user("1", 1, "G1a");
        let bob = user("2", 1, "G1b");

        let item = store
            .create(&ada, new(Audience::Class { group: Some("G1".into()) }))
            .await
            .expect("devoir noté");

        // Il est lisible par la classe, mais marqué en attente, et son auteur
        // ne peut pas se confirmer lui-même.
        assert!(item.is_pending());
        assert!(item.visible_to(&bob));
        assert!(!item.confirmable_by(&ada));
        assert!(item.confirmable_by(&bob));

        let confirmed = store
            .update(&item.id, |item| {
                item.confirmed_by = Some(bob.id.clone());
                item.confirmed_name = Some(bob.name.clone());
            })
            .await
            .expect("écriture")
            .expect("devoir existant");
        assert!(!confirmed.is_pending());
        assert!(!confirmed.confirmable_by(&bob), "on ne confirme pas deux fois");
    }

    #[tokio::test]
    async fn the_audience_decides_who_reads() {
        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let store = HomeworkStore::open(dir.path()).await.expect("fichier absent accepté");
        let ada = user("1", 1, "G1a");

        store.create(&ada, new(Audience::Class { group: Some("G1a".into()) })).await.expect("TP");
        store.create(&ada, new(Audience::Class { group: Some("G1".into()) })).await.expect("TD");
        store.create(&ada, new(Audience::Class { group: None })).await.expect("amphi");

        // Le demi-groupe voisin ne voit pas le TP, mais bien le TD et l'amphi.
        assert_eq!(store.visible_to(&user("2", 1, "G1b")).await.len(), 2);
        // Un autre groupe ne voit que ce qui s'adresse à l'année.
        assert_eq!(store.visible_to(&user("3", 1, "G2a")).await.len(), 1);
        // Une autre année ne voit rien.
        assert!(store.visible_to(&user("4", 2, "G1a")).await.is_empty());
    }

    #[tokio::test]
    async fn survives_a_restart_and_forgets_the_past() {
        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let store = HomeworkStore::open(dir.path()).await.expect("fichier absent accepté");
        let ada = user("1", 1, "G1a");

        let kept = store.create(&ada, new(Audience::Personal)).await.expect("devoir noté");
        let old = NewHomework { due: Some(date!(2026 - 09 - 01)), ..new(Audience::Personal) };
        store.create(&ada, old).await.expect("vieux devoir");

        let reopened = HomeworkStore::open(dir.path()).await.expect("fichier relu");
        assert_eq!(reopened.visible_to(&ada).await.len(), 2);

        assert_eq!(reopened.forget_before(date!(2026 - 09 - 15)).await.expect("ménage"), 1);
        let left = reopened.visible_to(&ada).await;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, kept.id);
    }

    #[tokio::test]
    async fn opening_a_note_to_the_class_asks_for_a_confirmation_again() {
        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let store = HomeworkStore::open(dir.path()).await.expect("fichier absent accepté");
        let ada = user("1", 1, "G1a");
        let item = store.create(&ada, new(Audience::Personal)).await.expect("devoir noté");

        let edit = Edit {
            audience: Some(Audience::Class { group: Some("G1".into()) }),
            ..Edit::default()
        };
        let opened = store
            .update(&item.id, |item| apply(item, edit))
            .await
            .expect("écriture")
            .expect("devoir existant");
        assert!(opened.is_pending());
    }
}
