//! Salons Discord où le bot annonce les emplois du temps d'une année.
//!
//! Un salon s'abonne avec `/salon ajouter` : chaque nouvelle semaine y est
//! annoncée avec son PDF, chaque changement avec l'image de la semaine, le
//! PDF et la liste de ce qui a bougé. Un salon peut suivre plusieurs années.
//!
//! Une poignée de salons au plus : un fichier JSON suffit, réécrit en entier
//! à chaque modification ([`persist`]).

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::RwLock;

use crate::persist;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelSubscription {
    pub guild_id: String,
    pub channel_id: String,
    pub year: u8,
    /// Rôle mentionné à chaque annonce ; celui du serveur lui-même est `@everyone`.
    pub mention: Option<String>,
    /// Qui a abonné le salon, pour s'y retrouver.
    pub added_by: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl ChannelSubscription {
    /// `<@&123>`, `@everyone`, ou rien.
    pub fn mention_text(&self) -> Option<String> {
        self.mention.as_ref().map(|role| {
            if *role == self.guild_id {
                "@everyone".to_owned()
            } else {
                format!("<@&{role}>")
            }
        })
    }
}

pub struct ChannelStore {
    path: PathBuf,
    items: RwLock<Vec<ChannelSubscription>>,
}

impl ChannelStore {
    /// Ouvre le fichier des salons, créé au premier abonnement.
    pub async fn open(dir: &Path) -> io::Result<Self> {
        let path = dir.join("channels.json");
        let items = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(Self { path, items: RwLock::new(items) })
    }

    /// Les salons qui suivent cette année.
    pub async fn for_year(&self, year: u8) -> Vec<ChannelSubscription> {
        self.items.read().await.iter().filter(|item| item.year == year).cloned().collect()
    }

    /// Les salons abonnés d'un serveur.
    pub async fn in_guild(&self, guild_id: &str) -> Vec<ChannelSubscription> {
        let mut found: Vec<ChannelSubscription> =
            self.items.read().await.iter().filter(|item| item.guild_id == guild_id).cloned().collect();
        found.sort_by(|a, b| (&a.channel_id, a.year).cmp(&(&b.channel_id, b.year)));
        found
    }

    /// Abonne un salon à une année ; un abonnement existant est remplacé
    /// (pour changer la mention, par exemple).
    pub async fn set(&self, subscription: ChannelSubscription) -> io::Result<()> {
        let mut items = self.items.write().await;
        let mut next = items.clone();
        next.retain(|item| !(item.channel_id == subscription.channel_id && item.year == subscription.year));
        next.push(subscription);
        self.save(&next).await?;
        *items = next;
        Ok(())
    }

    /// Désabonne un salon d'une année, ou de toutes sans année ; renvoie le
    /// nombre d'abonnements retirés.
    pub async fn remove(&self, channel_id: &str, year: Option<u8>) -> io::Result<usize> {
        let mut items = self.items.write().await;
        let mut next = items.clone();
        next.retain(|item| !(item.channel_id == channel_id && year.is_none_or(|year| item.year == year)));
        let removed = items.len() - next.len();
        if removed > 0 {
            self.save(&next).await?;
            *items = next;
        }
        Ok(removed)
    }

    async fn save(&self, items: &[ChannelSubscription]) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(items).map_err(io::Error::other)?;
        persist::write_with_backup(&self.path, &bytes).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subscription(channel: &str, year: u8) -> ChannelSubscription {
        ChannelSubscription {
            guild_id: "7".into(),
            channel_id: channel.into(),
            year,
            mention: None,
            added_by: "42".into(),
            created_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[tokio::test]
    async fn subscribes_replaces_and_removes_channels() {
        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let store = ChannelStore::open(dir.path()).await.expect("ouvert");
        store.set(subscription("100", 1)).await.expect("abonné");
        store.set(subscription("100", 3)).await.expect("abonné");
        let mut with_mention = subscription("100", 3);
        with_mention.mention = Some("7".into());
        store.set(with_mention).await.expect("remplacé");

        assert_eq!(store.for_year(3).await.len(), 1, "un abonnement par salon et par année");
        assert_eq!(store.for_year(3).await[0].mention_text().as_deref(), Some("@everyone"));
        assert_eq!(store.in_guild("7").await.len(), 2);
        assert!(store.in_guild("8").await.is_empty());

        // Relu depuis le disque.
        let reopened = ChannelStore::open(dir.path()).await.expect("rouvert");
        assert_eq!(reopened.in_guild("7").await.len(), 2);

        assert_eq!(reopened.remove("100", Some(1)).await.expect("retiré"), 1);
        assert_eq!(reopened.remove("100", Some(1)).await.expect("déjà retiré"), 0);
        reopened.set(subscription("100", 2)).await.expect("abonné");
        assert_eq!(reopened.remove("100", None).await.expect("tout retiré"), 2);
        assert!(reopened.in_guild("7").await.is_empty());
    }

    #[test]
    fn mentions_a_role_or_everyone() {
        let mut item = subscription("100", 1);
        assert_eq!(item.mention_text(), None);
        item.mention = Some("55".into());
        assert_eq!(item.mention_text().as_deref(), Some("<@&55>"));
    }
}
