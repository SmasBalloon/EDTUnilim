//! Cache des emplois du temps.
//!
//! Trois niveaux : les emplois du temps déjà analysés restent en mémoire, les
//! PDF téléchargés restent sur disque — ce qui permet de continuer à répondre
//! si le serveur du département est indisponible — et le reste vient du réseau.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use time::OffsetDateTime;
use tracing::{debug, info, warn};

use crate::config::{Config, YEARS};
use crate::error::ApiError;
use crate::model::Timetable;
use crate::pdf;
use crate::upstream::{self, Fetch, Upstream, Validators};

/// Un emploi du temps analysé, avec de quoi vérifier s'il est encore à jour.
#[derive(Debug)]
pub struct Entry {
    pub timetable: Timetable,
    pub fetched_at: OffsetDateTime,
    validators: Validators,
}

pub struct Store {
    upstream: Upstream,
    cache_dir: PathBuf,
    weeks: RwLock<HashMap<u8, Arc<Vec<u8>>>>,
    timetables: RwLock<HashMap<(u8, u8), Arc<Entry>>>,
}

impl Store {
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        Ok(Self {
            upstream: Upstream::new(config.upstream.clone(), config.upstream_timeout)?,
            cache_dir: config.cache_dir.clone(),
            weeks: RwLock::default(),
            timetables: RwLock::default(),
        })
    }

    /// Semaines publiées pour une année.
    pub async fn weeks(&self, year: u8) -> Result<Arc<Vec<u8>>, ApiError> {
        if !YEARS.contains(&year) {
            return Err(ApiError::UnknownYear(year));
        }
        if let Some(weeks) = self.weeks.read().expect("cache empoisonné").get(&year) {
            return Ok(Arc::clone(weeks));
        }

        let weeks = Arc::new(self.upstream.list_weeks(year).await.map_err(ApiError::UpstreamUnavailable)?);
        self.weeks.write().expect("cache empoisonné").insert(year, Arc::clone(&weeks));
        Ok(weeks)
    }

    /// Emploi du temps d'une semaine, analysé au premier appel.
    pub async fn timetable(&self, year: u8, week: u8) -> Result<Arc<Entry>, ApiError> {
        if !YEARS.contains(&year) {
            return Err(ApiError::UnknownYear(year));
        }
        if let Some(entry) = self.timetables.read().expect("cache empoisonné").get(&(year, week)) {
            return Ok(Arc::clone(entry));
        }

        let document = match self.upstream.fetch_week(year, week, &Validators::default()).await {
            Ok(Fetch::Fresh(document)) => {
                self.write_to_disk(year, week, &document.bytes).await;
                document
            }
            // Le serveur a répondu qu'il ne connaît pas cette semaine.
            Err(error) if error.status() == Some(reqwest::StatusCode::NOT_FOUND) => {
                return Err(ApiError::UnknownWeek { year, week })
            }
            // Aucun validateur n'a été envoyé : un 304 serait incohérent.
            Ok(Fetch::Unchanged) => {
                warn!(year, week, "réponse amont incohérente, repli sur le cache disque");
                self.read_cached(year, week, ApiError::UpstreamUnexpected).await?
            }
            Err(error) => {
                warn!(%error, year, week, "téléchargement impossible, repli sur le cache disque");
                self.read_cached(year, week, ApiError::UpstreamUnavailable(error)).await?
            }
        };

        self.store(year, week, document)
    }

    /// Revalide tout ce qui est en cache et réanalyse ce qui a changé.
    pub async fn refresh(&self) {
        for year in YEARS {
            match self.upstream.list_weeks(year).await {
                Ok(weeks) => {
                    self.weeks.write().expect("cache empoisonné").insert(year, Arc::new(weeks));
                }
                Err(error) => warn!(%error, year, "index des semaines indisponible"),
            }
        }

        let cached: Vec<((u8, u8), Validators)> = {
            let timetables = self.timetables.read().expect("cache empoisonné");
            timetables.iter().map(|(key, entry)| (*key, entry.validators.clone())).collect()
        };

        for ((year, week), validators) in cached {
            match self.upstream.fetch_week(year, week, &validators).await {
                Ok(Fetch::Unchanged) => debug!(year, week, "emploi du temps inchangé"),
                Ok(Fetch::Fresh(document)) => {
                    self.write_to_disk(year, week, &document.bytes).await;
                    match self.store(year, week, document) {
                        Ok(_) => info!(year, week, "emploi du temps mis à jour"),
                        Err(error) => warn!(%error, year, week, "nouvelle version illisible"),
                    }
                }
                Err(error) => warn!(%error, year, week, "revalidation impossible"),
            }
        }
    }

    fn store(&self, year: u8, week: u8, document: upstream::Document) -> Result<Arc<Entry>, ApiError> {
        let timetable = pdf::parse(&document.bytes, year, week).map_err(ApiError::Unreadable)?;
        let entry = Arc::new(Entry {
            timetable,
            fetched_at: OffsetDateTime::now_utc(),
            validators: document.validators,
        });
        self.timetables.write().expect("cache empoisonné").insert((year, week), Arc::clone(&entry));
        Ok(entry)
    }

    fn disk_path(&self, year: u8, week: u8) -> PathBuf {
        self.cache_dir.join(format!("A{year}")).join(upstream::week_file_name(year, week))
    }

    async fn write_to_disk(&self, year: u8, week: u8, bytes: &[u8]) {
        let path = self.disk_path(year, week);
        let written = match path.parent() {
            Some(parent) => tokio::fs::create_dir_all(parent).await.and(tokio::fs::write(&path, bytes).await),
            None => tokio::fs::write(&path, bytes).await,
        };
        if let Err(error) = written {
            warn!(%error, path = %path.display(), "écriture du cache disque impossible");
        }
    }

    /// Relit le PDF déjà téléchargé, ou rend l'erreur amont s'il est absent.
    async fn read_cached(
        &self,
        year: u8,
        week: u8,
        upstream_error: ApiError,
    ) -> Result<upstream::Document, ApiError> {
        match tokio::fs::read(self.disk_path(year, week)).await {
            Ok(bytes) => Ok(upstream::Document { bytes, validators: Validators::default() }),
            Err(_) => Err(upstream_error),
        }
    }
}
