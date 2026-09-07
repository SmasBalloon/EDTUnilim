//! Configuration du service, lue dans l'environnement.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Adresse d'écoute par défaut.
const DEFAULT_BIND: &str = "0.0.0.0:3000";
/// Racine des emplois du temps publiés par le département.
const DEFAULT_UPSTREAM: &str = "https://edt-iut-info.unilim.fr/edt";
/// Répertoire du cache de PDF.
const DEFAULT_CACHE_DIR: &str = "cache";
/// Périodicité du rafraîchissement en arrière-plan.
const DEFAULT_REFRESH: Duration = Duration::from_secs(15 * 60);
/// Délai maximal accordé au serveur amont.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
/// Années du BUT publiées (`A1`, `A2`, `A3`).
pub const YEARS: [u8; 3] = [1, 2, 3];

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub upstream: String,
    pub cache_dir: PathBuf,
    pub refresh_interval: Duration,
    pub upstream_timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0} : valeur invalide ({1})")]
    Invalid(&'static str, String),
}

impl Config {
    /// Lit la configuration ; toute variable absente prend sa valeur par défaut.
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind = var("EDT_BIND").unwrap_or_else(|| DEFAULT_BIND.to_owned());
        let bind = bind.parse().map_err(|_| ConfigError::Invalid("EDT_BIND", bind.clone()))?;

        Ok(Self {
            bind,
            upstream: var("EDT_UPSTREAM")
                .unwrap_or_else(|| DEFAULT_UPSTREAM.to_owned())
                .trim_end_matches('/')
                .to_owned(),
            cache_dir: var("EDT_CACHE_DIR").map_or_else(|| PathBuf::from(DEFAULT_CACHE_DIR), PathBuf::from),
            refresh_interval: duration("EDT_REFRESH_SECONDS", DEFAULT_REFRESH)?,
            upstream_timeout: duration("EDT_UPSTREAM_TIMEOUT_SECONDS", DEFAULT_TIMEOUT)?,
        })
    }
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn duration(name: &'static str, default: Duration) -> Result<Duration, ConfigError> {
    match var(name) {
        None => Ok(default),
        Some(value) => value.parse().map(Duration::from_secs).map_err(|_| ConfigError::Invalid(name, value)),
    }
}
