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
/// Répertoire des données à conserver (comptes).
const DEFAULT_STATE_DIR: &str = "state";
/// Adresse publique du site, qui sert à construire l'URL de retour OAuth.
const DEFAULT_PUBLIC_URL: &str = "http://localhost:8080";
/// API Discord.
const DEFAULT_DISCORD_API: &str = "https://discord.com/api/v10";
/// Longueur minimale du secret de session, en octets.
const MIN_SECRET_LEN: usize = 32;
/// Années du BUT publiées (`A1`, `A2`, `A3`).
pub const YEARS: [u8; 3] = [1, 2, 3];

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub upstream: String,
    pub cache_dir: PathBuf,
    pub refresh_interval: Duration,
    pub upstream_timeout: Duration,
    pub state_dir: PathBuf,
    /// Adresse publique du site, sans barre oblique finale.
    pub public_url: String,
    /// Connexion Discord et messages privés ; absente, ces fonctions sont coupées.
    pub discord: Option<DiscordConfig>,
}

#[derive(Clone)]
pub struct DiscordConfig {
    pub client_id: String,
    pub client_secret: String,
    pub bot_token: String,
    /// Serveur où les personnes connectées sont ajoutées, et où se trouve le bot.
    pub guild_id: String,
    /// Serveurs où le bot se trouve déjà : leurs membres peuvent recevoir des
    /// messages privés sans être ajoutés à `guild_id`.
    pub known_guild_ids: Vec<String>,
    /// Comptes prévenus en message privé des salles réservées par plusieurs
    /// cours en même temps (enseignants, administration) ; vide, rien ne part.
    pub conflict_recipients: Vec<String>,
    /// Comptes autorisés à voir les PDF en cache et à en déposer de nouveaux,
    /// quand le serveur du département est injoignable.
    pub admins: Vec<String>,
    /// Clé de signature des cookies de session.
    pub session_secret: String,
    /// Clé publique de l'application Discord : sans elle, les commandes slash
    /// restent éteintes, car on ne saurait pas vérifier ce qui nous arrive.
    pub public_key: Option<String>,
    pub api_base: String,
}

impl std::fmt::Debug for DiscordConfig {
    // Secrets : seuls les identifiants publics apparaissent.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscordConfig")
            .field("client_id", &self.client_id)
            .field("guild_id", &self.guild_id)
            .field("known_guild_ids", &self.known_guild_ids)
            .field("conflict_recipients", &self.conflict_recipients)
            .field("admins", &self.admins)
            .field("api_base", &self.api_base)
            .field("slash_commands", &self.public_key.is_some())
            .finish_non_exhaustive()
    }
}

/// Variables nécessaires à la connexion Discord : toutes ou aucune.
const DISCORD_VARS: [&str; 5] = [
    "EDT_DISCORD_CLIENT_ID",
    "EDT_DISCORD_CLIENT_SECRET",
    "EDT_DISCORD_BOT_TOKEN",
    "EDT_DISCORD_GUILD_ID",
    "EDT_SESSION_SECRET",
];

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0} : valeur invalide ({1})")]
    Invalid(&'static str, String),
    #[error("connexion Discord incomplète, variables manquantes : {0}")]
    IncompleteDiscord(String),
    #[error("EDT_SESSION_SECRET : au moins {MIN_SECRET_LEN} caractères")]
    WeakSessionSecret,
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
            state_dir: var("EDT_STATE_DIR").map_or_else(|| PathBuf::from(DEFAULT_STATE_DIR), PathBuf::from),
            public_url: var("EDT_PUBLIC_URL")
                .unwrap_or_else(|| DEFAULT_PUBLIC_URL.to_owned())
                .trim_end_matches('/')
                .to_owned(),
            discord: discord()?,
        })
    }
}

fn discord() -> Result<Option<DiscordConfig>, ConfigError> {
    let values: Vec<Option<String>> = DISCORD_VARS.iter().map(|name| var(name)).collect();
    // Le secret de session peut être préparé d'avance : ce sont les variables
    // Discord qui activent la connexion.
    if values[..4].iter().all(Option::is_none) {
        return Ok(None);
    }
    let missing: Vec<&str> = DISCORD_VARS
        .iter()
        .zip(&values)
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| *name)
        .collect();
    if !missing.is_empty() {
        return Err(ConfigError::IncompleteDiscord(missing.join(", ")));
    }
    let [client_id, client_secret, bot_token, guild_id, session_secret] =
        <[Option<String>; 5]>::try_from(values)
            .expect("autant de valeurs que de variables")
            .map(Option::unwrap);
    if session_secret.len() < MIN_SECRET_LEN {
        return Err(ConfigError::WeakSessionSecret);
    }
    Ok(Some(DiscordConfig {
        client_id,
        client_secret,
        bot_token,
        guild_id,
        known_guild_ids: ids("EDT_DISCORD_KNOWN_GUILD_IDS"),
        conflict_recipients: ids("EDT_DISCORD_CONFLICT_RECIPIENTS"),
        admins: ids("EDT_DISCORD_ADMINS"),
        session_secret,
        public_key: var("EDT_DISCORD_PUBLIC_KEY").map(|key| key.trim().to_owned()),
        api_base: var("EDT_DISCORD_API")
            .unwrap_or_else(|| DEFAULT_DISCORD_API.to_owned())
            .trim_end_matches('/')
            .to_owned(),
    }))
}

impl Config {
    /// Vrai quand le site est servi en HTTPS : les cookies sont alors `Secure`.
    pub fn is_https(&self) -> bool {
        self.public_url.starts_with("https://")
    }
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.trim().is_empty())
}

/// Identifiants Discord séparés par des virgules.
fn ids(name: &str) -> Vec<String> {
    var(name)
        .map(|ids| ids.split(',').map(str::trim).filter(|id| !id.is_empty()).map(str::to_owned).collect())
        .unwrap_or_default()
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
            "EDT_STATE_DIR",
            "EDT_PUBLIC_URL",
            "EDT_DISCORD_API",
            "EDT_DISCORD_KNOWN_GUILD_IDS",
            "EDT_DISCORD_CONFLICT_RECIPIENTS",
            "EDT_DISCORD_ADMINS",
        ]
        .into_iter()
        .chain(DISCORD_VARS)
        {
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
    fn leaves_discord_off_when_unconfigured() {
        clear_env();
        let config = Config::from_env().expect("défauts valides");
        assert!(config.discord.is_none());
        assert_eq!(config.public_url, DEFAULT_PUBLIC_URL);
        assert!(!config.is_https());
    }

    #[test]
    #[serial]
    fn reads_a_complete_discord_configuration() {
        clear_env();
        for name in DISCORD_VARS {
            std::env::set_var(name, "x".repeat(MIN_SECRET_LEN));
        }
        std::env::set_var("EDT_PUBLIC_URL", "https://edt.example/");
        std::env::set_var("EDT_DISCORD_KNOWN_GUILD_IDS", " 111, 222 ,");
        std::env::set_var("EDT_DISCORD_CONFLICT_RECIPIENTS", "333");
        std::env::set_var("EDT_DISCORD_ADMINS", "444");

        let config = Config::from_env().expect("configuration valide");
        let discord = config.discord.as_ref().expect("Discord configuré");
        assert_eq!(discord.api_base, DEFAULT_DISCORD_API);
        assert_eq!(discord.known_guild_ids, ["111", "222"]);
        assert_eq!(discord.conflict_recipients, ["333"]);
        assert_eq!(discord.admins, ["444"]);
        assert_eq!(config.public_url, "https://edt.example");
        assert!(config.is_https());
        assert!(!format!("{discord:?}").contains("client_secret"), "les secrets restent hors des journaux");

        clear_env();
    }

    #[test]
    #[serial]
    fn rejects_a_partial_discord_configuration() {
        clear_env();
        std::env::set_var("EDT_SESSION_SECRET", "x".repeat(MIN_SECRET_LEN));
        assert!(Config::from_env().expect("secret seul accepté").discord.is_none());

        std::env::set_var("EDT_DISCORD_CLIENT_ID", "123");

        let error = Config::from_env().unwrap_err();
        assert!(
            matches!(&error, ConfigError::IncompleteDiscord(missing) if missing.contains("EDT_DISCORD_BOT_TOKEN"))
        );

        std::env::set_var("EDT_DISCORD_CLIENT_SECRET", "s");
        std::env::set_var("EDT_DISCORD_BOT_TOKEN", "t");
        std::env::set_var("EDT_DISCORD_GUILD_ID", "1");
        std::env::set_var("EDT_SESSION_SECRET", "court");
        assert!(matches!(Config::from_env().unwrap_err(), ConfigError::WeakSessionSecret));

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
