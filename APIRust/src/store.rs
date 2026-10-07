//! Cache des emplois du temps.
//!
//! Trois niveaux : les emplois du temps déjà analysés restent en mémoire, les
//! PDF téléchargés restent sur disque — ce qui permet de continuer à répondre
//! si le serveur du département est indisponible — et le reste vient du réseau.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use time::OffsetDateTime;
use tracing::{debug, info, warn};

use crate::config::{Config, YEARS};
use crate::error::ApiError;
use crate::model::Timetable;
use crate::notify::Notifier;
use crate::pdf;
use crate::upstream::{self, Fetch, Upstream, Validators};

/// Un emploi du temps analysé, avec de quoi vérifier s'il est encore à jour.
#[derive(Debug)]
pub struct Entry {
    pub timetable: Timetable,
    pub fetched_at: OffsetDateTime,
    validators: Validators,
}

/// Bilan d'une revalidation périodique, pour les journaux.
#[derive(Debug, Default, Clone, Copy)]
pub struct RefreshSummary {
    /// Semaines en cache revérifiées auprès du département.
    pub checked: usize,
    pub updated: usize,
    pub failed: usize,
}

/// Issue de la revalidation d'une semaine.
enum Revalidation {
    Unchanged,
    Updated,
    Failed,
}

/// Délai minimal entre deux revalidations demandées par un client : un bouton
/// martelé ne doit pas se traduire en rafale de requêtes vers le département.
const MIN_REVALIDATION: Duration = Duration::from_secs(30);

pub struct Store {
    upstream: Upstream,
    cache_dir: PathBuf,
    weeks: RwLock<HashMap<u8, Arc<Vec<u8>>>>,
    timetables: RwLock<HashMap<(u8, u8), Arc<Entry>>>,
    /// Dernière revalidation forcée, par index d'année (`None`) ou semaine.
    revalidated: Mutex<HashMap<(u8, Option<u8>), Instant>>,
    /// Sans Discord configuré, aucun changement n'est signalé.
    notifier: Option<Notifier>,
}

impl Store {
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        Ok(Self {
            upstream: Upstream::new(config.upstream.clone(), config.upstream_timeout)?,
            cache_dir: config.cache_dir.clone(),
            weeks: RwLock::default(),
            timetables: RwLock::default(),
            revalidated: Mutex::default(),
            notifier: None,
        })
    }

    /// Prévient les abonnés des semaines publiées et des changements.
    pub fn with_notifier(mut self, notifier: Notifier) -> Self {
        self.notifier = Some(notifier);
        self
    }

    /// La semaine en cours, ou la prochaine publiée, parmi celles en cache.
    pub async fn upcoming_week(&self, year: u8, today: time::Date) -> Option<Arc<Entry>> {
        let weeks = self.weeks(year).await.ok()?;
        let mut best: Option<Arc<Entry>> = None;
        for &week in weeks.iter() {
            let Ok(entry) = self.timetable(year, week).await else { continue };
            let period = &entry.timetable.period;
            if period.end >= today && best.as_ref().is_none_or(|b| period.end < b.timetable.period.end) {
                best = Some(entry);
            }
        }
        best
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
        info!(year, count = weeks.len(), "index des semaines chargé");
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
                info!(year, week, bytes = document.bytes.len(), "emploi du temps téléchargé");
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

        let entry = self.store(year, week, document)?;
        debug!(year, week, courses = entry.timetable.courses.len(), "emploi du temps analysé");
        Ok(entry)
    }

    /// Revalide tout ce qui est en cache et réanalyse ce qui a changé.
    ///
    /// Quand les changements sont signalés, toutes les semaines publiées sont
    /// suivies, et pas seulement celles qu'un client a déjà consultées.
    pub async fn refresh(&self) -> RefreshSummary {
        for year in YEARS {
            let known = self.weeks.read().expect("cache empoisonné").get(&year).cloned();
            self.refresh_weeks(year).await;
            if self.notifier.is_some() {
                self.watch_year(year, known.as_deref()).await;
            }
        }

        let cached: Vec<(u8, u8)> =
            self.timetables.read().expect("cache empoisonné").keys().copied().collect();
        let mut summary = RefreshSummary { checked: cached.len(), ..RefreshSummary::default() };
        for (year, week) in cached {
            match self.refresh_timetable(year, week).await {
                Revalidation::Unchanged => {}
                Revalidation::Updated => summary.updated += 1,
                Revalidation::Failed => summary.failed += 1,
            }
        }
        summary
    }

    /// Revalide à la demande d'un client l'index d'une année, sans attendre
    /// le prochain passage périodique.
    pub async fn revalidate_weeks(&self, year: u8) {
        if YEARS.contains(&year) && self.claim_revalidation(year, None) {
            self.refresh_weeks(year).await;
        }
    }

    /// Revalide à la demande d'un client une semaine déjà en cache.
    pub async fn revalidate_timetable(&self, year: u8, week: u8) {
        if self.claim_revalidation(year, Some(week)) {
            self.refresh_timetable(year, week).await;
        }
    }

    /// Vrai si la revalidation peut partir, faux si une autre vient d'avoir lieu.
    fn claim_revalidation(&self, year: u8, week: Option<u8>) -> bool {
        let now = Instant::now();
        let mut revalidated = self.revalidated.lock().expect("cache empoisonné");
        match revalidated.get(&(year, week)) {
            Some(last) if now.duration_since(*last) < MIN_REVALIDATION => false,
            _ => {
                revalidated.insert((year, week), now);
                true
            }
        }
    }

    async fn refresh_weeks(&self, year: u8) {
        match self.upstream.list_weeks(year).await {
            Ok(weeks) => {
                debug!(year, count = weeks.len(), "index des semaines revalidé");
                self.weeks.write().expect("cache empoisonné").insert(year, Arc::new(weeks));
            }
            Err(error) => warn!(%error, year, "index des semaines indisponible"),
        }
    }

    /// Met en cache les semaines publiées qui n'y sont pas encore, pour que
    /// leurs prochaines versions puissent leur être comparées.
    async fn watch_year(&self, year: u8, known: Option<&Vec<u8>>) {
        let Some(weeks) = self.weeks.read().expect("cache empoisonné").get(&year).cloned() else { return };
        for &week in weeks.iter() {
            if self.timetables.read().expect("cache empoisonné").contains_key(&(year, week)) {
                continue;
            }
            // Une semaine absente de l'index précédent vient d'être publiée.
            // Au premier passage, rien n'est « nouveau » : l'index était inconnu.
            let published = known.is_some_and(|known| !known.contains(&week));

            // Après un redémarrage, la version sur disque sert de point de
            // comparaison : un changement survenu pendant l'arrêt est signalé
            // au passage suivant.
            if !published && self.load_from_disk(year, week).await {
                continue;
            }
            match self.timetable(year, week).await {
                Ok(entry) if published => {
                    info!(year, week, "nouvelle semaine publiée");
                    if let Some(notifier) = &self.notifier {
                        let others = self.same_dates(&entry.timetable).await;
                        let others: Vec<&Timetable> = others.iter().map(|other| &other.timetable).collect();
                        notifier.week_published(&entry.timetable, &others).await;
                    }
                }
                Ok(_) => {}
                Err(error) => warn!(%error, year, week, "semaine publiée illisible"),
            }
        }
    }

    /// Les semaines des autres années qui couvrent les mêmes dates : les salles
    /// sont partagées, leurs cours se rapprochent de ceux-ci.
    ///
    /// Le numéro de semaine n'est pas le même d'une année à l'autre ; ce sont
    /// les dates de l'en-tête qui font foi. Une semaine illisible est ignorée.
    async fn same_dates(&self, timetable: &Timetable) -> Vec<Arc<Entry>> {
        let mut found = Vec::new();
        for year in YEARS.into_iter().filter(|year| *year != timetable.year) {
            let Ok(weeks) = self.weeks(year).await else { continue };
            // Même numéro d'abord : c'est presque toujours la bonne.
            let first = weeks.iter().copied().filter(|week| *week == timetable.week);
            for week in first.chain(weeks.iter().copied().filter(|week| *week != timetable.week)) {
                let Ok(entry) = self.timetable(year, week).await else { continue };
                if entry.timetable.period.start == timetable.period.start {
                    found.push(entry);
                    break;
                }
            }
        }
        found
    }

    /// Reprend en mémoire un PDF déjà sur disque, sans validateurs : la
    /// prochaine revalidation le retéléchargera et le comparera.
    async fn load_from_disk(&self, year: u8, week: u8) -> bool {
        let Ok(bytes) = tokio::fs::read(self.disk_path(year, week)).await else { return false };
        self.store(year, week, upstream::Document { bytes, validators: Validators::default() }).is_ok()
    }

    /// Relit une semaine déjà en cache ; sans effet si elle n'y est pas.
    async fn refresh_timetable(&self, year: u8, week: u8) -> Revalidation {
        let Some(previous) = self.timetables.read().expect("cache empoisonné").get(&(year, week)).cloned()
        else {
            return Revalidation::Unchanged;
        };

        match self.upstream.fetch_week(year, week, &previous.validators).await {
            Ok(Fetch::Unchanged) => {
                debug!(year, week, "emploi du temps inchangé");
                Revalidation::Unchanged
            }
            Ok(Fetch::Fresh(document)) => {
                let bytes = document.bytes.len();
                self.write_to_disk(year, week, &document.bytes).await;
                match self.store(year, week, document) {
                    Ok(entry) => {
                        info!(
                            year,
                            week,
                            bytes,
                            courses_before = previous.timetable.courses.len(),
                            courses_after = entry.timetable.courses.len(),
                            "emploi du temps mis à jour"
                        );
                        if let Some(notifier) = &self.notifier {
                            let others = self.same_dates(&entry.timetable).await;
                            let others: Vec<&Timetable> =
                                others.iter().map(|other| &other.timetable).collect();
                            notifier.week_changed(&previous.timetable, &entry.timetable, &others).await;
                        }
                        Revalidation::Updated
                    }
                    Err(error) => {
                        warn!(%error, year, week, "nouvelle version illisible");
                        Revalidation::Failed
                    }
                }
            }
            Err(error) => {
                warn!(%error, year, week, "revalidation impossible");
                Revalidation::Failed
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
            Ok(bytes) => {
                info!(year, week, "emploi du temps servi depuis le cache disque");
                Ok(upstream::Document { bytes, validators: Validators::default() })
            }
            Err(_) => Err(upstream_error),
        }
    }
}
