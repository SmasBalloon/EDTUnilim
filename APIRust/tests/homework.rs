//! Les devoirs, bout en bout : les routes du site et les commandes du bot.

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use axum_test::TestServer;
use ed25519_dalek::{Signer as _, SigningKey};
use edt_unilim_api::config::{Config, DiscordConfig};
use edt_unilim_api::discord::Discord;
use edt_unilim_api::homework::HomeworkStore;
use edt_unilim_api::http::{self, Account};
use edt_unilim_api::notify::Notifier;
use edt_unilim_api::session::{Signer, SESSION_COOKIE, SESSION_TTL};
use edt_unilim_api::store::Store;
use edt_unilim_api::users::UserStore;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Graine de la clé de signature : les tests n'ont pas besoin d'aléa.
const KEY_SEED: [u8; 32] = [7; 32];

struct Setup {
    /// Un serveur par personne : chacun porte sa session.
    ada: TestServer,
    bob: TestServer,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn setup(upstream: &MockServer) -> Setup {
    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let state_dir = tempfile::tempdir().expect("répertoire temporaire");
    let key = SigningKey::from_bytes(&KEY_SEED);

    let discord_config = DiscordConfig {
        client_id: "1".into(),
        client_secret: "secret".into(),
        bot_token: "bot".into(),
        guild_id: "7".into(),
        known_guild_ids: Vec::new(),
        session_secret: "s".repeat(32),
        public_key: Some(hex(key.verifying_key().as_bytes())),
        api_base: format!("{}/discord", upstream.uri()),
    };
    let config = Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        upstream: upstream.uri(),
        cache_dir: cache_dir.path().to_path_buf(),
        refresh_interval: Duration::from_secs(3600),
        upstream_timeout: Duration::from_secs(5),
        state_dir: state_dir.path().to_path_buf(),
        public_url: "http://localhost:8080".into(),
        discord: Some(discord_config.clone()),
    };

    let users = Arc::new(UserStore::open(state_dir.path()).await.expect("comptes"));
    let homework = Arc::new(HomeworkStore::open(state_dir.path()).await.expect("devoirs"));
    let discord = Discord::new(discord_config.clone()).expect("client Discord");
    let notifier = Notifier::new(discord.clone(), Arc::clone(&users));
    let store = Arc::new(Store::new(&config).expect("store"));

    // Deux camarades du même groupe : l'une note, l'autre confirme.
    for (id, name) in [("42", "Ada"), ("43", "Bob")] {
        users.sign_in(id, name, None).await.expect("compte créé");
        users
            .update(id, |user| {
                user.year = Some(1);
                user.group = Some("G1a".into());
            })
            .await
            .expect("classe enregistrée")
            .expect("compte existant");
    }

    let account = Arc::new(Account {
        discord,
        users,
        homework,
        notifier,
        signer: Signer::new(&discord_config.session_secret),
        public_url: config.public_url.clone(),
        secure: false,
    });

    let session = |id: &str| Signer::new(&discord_config.session_secret).sign(id, SESSION_TTL);
    let mut ada = TestServer::new(http::router(&config, Arc::clone(&store), Some(Arc::clone(&account))));
    ada.add_header("cookie", format!("{SESSION_COOKIE}={}", session("42")));
    let mut bob = TestServer::new(http::router(&config, store, Some(account)));
    bob.add_header("cookie", format!("{SESSION_COOKIE}={}", session("43")));

    Setup { ada, bob, _dirs: (cache_dir, state_dir) }
}

/// Les messages privés partent dans le vide : ce test regarde les devoirs.
async fn discord_accepts_dms(upstream: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/discord/users/@me/channels"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"900"}"#))
        .mount(upstream)
        .await;
    Mock::given(method("POST"))
        .and(path("/discord/channels/900/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(upstream)
        .await;
}

fn class_homework() -> Value {
    json!({
        "title": "Exercices 3 à 7",
        "resource": "r1.05",
        "due": "2026-09-21",
        "audience": { "kind": "class", "group": "G1" },
    })
}

#[tokio::test]
async fn a_class_homework_needs_someone_else_to_confirm_it() {
    let upstream = MockServer::start().await;
    discord_accepts_dms(&upstream).await;
    let setup = setup(&upstream).await;

    // Ada note un devoir pour le groupe : il attend une confirmation.
    let created = setup.ada.post("/api/v1/homework").json(&class_homework()).await;
    created.assert_status_ok();
    let item: Value = created.json();
    assert_eq!(item["title"], "Exercices 3 à 7");
    assert_eq!(item["resource"], "R1.05", "le code est normalisé");
    assert_eq!(item["pending"], true);
    assert_eq!(item["mine"], true);
    assert_eq!(item["can_confirm"], false, "on ne confirme pas son propre devoir");
    let id = item["id"].as_str().expect("identifiant").to_owned();

    // Bob le voit, marqué en attente, et peut le confirmer.
    let list: Value = setup.bob.get("/api/v1/homework").await.json();
    assert_eq!(list.as_array().expect("liste").len(), 1);
    assert_eq!(list[0]["pending"], true);
    assert_eq!(list[0]["can_confirm"], true);
    assert_eq!(list[0]["mine"], false);

    let confirmed: Value = setup.bob.post(&format!("/api/v1/homework/{id}/confirm")).await.json();
    assert_eq!(confirmed["pending"], false);
    assert_eq!(confirmed["confirmed_name"], "Bob");

    // Deux fois, non.
    setup.bob.post(&format!("/api/v1/homework/{id}/confirm")).await.assert_status(StatusCode::FORBIDDEN);

    // « Fait » ne vaut que pour soi.
    let done: Value =
        setup.bob.put(&format!("/api/v1/homework/{id}/done")).json(&json!({"done": true})).await.json();
    assert_eq!(done["done_by_me"], true);
    let seen_by_ada: Value = setup.ada.get("/api/v1/homework").await.json();
    assert_eq!(seen_by_ada[0]["done_by_me"], false, "le devoir de Bob reste à faire pour Ada");

    // Seule Ada peut l'effacer.
    setup.bob.delete(&format!("/api/v1/homework/{id}")).await.assert_status(StatusCode::FORBIDDEN);
    setup.ada.delete(&format!("/api/v1/homework/{id}")).await.assert_status(StatusCode::NO_CONTENT);
    let empty: Value = setup.ada.get("/api/v1/homework").await.json();
    assert!(empty.as_array().expect("liste").is_empty());
}

#[tokio::test]
async fn a_personal_homework_stays_out_of_sight() {
    let upstream = MockServer::start().await;
    discord_accepts_dms(&upstream).await;
    let setup = setup(&upstream).await;

    let body = json!({ "title": "Relire le cours", "audience": { "kind": "personal" } });
    setup.ada.post("/api/v1/homework").json(&body).await.assert_status_ok();

    let mine: Value = setup.ada.get("/api/v1/homework").await.json();
    assert_eq!(mine.as_array().expect("liste").len(), 1);
    let theirs: Value = setup.bob.get("/api/v1/homework").await.json();
    assert!(theirs.as_array().expect("liste").is_empty());
}

#[tokio::test]
async fn one_cannot_note_a_homework_for_another_class() {
    let upstream = MockServer::start().await;
    discord_accepts_dms(&upstream).await;
    let setup = setup(&upstream).await;

    let body = json!({ "title": "Exercices", "audience": { "kind": "class", "group": "G2" } });
    let refused = setup.ada.post("/api/v1/homework").json(&body).await;
    refused.assert_status(StatusCode::BAD_REQUEST);
}

/// Discord met du temps à répondre : les messages privés ne doivent pas pour
/// autant retarder la réponse à l'interaction.
async fn discord_dms_are_slow(upstream: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/discord/users/@me/channels"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"id":"900"}"#).set_delay(Duration::from_secs(5)),
        )
        .mount(upstream)
        .await;
}

/// Signe une requête comme Discord le fait : horodatage puis corps.
fn signed(server: &TestServer, body: &Value) -> axum_test::TestRequest {
    let key = SigningKey::from_bytes(&KEY_SEED);
    let timestamp = "1758196800";
    let payload = body.to_string();
    let signature = key.sign(format!("{timestamp}{payload}").as_bytes());
    server
        .post("/api/v1/discord/interactions")
        .add_header("x-signature-ed25519", hex(&signature.to_bytes()))
        .add_header("x-signature-timestamp", timestamp)
        .add_header("content-type", "application/json")
        .text(payload)
}

#[tokio::test]
async fn discord_interactions_must_be_signed() {
    let upstream = MockServer::start().await;
    let setup = setup(&upstream).await;

    // Discord vérifie la route en la sondant, et en la sondant mal.
    let ping = json!({ "type": 1 });
    let pong: Value = signed(&setup.ada, &ping).await.json();
    assert_eq!(pong["type"], 1);

    let refused = setup
        .ada
        .post("/api/v1/discord/interactions")
        .add_header("x-signature-ed25519", "00".repeat(64))
        .add_header("x-signature-timestamp", "1758196800")
        .json(&ping)
        .await;
    refused.assert_status(StatusCode::UNAUTHORIZED);

    // Sans signature du tout non plus.
    setup.ada.post("/api/v1/discord/interactions").json(&ping).await.assert_status(StatusCode::UNAUTHORIZED);
}

/// Discord abandonne au bout de trois secondes : « l'application n'a pas
/// répondu à temps ». Prévenir une classe demande deux requêtes par personne,
/// donc l'annonce doit partir de son côté, sans retenir la réponse.
#[tokio::test]
async fn an_interaction_answers_without_waiting_for_discord() {
    let upstream = MockServer::start().await;
    discord_dms_are_slow(&upstream).await;
    let setup = setup(&upstream).await;

    let add = json!({
        "type": 2,
        "user": { "id": "42" },
        "data": {
            "name": "devoir",
            "options": [{
                "name": "ajouter",
                "type": 1,
                "options": [
                    { "name": "intitule", "value": "Exercices 3 à 7" },
                    { "name": "portee", "value": "group" },
                ],
            }],
        },
    });

    let answered = tokio::time::timeout(Duration::from_secs(1), signed(&setup.ada, &add)).await;
    let answer: Value = answered.expect("réponse rendue sans attendre Discord").json();
    assert!(answer["data"]["content"].as_str().expect("réponse").contains("Noté"));

    // Et le devoir est bien enregistré, lui, avant que la réponse parte.
    let list: Value = setup.bob.get("/api/v1/homework").await.json();
    assert_eq!(list.as_array().expect("liste").len(), 1);
}

#[tokio::test]
async fn the_bot_notes_and_lists_homework() {
    let upstream = MockServer::start().await;
    discord_accepts_dms(&upstream).await;
    let setup = setup(&upstream).await;

    // `/devoir ajouter intitule:… matiere:R1.05 pour:21/09 portee:groupe`
    let add = json!({
        "type": 2,
        "user": { "id": "42" },
        "data": {
            "name": "devoir",
            "options": [{
                "name": "ajouter",
                "type": 1,
                "options": [
                    { "name": "intitule", "value": "Exercices 3 à 7" },
                    { "name": "matiere", "value": "r1.05" },
                    { "name": "pour", "value": "21/09/2026" },
                    { "name": "portee", "value": "group" },
                ],
            }],
        },
    });
    let answer: Value = signed(&setup.ada, &add).await.json();
    let text = answer["data"]["content"].as_str().expect("réponse");
    assert!(text.contains("R1.05"), "{text}");
    assert!(text.contains("confirm"), "la classe est invitée à confirmer : {text}");
    assert_eq!(answer["data"]["flags"], 64, "la réponse ne s'affiche que pour l'auteur");

    // Bob confirme depuis le bouton de sa notification.
    let list: Value = setup.bob.get("/api/v1/homework").await.json();
    let id = list[0]["id"].as_str().expect("identifiant");
    let press = json!({
        "type": 3,
        "user": { "id": "43" },
        "data": { "custom_id": format!("hw:confirm:{id}") },
    });
    let answer: Value = signed(&setup.bob, &press).await.json();
    assert!(answer["data"]["content"].as_str().expect("réponse").contains("Confirmé"));

    // `/devoirs` liste ce qui reste à faire, avec un bouton pour le cocher.
    let list = json!({ "type": 2, "user": { "id": "43" }, "data": { "name": "devoirs" } });
    let answer: Value = signed(&setup.bob, &list).await.json();
    let text = answer["data"]["content"].as_str().expect("réponse");
    assert!(text.contains("Exercices 3 à 7"), "{text}");
    let button = &answer["data"]["components"][0]["components"][0];
    assert_eq!(button["custom_id"], format!("hw:done:{id}"));

    // Le bouton coche le devoir pour Bob seul.
    let press = json!({
        "type": 3,
        "user": { "id": "43" },
        "data": { "custom_id": format!("hw:done:{id}") },
    });
    signed(&setup.bob, &press).await.assert_status_ok();
    let bob_sees: Value = setup.bob.get("/api/v1/homework").await.json();
    assert_eq!(bob_sees[0]["done_by_me"], true);
    let ada_sees: Value = setup.ada.get("/api/v1/homework").await.json();
    assert_eq!(ada_sees[0]["done_by_me"], false);
}
