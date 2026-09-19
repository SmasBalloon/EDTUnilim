//! La présence du bot : « en ligne », et ce qu'il est en train de regarder.
//!
//! Tout le reste du service parle à Discord en HTTP : on appelle, on raccroche.
//! Une présence, elle, ne s'envoie pas par une requête — Discord ne la lit que
//! sur la *passerelle*, une connexion permanente. C'est la seule raison d'être
//! de ce module : sans lui le bot fonctionne, il apparaît simplement hors ligne
//! dans la liste des membres.
//!
//! On ne demande donc aucune intention (`intents: 0`) : le bot ne reçoit ni
//! messages ni événements de serveur. Il se contente de dire qu'il est là.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use time::OffsetDateTime;
use tokio::time::{interval, sleep, MissedTickBehavior};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

use crate::config::DiscordConfig;
use crate::homework::HomeworkStore;

/// La passerelle de Discord, en version 10 et en JSON.
const GATEWAY: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
/// Périodicité de rafraîchissement du texte affiché.
const REFRESH: Duration = Duration::from_secs(10 * 60);
/// Attente avant de se reconnecter, doublée à chaque échec.
const BACKOFF: Duration = Duration::from_secs(5);
const BACKOFF_MAX: Duration = Duration::from_secs(5 * 60);

/// Tient le bot en ligne, et se reconnecte tant que le service tourne.
///
/// Une passerelle se ferme pour mille raisons — Discord redémarre un nœud, le
/// réseau hoquette. Ce n'est pas une erreur, c'est l'usage : on repart, en
/// patientant de plus en plus longtemps si ça persiste.
pub async fn keep_online(config: DiscordConfig, homework: Arc<HomeworkStore>) {
    let mut wait = BACKOFF;
    loop {
        match connect(&config, &homework).await {
            Ok(()) => {
                debug!("passerelle Discord fermée, reconnexion");
                wait = BACKOFF;
            }
            // Un refus d'authentification ne s'arrange pas en réessayant : on
            // le dit une fois, clairement, et on s'arrête là.
            Err(Error::Rejected { code, reason }) => {
                error!(code, %reason, "{}", explain(code));
                return;
            }
            Err(error) => {
                warn!(%error, ?wait, "passerelle Discord injoignable");
                wait = (wait * 2).min(BACKOFF_MAX);
            }
        }
        sleep(wait).await;
    }
}

/// Ce que veut dire un code de fermeture, dit en clair.
fn explain(code: u16) -> &'static str {
    match code {
        4004 => "présence Discord impossible : le jeton du bot est refusé (EDT_DISCORD_BOT_TOKEN)",
        4013 | 4014 => "présence Discord impossible : intentions refusées par l'application",
        4010..=4012 => "présence Discord impossible : identification rejetée",
        _ => "présence Discord impossible",
    }
}

/// Une session : on se présente, on bat la mesure, on tient le texte à jour.
async fn connect(config: &DiscordConfig, homework: &Arc<HomeworkStore>) -> Result<(), Error> {
    let (mut socket, _) = tokio_tungstenite::connect_async(GATEWAY).await?;

    // Discord parle le premier : il dit à quelle cadence battre le cœur.
    let hello = next_payload(&mut socket).await?.ok_or(Error::Closed)?;
    let beat = hello["d"]["heartbeat_interval"].as_u64().ok_or(Error::Unexpected)?;
    let mut heartbeat = interval(Duration::from_millis(beat));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut refresh = interval(REFRESH);
    refresh.set_missed_tick_behavior(MissedTickBehavior::Delay);
    refresh.tick().await; // la première présence part avec l'identification

    socket.send(Message::text(identify(config, activity(homework).await).to_string())).await?;

    // Dernier numéro d'événement reçu, à renvoyer à chaque battement.
    let mut last: Option<u64> = None;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                socket.send(Message::text(json!({ "op": 1, "d": last }).to_string())).await?;
            }
            _ = refresh.tick() => {
                let update = json!({ "op": 3, "d": presence(activity(homework).await) });
                socket.send(Message::text(update.to_string())).await?;
            }
            payload = next_payload(&mut socket) => {
                let Some(payload) = payload? else { return Ok(()) };
                if let Some(sequence) = payload["s"].as_u64() {
                    last = Some(sequence);
                }
                match payload["op"].as_u64() {
                    // Discord réclame un battement immédiat.
                    Some(1) => socket.send(Message::text(json!({ "op": 1, "d": last }).to_string())).await?,
                    // Il nous demande de repartir de zéro : on referme, la
                    // boucle appelante se chargera de revenir.
                    Some(7 | 9) => return Ok(()),
                    Some(11) => {}
                    Some(0) if payload["t"] == "READY" => {
                        let name = payload["d"]["user"]["username"].as_str().unwrap_or("le bot");
                        info!(bot = name, "présence Discord établie");
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Le message d'identification : qui nous sommes, et ce qu'on affiche.
fn identify(config: &DiscordConfig, activity: String) -> Value {
    json!({
        "op": 2,
        "d": {
            "token": config.bot_token,
            // Aucune intention : le bot n'écoute rien, il se montre.
            "intents": 0,
            "properties": { "os": "linux", "browser": "edt-unilim", "device": "edt-unilim" },
            "presence": presence(activity),
        },
    })
}

fn presence(activity: String) -> Value {
    json!({
        // 3 : « Regarde … ».
        "activities": [{ "name": activity, "type": 3 }],
        "status": "online",
        "since": Value::Null,
        "afk": false,
    })
}

/// Ce que le bot regarde : les emplois du temps, et les devoirs en cours.
async fn activity(homework: &Arc<HomeworkStore>) -> String {
    let today = OffsetDateTime::now_utc().date();
    let pending = homework
        .all()
        .await
        .iter()
        .filter(|item| !item.is_pending() && item.due.is_none_or(|due| due >= today))
        .count();

    match pending {
        0 => "les emplois du temps".to_owned(),
        1 => "les emplois du temps · 1 devoir".to_owned(),
        count => format!("les emplois du temps · {count} devoirs"),
    }
}

/// Le prochain message JSON, en passant les trames de service.
async fn next_payload<S>(socket: &mut S) -> Result<Option<Value>, Error>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(message) = socket.next().await {
        match message? {
            Message::Text(text) => return Ok(Some(serde_json::from_str(&text)?)),
            // Discord dit pourquoi il raccroche : certains motifs sont
            // définitifs, il faut les distinguer d'une coupure ordinaire.
            Message::Close(frame) => {
                return match frame {
                    Some(frame) if (4004..=4014).contains(&u16::from(frame.code)) => {
                        Err(Error::Rejected { code: frame.code.into(), reason: frame.reason.to_string() })
                    }
                    _ => Ok(None),
                }
            }
            _ => continue,
        }
    }
    Ok(None)
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("passerelle : {0}")]
    Socket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("message illisible : {0}")]
    Unreadable(#[from] serde_json::Error),
    #[error("la passerelle s'est fermée avant de se présenter")]
    Closed,
    #[error("la passerelle a répondu autre chose que prévu")]
    Unexpected,
    #[error("la passerelle a raccroché ({code}) : {reason}")]
    Rejected { code: u16, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> DiscordConfig {
        DiscordConfig {
            client_id: "1".into(),
            client_secret: "secret".into(),
            bot_token: "jeton".into(),
            guild_id: "7".into(),
            known_guild_ids: Vec::new(),
            session_secret: "s".repeat(32),
            public_key: None,
            api_base: "https://discord.test".into(),
        }
    }

    #[test]
    fn identifies_without_asking_for_anything() {
        let payload = identify(&config(), "les emplois du temps".into());
        assert_eq!(payload["op"], 2);
        assert_eq!(payload["d"]["token"], "jeton");
        assert_eq!(payload["d"]["intents"], 0, "le bot n'écoute aucun événement");
        assert_eq!(payload["d"]["presence"]["status"], "online");
        assert_eq!(payload["d"]["presence"]["activities"][0]["type"], 3);
        assert_eq!(payload["d"]["presence"]["activities"][0]["name"], "les emplois du temps");
    }

    #[test]
    fn presence_watches_and_stays_online() {
        let payload = presence("les cours".into());
        assert_eq!(payload["status"], "online");
        assert_eq!(payload["afk"], false);
        assert!(payload["since"].is_null());
        assert_eq!(payload["activities"][0]["name"], "les cours");
    }

    #[test]
    fn explains_each_closing_code_in_plain_words() {
        assert!(explain(4004).contains("EDT_DISCORD_BOT_TOKEN"));
        assert!(explain(4013).contains("intentions"));
        assert!(explain(4014).contains("intentions"));
        assert!(explain(4011).contains("identification"));
        assert_eq!(explain(1000), "présence Discord impossible");
    }

    /// Une passerelle simulée : les messages qu'elle enverrait, dans l'ordre.
    fn gateway(
        messages: Vec<Result<Message, tokio_tungstenite::tungstenite::Error>>,
    ) -> impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin {
        futures_util::stream::iter(messages)
    }

    fn close(code: u16, reason: &str) -> Message {
        use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
        use tokio_tungstenite::tungstenite::protocol::CloseFrame;
        Message::Close(Some(CloseFrame { code: CloseCode::from(code), reason: reason.into() }))
    }

    #[tokio::test]
    async fn reads_the_next_json_message_and_skips_service_frames() {
        let mut socket = gateway(vec![
            Ok(Message::Ping(Vec::new().into())),
            Ok(Message::text(r#"{"op":10,"d":{"heartbeat_interval":41250}}"#)),
        ]);
        let payload = next_payload(&mut socket).await.expect("lu").expect("un message");
        assert_eq!(payload["op"], 10);
        assert_eq!(payload["d"]["heartbeat_interval"], 41250);
    }

    #[tokio::test]
    async fn a_silent_end_is_not_an_error() {
        assert!(next_payload(&mut gateway(Vec::new())).await.expect("fin propre").is_none());
        // Une fermeture ordinaire non plus : on se reconnecte.
        assert!(next_payload(&mut gateway(vec![Ok(close(1000, "bye"))])).await.expect("fermé").is_none());
        assert!(next_payload(&mut gateway(vec![Ok(Message::Close(None))])).await.expect("fermé").is_none());
    }

    #[tokio::test]
    async fn a_refusal_is_final() {
        let error = next_payload(&mut gateway(vec![Ok(close(4004, "jeton"))])).await.expect_err("refus");
        assert!(matches!(error, Error::Rejected { code: 4004, .. }));
        assert!(error.to_string().contains("4004"));
    }

    #[tokio::test]
    async fn unreadable_text_is_reported() {
        let error = next_payload(&mut gateway(vec![Ok(Message::text("pas du json"))])).await.expect_err("illisible");
        assert!(matches!(error, Error::Unreadable(_)));
    }

    #[tokio::test]
    async fn counts_the_homework_still_standing() {
        use crate::homework::{Audience, NewHomework};
        use crate::users::User;
        use time::Duration as Days;

        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let store = Arc::new(HomeworkStore::open(dir.path()).await.expect("devoirs"));
        let today = OffsetDateTime::now_utc().date();
        let ada = User {
            id: "1".into(),
            name: "Ada".into(),
            avatar: None,
            year: Some(1),
            group: Some("G1a".into()),
            notify: true,
            created_at: OffsetDateTime::now_utc(),
        };
        let note = |due| NewHomework {
            year: 1,
            audience: Audience::Personal,
            title: "Exercices".into(),
            details: None,
            resource: None,
            due,
            course: None,
        };

        assert_eq!(activity(&store).await, "les emplois du temps");

        store.create(&ada, note(Some(today + Days::days(2)))).await.expect("à venir");
        assert_eq!(activity(&store).await, "les emplois du temps · 1 devoir");

        // Ce qui est déjà rendu ne compte plus.
        store.create(&ada, note(Some(today - Days::days(2)))).await.expect("passé");
        assert_eq!(activity(&store).await, "les emplois du temps · 1 devoir");

        store.create(&ada, note(None)).await.expect("sans échéance");
        assert_eq!(activity(&store).await, "les emplois du temps · 2 devoirs");
    }
}
