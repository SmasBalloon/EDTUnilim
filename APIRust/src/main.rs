//! Point d'entrée du service.

use std::io::IsTerminal;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::TcpListener;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use edt_unilim_api::config::Config;
use edt_unilim_api::discord::Discord;
use edt_unilim_api::homework::HomeworkStore;
use edt_unilim_api::http::{self, Account};
use edt_unilim_api::notify::Notifier;
use edt_unilim_api::presence;
use edt_unilim_api::session::Signer;
use edt_unilim_api::store::Store;
use edt_unilim_api::users::UserStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("EDT_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        // Pas de codes couleur dans `docker logs` ni dans un fichier.
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    let config = Config::from_env()?;
    info!(
        version = env!("CARGO_PKG_VERSION"),
        upstream = %config.upstream,
        refresh_s = config.refresh_interval.as_secs(),
        upstream_timeout_s = config.upstream_timeout.as_secs(),
        cache_dir = %config.cache_dir.display(),
        state_dir = %config.state_dir.display(),
        discord = config.discord.is_some(),
        "configuration chargée"
    );
    let mut store = Store::new(&config)?;
    let mut account = None;
    if let Some(discord_config) = &config.discord {
        let users = Arc::new(UserStore::open(&config.state_dir).await?);
        let homework = Arc::new(HomeworkStore::open(&config.state_dir).await?);
        info!(
            subscribers = users.subscribers_all().await.len(),
            homework = homework.all().await.len(),
            "comptes et devoirs chargés"
        );
        let discord = Discord::new(discord_config.clone())?;
        let notifier = Notifier::new(discord.clone(), Arc::clone(&users))
            .with_conflict_recipients(discord_config.conflict_recipients.clone());
        store = store.with_notifier(notifier.clone());
        tokio::spawn(reminder_loop(Arc::clone(&homework), Arc::clone(&users), notifier.clone()));
        // La seule connexion permanente du service : sans elle, le bot est là
        // mais paraît hors ligne.
        tokio::spawn(presence::keep_online(discord_config.clone(), Arc::clone(&homework)));
        account = Some(Arc::new(Account {
            discord,
            users,
            homework,
            notifier,
            signer: Signer::new(&discord_config.session_secret),
            public_url: config.public_url.clone(),
            secure: config.is_https(),
            admins: discord_config.admins.clone(),
        }));
        info!(public_url = %config.public_url, "connexion Discord et messages privés actifs");

        // Les commandes se redéclarent à chaque démarrage : c'est peu coûteux,
        // et le bot ne reste jamais avec une commande d'une version passée.
        //
        // Sans clé publique, on ne les déclare pas : une commande qui apparaît
        // dans Discord mais dont la réponse ne peut être ni reçue ni vérifiée
        // est pire que pas de commande du tout.
        let url = format!("{}/api/v1/discord/interactions", config.public_url);
        match (&account, discord_config.public_key.is_some()) {
            (Some(account), true) => {
                let discord = account.discord.clone();
                let endpoint = url.clone();
                tokio::spawn(async move {
                    match discord.register_commands(&http::commands()).await {
                        Ok(()) => info!(
                            endpoint = %endpoint,
                            "commandes Discord déclarées — à déclarer aussi comme « Interactions Endpoint URL » dans le portail ; Discord met quelques minutes à les propager"
                        ),
                        Err(error) => error!(%error, "commandes Discord refusées"),
                    }
                });
            }
            _ => warn!(
                "commandes slash éteintes : EDT_DISCORD_PUBLIC_KEY est absente (les messages privés, eux, fonctionnent)"
            ),
        }
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

/// Heure du rappel du soir, en temps universel — environ 19 h en France.
const REMINDER_HOUR: u8 = 17;
/// Au-delà, un devoir rendu depuis longtemps ne sert plus à rien.
const KEEP_DAYS: i64 = 30;

/// Rappelle chaque soir ce qui est à rendre le lendemain.
///
/// La boucle se réveille tous les quarts d'heure et ne fait rien d'autre que
/// regarder l'heure : pas d'ordonnanceur, et un redémarrage en pleine journée
/// ne fait ni doublon ni oubli, puisque le jour du dernier envoi est retenu.
async fn reminder_loop(homework: Arc<HomeworkStore>, users: Arc<UserStore>, notifier: Notifier) {
    let mut ticker = tokio::time::interval(Duration::from_secs(15 * 60));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last: Option<time::Date> = None;

    loop {
        ticker.tick().await;
        let now = time::OffsetDateTime::now_utc();
        if now.hour() < REMINDER_HOUR || last == Some(now.date()) {
            continue;
        }
        last = Some(now.date());

        let tomorrow = now.date().next_day().unwrap_or(now.date());
        let items = homework.all().await;
        let mut reminded = 0;
        for user in users.subscribers_all().await {
            let due: Vec<&edt_unilim_api::homework::Homework> = items
                .iter()
                .filter(|item| {
                    item.due == Some(tomorrow)
                        && !item.is_pending()
                        && !item.is_done_by(&user.id)
                        && item.visible_to(&user)
                })
                .collect();
            if !due.is_empty() {
                reminded += 1;
            }
            notifier.homework_due_tomorrow(&user, &due).await;
        }
        info!(reminded, %tomorrow, "rappels de devoirs envoyés");

        // Le ménage du soir : les devoirs rendus depuis un mois s'effacent.
        let limit = now.date() - time::Duration::days(KEEP_DAYS);
        match homework.forget_before(limit).await {
            Ok(0) => {}
            Ok(count) => info!(count, "devoirs échus oubliés"),
            Err(error) => error!(%error, "ménage des devoirs impossible"),
        }
    }
}

/// Revalide périodiquement les emplois du temps déjà consultés : le
/// département republie un PDF dès qu'un créneau bouge.
async fn refresh_loop(store: Arc<Store>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut first = true;
    loop {
        ticker.tick().await;
        let started = Instant::now();
        let summary = store.refresh().await;
        info!(
            checked = summary.checked,
            updated = summary.updated,
            failed = summary.failed,
            ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "revalidation périodique terminée"
        );
        // Les semaines publiées sont en cache après ce premier passage : c'est
        // le moment de faire l'état des salles doubles déjà présentes.
        if std::mem::take(&mut first) {
            store.report_conflicts(time::OffsetDateTime::now_utc().date()).await;
        }
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
        () = interrupt => info!("Ctrl+C reçu, arrêt en cours"),
        () = terminate => info!("SIGTERM reçu, arrêt en cours"),
    }
}
