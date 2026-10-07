//! Connexion Discord et compte, bout en bout, avec une API Discord simulée.

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use axum_test::TestServer;
use edt_unilim_api::config::{Config, DiscordConfig};
use edt_unilim_api::discord::Discord;
use edt_unilim_api::homework::HomeworkStore;
use edt_unilim_api::http::{self, Account};
use edt_unilim_api::notify::Notifier;
use edt_unilim_api::session::Signer;
use edt_unilim_api::store::Store;
use edt_unilim_api::users::UserStore;
use serde_json::{json, Value};
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const A1_S3: &[u8] = include_bytes!("fixtures/A1_S3.pdf");

struct Setup {
    server: TestServer,
    _dirs: (tempfile::TempDir, tempfile::TempDir),
}

async fn setup(upstream: &MockServer) -> Setup {
    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let state_dir = tempfile::tempdir().expect("répertoire temporaire");
    let discord_config = DiscordConfig {
        client_id: "1".into(),
        client_secret: "secret".into(),
        bot_token: "bot".into(),
        guild_id: "7".into(),
        known_guild_ids: vec!["5".into()],
        conflict_recipients: Vec::new(),
        session_secret: "s".repeat(32),
        public_key: None,
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
    let store = Arc::new(Store::new(&config).expect("store").with_notifier(notifier.clone()));
    let account = Arc::new(Account {
        discord,
        users,
        homework,
        notifier,
        signer: Signer::new(&discord_config.session_secret),
        public_url: config.public_url.clone(),
        secure: false,
    });

    let mut server = TestServer::new(http::router(&config, store, Some(account)));
    server.save_cookies();
    Setup { server, _dirs: (cache_dir, state_dir) }
}

/// Discord accepte le code et présente Ada. Si elle est déjà membre du serveur
/// connu (`5`), elle n'est pas ajoutée au serveur d'accueil (`7`).
async fn discord_accepts_login(upstream: &MockServer, in_known_guild: bool) {
    Mock::given(method("POST"))
        .and(path("/discord/oauth2/token"))
        .and(body_string_contains("code=abc"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"access_token":"tok"}"#))
        .mount(upstream)
        .await;
    Mock::given(method("GET"))
        .and(path("/discord/users/@me"))
        .and(header("authorization", "Bearer tok"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"id":"42","username":"ada","global_name":"Ada","avatar":null}"#),
        )
        .mount(upstream)
        .await;
    let membership = if in_known_guild {
        ResponseTemplate::new(200).set_body_string(r#"{"user":{"id":"42"}}"#)
    } else {
        ResponseTemplate::new(404).set_body_string(r#"{"message":"Unknown Member","code":10007}"#)
    };
    Mock::given(method("GET"))
        .and(path("/discord/guilds/5/members/42"))
        .and(header("authorization", "Bot bot"))
        .respond_with(membership)
        .expect(1)
        .mount(upstream)
        .await;
    Mock::given(method("PUT"))
        .and(path("/discord/guilds/7/members/42"))
        .and(header("authorization", "Bot bot"))
        .and(body_string_contains("tok"))
        .respond_with(ResponseTemplate::new(201).set_body_string("{}"))
        .expect(if in_known_guild { 0 } else { 1 })
        .mount(upstream)
        .await;
}

/// Démarre la connexion et renvoie le nonce transmis à Discord.
async fn start_login(server: &TestServer) -> String {
    let response = server.get("/api/v1/auth/login").await;
    response.assert_status(StatusCode::SEE_OTHER);
    let location = response.header("location").to_str().unwrap().to_owned();
    assert!(location.starts_with("https://discord.com/oauth2/authorize?"), "{location}");
    assert!(location.contains("scope=identify%20guilds.join"), "{location}");
    assert!(location.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fapi%2Fv1%2Fauth%2Fcallback"));
    location.split("state=").nth(1).unwrap().split('&').next().unwrap().to_owned()
}

#[tokio::test]
async fn signs_in_with_discord_and_subscribes_to_a_class() {
    let upstream = MockServer::start().await;
    discord_accepts_login(&upstream, false).await;
    // La semaine en cours accompagne le message de bienvenue.
    Mock::given(method("GET"))
        .and(path("/A1/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"<a href="A1_S3.pdf">A1_S3.pdf</a>"#))
        .mount(&upstream)
        .await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3))
        .mount(&upstream)
        .await;
    Mock::given(method("POST"))
        .and(path("/discord/users/@me/channels"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"900"}"#))
        .mount(&upstream)
        .await;
    Mock::given(method("POST"))
        .and(path("/discord/channels/900/messages"))
        .and(body_string_contains("BUT 1 G1A"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&upstream)
        .await;

    let Setup { server, _dirs } = setup(&upstream).await;
    server.get("/api/v1/me").await.assert_status(StatusCode::UNAUTHORIZED);

    let state = start_login(&server).await;
    let back = server.get(&format!("/api/v1/auth/callback?code=abc&state={state}")).await;
    back.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(back.header("location"), "/#compte");

    let me: Value = server.get("/api/v1/me").await.json();
    assert_eq!(me["name"], "Ada");
    assert_eq!(me["group"], Value::Null);

    let updated = server.put("/api/v1/me").json(&json!({ "year": 1, "group": "g1A", "notify": true })).await;
    updated.assert_status_ok();
    let updated: Value = updated.json();
    assert_eq!(updated["user"]["group"], "G1a");
    assert_eq!(updated["dm"], "sent");

    // Même classe : pas de second message de bienvenue (le mock n'en attend qu'un).
    let again: Value =
        server.put("/api/v1/me").json(&json!({ "year": 1, "group": "G1a", "notify": true })).await.json();
    assert_eq!(again["dm"], "none");

    server
        .put("/api/v1/me")
        .json(&json!({ "year": 1, "group": "G1", "notify": true }))
        .await
        .assert_status(StatusCode::BAD_REQUEST);

    server.post("/api/v1/auth/logout").await.assert_status(StatusCode::NO_CONTENT);
    server.get("/api/v1/me").await.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn does_not_add_a_member_of_a_known_server_to_the_home_server() {
    let upstream = MockServer::start().await;
    discord_accepts_login(&upstream, true).await;

    let Setup { server, _dirs } = setup(&upstream).await;
    let state = start_login(&server).await;
    let back = server.get(&format!("/api/v1/auth/callback?code=abc&state={state}")).await;
    assert_eq!(back.header("location"), "/#compte");
    server.get("/api/v1/me").await.assert_status_ok();
}

#[tokio::test]
async fn refuses_a_callback_whose_state_does_not_match() {
    let upstream = MockServer::start().await;
    // Aucun appel à Discord ne doit partir.
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(500)).expect(0).mount(&upstream).await;

    let Setup { server, _dirs } = setup(&upstream).await;
    start_login(&server).await;
    let back = server.get("/api/v1/auth/callback?code=abc&state=forged").await;
    back.assert_status(StatusCode::SEE_OTHER);
    assert_eq!(back.header("location"), "/#compte?erreur=session");
    server.get("/api/v1/me").await.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn deletes_the_account() {
    let upstream = MockServer::start().await;
    discord_accepts_login(&upstream, false).await;

    let Setup { server, _dirs } = setup(&upstream).await;
    let state = start_login(&server).await;
    server.get(&format!("/api/v1/auth/callback?code=abc&state={state}")).await;
    server.get("/api/v1/me").await.assert_status_ok();

    server.delete("/api/v1/me").await.assert_status(StatusCode::NO_CONTENT);
    server.get("/api/v1/me").await.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn reports_the_account_routes_as_disabled_without_discord() {
    let config = Config::from_env().expect("configuration par défaut valide");
    let store = Arc::new(Store::new(&config).expect("store"));
    let server = TestServer::new(http::router(&config, store, None));

    let response = server.get("/api/v1/me").await;
    response.assert_status(StatusCode::NOT_FOUND);
    response.assert_json_contains(&json!({ "error": "auth_disabled" }));
}
