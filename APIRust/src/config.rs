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
    /// Webhook Discord où signaler les changements d'emploi du temps.
    pub discord_webhook: Option<String>,
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
            discord_webhook: var("EDT_DISCORD_WEBHOOK"),
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

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    /// Les variables lues par `Config` sont globales au process ; les tests
    /// qui les manipulent doivent donc s'exécuter en série.
    fn clear_env() {
        for name in [
            "EDT_BIND",
            "EDT_UPSTREAM",
            "EDT_CACHE_DIR",
            "EDT_REFRESH_SECONDS",
            "EDT_UPSTREAM_TIMEOUT_SECONDS",
            "EDT_DISCORD_WEBHOOK",
        ] {
            std::env::remove_var(name);
        }
    }

    #[test]
    #[serial]
    fn falls_back_to_defaults() {
        clear_env();
        let config = Config::from_env().expect("défauts valides");

        assert_eq!(config.bind.to_string(), DEFAULT_BIND);
        assert_eq!(config.upstream, DEFAULT_UPSTREAM);
        assert_eq!(config.cache_dir, PathBuf::from(DEFAULT_CACHE_DIR));
        assert_eq!(config.refresh_interval, DEFAULT_REFRESH);
        assert_eq!(config.upstream_timeout, DEFAULT_TIMEOUT);
    }

    #[test]
    #[serial]
    fn reads_overrides_from_the_environment() {
        clear_env();
        std::env::set_var("EDT_BIND", "127.0.0.1:8080");
        std::env::set_var("EDT_UPSTREAM", "https://example.test/edt/");
        std::env::set_var("EDT_CACHE_DIR", "/tmp/edt-cache");
        std::env::set_var("EDT_REFRESH_SECONDS", "60");
        std::env::set_var("EDT_UPSTREAM_TIMEOUT_SECONDS", "5");

        let config = Config::from_env().expect("configuration valide");

        assert_eq!(config.bind.to_string(), "127.0.0.1:8080");
        // La barre oblique finale est retirée pour ne pas la doubler dans les URL construites.
        assert_eq!(config.upstream, "https://example.test/edt");
        assert_eq!(config.cache_dir, PathBuf::from("/tmp/edt-cache"));
        assert_eq!(config.refresh_interval, Duration::from_secs(60));
        assert_eq!(config.upstream_timeout, Duration::from_secs(5));

        clear_env();
    }

    #[test]
    #[serial]
    fn treats_blank_variables_as_absent() {
        clear_env();
        std::env::set_var("EDT_UPSTREAM", "   ");

        let config = Config::from_env().expect("défauts valides");

        assert_eq!(config.upstream, DEFAULT_UPSTREAM);
        clear_env();
    }

    #[test]
    #[serial]
    fn rejects_an_invalid_bind_address() {
        clear_env();
        std::env::set_var("EDT_BIND", "not-an-address");

        let error = Config::from_env().unwrap_err();
        assert!(matches!(error, ConfigError::Invalid("EDT_BIND", _)));

        clear_env();
    }

    #[test]
    #[serial]
    fn rejects_a_non_numeric_duration() {
        clear_env();
        std::env::set_var("EDT_REFRESH_SECONDS", "bientôt");

        let error = Config::from_env().unwrap_err();
        assert!(matches!(error, ConfigError::Invalid("EDT_REFRESH_SECONDS", _)));

        clear_env();
    }
}
