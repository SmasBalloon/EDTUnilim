//! Point d'entrée du service.

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use edt_unilim_api::config::Config;
use edt_unilim_api::discord::Discord;
use edt_unilim_api::http::{self, Account};
use edt_unilim_api::notify::Notifier;
use edt_unilim_api::session::Signer;
use edt_unilim_api::store::Store;
use edt_unilim_api::users::UserStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("EDT_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = Config::from_env()?;
    let mut store = Store::new(&config)?;
    let mut account = None;
    if let Some(discord_config) = &config.discord {
        let users = Arc::new(UserStore::open(&config.state_dir).await?);
        let discord = Discord::new(discord_config.clone())?;
        let notifier = Notifier::new(discord.clone(), Arc::clone(&users));
        store = store.with_notifier(notifier.clone());
        account = Some(Arc::new(Account {
            discord,
            users,
            notifier,
            signer: Signer::new(&discord_config.session_secret),
            public_url: config.public_url.clone(),
            secure: config.is_https(),
        }));
        info!(public_url = %config.public_url, "connexion Discord et messages privés actifs");
    }
    let store = Arc::new(store);

    tokio::spawn(refresh_loop(Arc::clone(&store), config.refresh_interval));

    let listener = TcpListener::bind(config.bind).await?;
    info!(address = %listener.local_addr()?, upstream = %config.upstream, "service démarré");

    let router = http::router(&config, store, account);
    axum::serve(listener, router).with_graceful_shutdown(shutdown_signal()).await?;

    info!("arrêt du service");
    Ok(())
}

/// Revalide périodiquement les emplois du temps déjà consultés : le
/// département republie un PDF dès qu'un créneau bouge.
async fn refresh_loop(store: Arc<Store>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        store.refresh().await;
    }
}

async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = signal::ctrl_c().await {
            error!(%error, "écoute de Ctrl+C impossible");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => error!(%error, "écoute de SIGTERM impossible"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {},
        () = terminate => {},
    }
}
