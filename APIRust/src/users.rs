//! Comptes des personnes connectées avec Discord.
//!
//! Quelques centaines d'étudiants au plus : un fichier JSON suffit. Il est
//! réécrit en entier à chaque modification, via un fichier temporaire renommé,
//! pour ne jamais laisser une version à moitié écrite.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    /// Identifiant Discord.
    pub id: String,
    pub name: String,
    /// Empreinte de l'avatar Discord, s'il en a un.
    pub avatar: Option<String>,
    /// Classe suivie : année et demi-groupe (`G1a`).
    pub year: Option<u8>,
    pub group: Option<String>,
    /// Recevoir les emplois du temps et leurs changements en message privé.
    pub notify: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl User {
    pub fn class(&self) -> Option<(u8, &str)> {
        Some((self.year?, self.group.as_deref()?))
    }
}

pub struct UserStore {
    path: PathBuf,
    users: RwLock<BTreeMap<String, User>>,
}

impl UserStore {
    /// Ouvre le fichier des comptes, créé au premier enregistrement.
    pub async fn open(dir: &Path) -> io::Result<Self> {
        let path = dir.join("users.json");
        let users = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice::<Vec<User>>(&bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
                .into_iter()
                .map(|user| (user.id.clone(), user))
                .collect(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error),
        };
        Ok(Self { path, users: RwLock::new(users) })
    }

    pub async fn get(&self, id: &str) -> Option<User> {
        self.users.read().await.get(id).cloned()
    }

    /// Personnes à prévenir pour une année, avec leur demi-groupe.
    pub async fn subscribers(&self, year: u8) -> Vec<User> {
        self.users
            .read()
            .await
            .values()
            .filter(|user| user.notify && user.year == Some(year) && user.group.is_some())
            .cloned()
            .collect()
    }

    /// Crée le compte à la première connexion, ou rafraîchit nom et avatar.
    pub async fn sign_in(&self, id: &str, name: &str, avatar: Option<String>) -> io::Result<User> {
        let mut users = self.users.write().await;
        let user = users.entry(id.to_owned()).or_insert_with(|| User {
            id: id.to_owned(),
            name: name.to_owned(),
            avatar: None,
            year: None,
            group: None,
            notify: true,
            created_at: OffsetDateTime::now_utc(),
        });
        user.name = name.to_owned();
        user.avatar = avatar;
        let user = user.clone();
        self.save(&users).await?;
        Ok(user)
    }

    /// Applique une modification au compte, s'il existe.
    pub async fn update(&self, id: &str, change: impl FnOnce(&mut User)) -> io::Result<Option<User>> {
        let mut users = self.users.write().await;
        let Some(user) = users.get_mut(id) else { return Ok(None) };
        change(user);
        let user = user.clone();
        self.save(&users).await?;
        Ok(Some(user))
    }

    pub async fn delete(&self, id: &str) -> io::Result<bool> {
        let mut users = self.users.write().await;
        let removed = users.remove(id).is_some();
        if removed {
            self.save(&users).await?;
        }
        Ok(removed)
    }

    async fn save(&self, users: &BTreeMap<String, User>) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let list: Vec<&User> = users.values().collect();
        let bytes = serde_json::to_vec_pretty(&list).map_err(io::Error::other)?;
        let temporary = self.path.with_extension("json.tmp");
        tokio::fs::write(&temporary, bytes).await?;
        tokio::fs::rename(&temporary, &self.path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn keeps_accounts_across_restarts() {
        let dir = tempfile::tempdir().expect("répertoire temporaire");

        let store = UserStore::open(dir.path()).await.expect("fichier absent accepté");
        store.sign_in("42", "Ada", None).await.expect("compte créé");
        store
            .update("42", |user| {
                user.year = Some(1);
                user.group = Some("G1a".into());
            })
            .await
            .expect("écriture")
            .expect("compte existant");

        let reopened = UserStore::open(dir.path()).await.expect("fichier relu");
        let user = reopened.get("42").await.expect("compte conservé");
        assert_eq!(user.class(), Some((1, "G1a")));
        assert!(user.notify, "les messages privés sont actifs par défaut");
        assert_eq!(reopened.subscribers(1).await.len(), 1);
        assert!(reopened.subscribers(2).await.is_empty());

        // Une reconnexion ne perd pas la classe choisie.
        let again = reopened.sign_in("42", "Ada L.", Some("abc".into())).await.expect("reconnexion");
        assert_eq!(again.class(), Some((1, "G1a")));
        assert_eq!(again.name, "Ada L.");

        assert!(reopened.delete("42").await.expect("suppression"));
        assert!(reopened.get("42").await.is_none());
    }
}
