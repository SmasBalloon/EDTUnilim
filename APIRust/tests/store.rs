//! Le cache : reprise en mémoire, écriture disque et repli quand l'amont
//! est indisponible.

use std::sync::Arc;
use std::time::Duration;

use edt_unilim_api::config::{Config, DiscordConfig};
use edt_unilim_api::discord::Discord;
use edt_unilim_api::error::ApiError;
use edt_unilim_api::notify::Notifier;
use edt_unilim_api::store::Store;
use edt_unilim_api::users::UserStore;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const A1_S3: &[u8] = include_bytes!("fixtures/A1_S3.pdf");
const A3_S2: &[u8] = include_bytes!("fixtures/A3_S2.pdf");

fn config(upstream: &MockServer, cache_dir: &std::path::Path) -> Config {
    Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        upstream: upstream.uri(),
        cache_dir: cache_dir.to_path_buf(),
        refresh_interval: Duration::from_secs(3600),
        upstream_timeout: Duration::from_secs(5),
        state_dir: std::env::temp_dir(),
        public_url: "http://localhost:8080".into(),
        discord: None,
    }
}

#[tokio::test]
async fn rejects_an_unknown_year_without_any_network_call() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = Store::new(&config(&server, cache_dir.path())).expect("store construit");

    let error = store.weeks(9).await.unwrap_err();
    assert!(matches!(error, ApiError::UnknownYear(9)));
    // Aucun stub n'a été posé : un appel réseau aurait fait échouer le test.
}

#[tokio::test]
async fn lists_weeks_from_the_upstream_index() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"<a href="A1_S12.pdf">A1_S12.pdf</a> <a href="A1_S3.pdf">A1_S3.pdf</a>"#),
        )
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = Store::new(&config(&server, cache_dir.path())).expect("store construit");

    let weeks = store.weeks(1).await.expect("index lisible");
    assert_eq!(*weeks, vec![3, 12]);
}

#[tokio::test]
async fn fetches_and_caches_a_timetable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3).insert_header("etag", "\"v1\""))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = Store::new(&config(&server, cache_dir.path())).expect("store construit");

    let first = store.timetable(1, 3).await.expect("PDF analysable");
    assert_eq!(first.timetable.week, 3);

    // Un deuxième appel doit venir du cache mémoire : le mock n'accepte
    // qu'une seule requête (`.expect(1)`), un second appel réseau ferait
    // échouer la vérification au moment où le serveur s'arrête.
    let second = store.timetable(1, 3).await.expect("servi depuis le cache");
    assert_eq!(second.fetched_at, first.fetched_at);

    let cached_pdf = cache_dir.path().join("A1").join("A1_S3.pdf");
    assert!(cached_pdf.exists(), "le PDF téléchargé doit être écrit sur disque");
}

#[tokio::test]
async fn reports_an_unknown_week_when_upstream_returns_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S45.pdf"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = Store::new(&config(&server, cache_dir.path())).expect("store construit");

    let error = store.timetable(1, 45).await.unwrap_err();
    assert!(matches!(error, ApiError::UnknownWeek { year: 1, week: 45 }));
}

#[tokio::test]
async fn falls_back_to_the_disk_cache_when_upstream_is_unavailable() {
    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");

    // Premier passage : l'amont répond, le PDF est écrit sur disque.
    {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/A1/A1_S3.pdf"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3))
            .mount(&server)
            .await;
        let store = Store::new(&config(&server, cache_dir.path())).expect("store construit");
        store.timetable(1, 3).await.expect("premier téléchargement réussi");
    }

    // Second passage, store neuf (cache mémoire vide) : l'amont est en panne,
    // le service doit se rabattre sur le PDF déjà sur disque.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let store = Store::new(&config(&server, cache_dir.path())).expect("store construit");

    let entry = store.timetable(1, 3).await.expect("repli sur le cache disque");
    assert_eq!(entry.timetable.week, 3);
}

#[tokio::test]
async fn keeps_the_cached_entry_when_refresh_finds_nothing_new() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .and(header("if-none-match", "\"v1\""))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3).insert_header("etag", "\"v1\""))
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = Store::new(&config(&server, cache_dir.path())).expect("store construit");

    let before = store.timetable(1, 3).await.expect("premier téléchargement réussi").fetched_at;

    store.refresh().await;

    let after = store.timetable(1, 3).await.expect("toujours en cache").fetched_at;
    assert_eq!(before, after, "un 304 ne doit pas remplacer l'entrée en cache");
}

#[tokio::test]
async fn revalidates_a_week_on_demand_but_not_twice_in_a_row() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .and(header("if-none-match", "\"v1\""))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3).insert_header("etag", "\"v2\""))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3).insert_header("etag", "\"v1\""))
        .expect(1)
        .mount(&server)
        .await;

    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = Store::new(&config(&server, cache_dir.path())).expect("store construit");

    let before = store.timetable(1, 3).await.expect("premier téléchargement réussi").fetched_at;

    store.revalidate_timetable(1, 3).await;
    let after = store.timetable(1, 3).await.expect("nouvelle version en cache").fetched_at;
    assert!(after > before, "une nouvelle version doit remplacer l'entrée en cache");

    // Revalidation immédiate : ignorée, le mock refuserait un troisième appel.
    store.revalidate_timetable(1, 3).await;
}

/// Store relié à une API Discord simulée, servie par le même serveur que
/// l'amont, avec une personne abonnée à BUT 1 G1A.
async fn watching(server: &MockServer, cache_dir: &std::path::Path, state_dir: &std::path::Path) -> Store {
    let discord = Discord::new(DiscordConfig {
        client_id: "1".into(),
        client_secret: "secret".into(),
        bot_token: "bot".into(),
        guild_id: "7".into(),
        known_guild_ids: vec!["5".into()],
        session_secret: "s".repeat(32),
        api_base: format!("{}/discord", server.uri()),
    })
    .expect("client Discord");

    let users = Arc::new(UserStore::open(state_dir).await.expect("comptes"));
    users.sign_in("42", "Ada", None).await.expect("compte créé");
    users
        .update("42", |user| {
            user.year = Some(1);
            user.group = Some("G1a".into());
        })
        .await
        .expect("classe enregistrée");

    Store::new(&config(server, cache_dir))
        .expect("store construit")
        .with_notifier(Notifier::new(discord, users))
}

async fn index(server: &MockServer, body: &'static str) {
    Mock::given(method("GET"))
        .and(path("/A1/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(server)
        .await;
    for year in ["/A2/", "/A3/"] {
        Mock::given(method("GET"))
            .and(path(year))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(server)
            .await;
    }
}

/// Le bot ouvre la conversation privée ; le message attendu y est posté `times` fois.
async fn expect_dm(server: &MockServer, contains: &'static str, times: u64) {
    Mock::given(method("POST"))
        .and(path("/discord/users/@me/channels"))
        .and(header("authorization", "Bot bot"))
        .and(body_string_contains("\"recipient_id\":\"42\""))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"900"}"#))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/discord/channels/900/messages"))
        .and(body_string_contains(contains))
        .and(body_string_contains("BEGIN:VCALENDAR"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(times)
        .mount(server)
        .await;
}

const WITH_S3: &str = r#"<a href="A1_S3.pdf">A1_S3.pdf</a>"#;

#[tokio::test]
async fn sends_a_dm_when_the_class_timetable_changes() {
    let server = MockServer::start().await;
    index(&server, WITH_S3).await;
    // Première version, puis une version où les cours de G1A ont disparu.
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3).insert_header("etag", "\"v1\""))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A3_S2).insert_header("etag", "\"v2\""))
        .with_priority(2)
        .mount(&server)
        .await;
    expect_dm(&server, "modifiée pour G1A", 1).await;

    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let state_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = watching(&server, cache_dir.path(), state_dir.path()).await;

    // Premier passage : la semaine est suivie sans avoir été consultée.
    store.refresh().await;
    // Second passage : le PDF a changé.
    store.refresh().await;
}

#[tokio::test]
async fn stays_silent_when_a_republished_pdf_has_the_same_courses() {
    let server = MockServer::start().await;
    index(&server, WITH_S3).await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3))
        .mount(&server)
        .await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&server).await;

    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let state_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = watching(&server, cache_dir.path(), state_dir.path()).await;
    store.refresh().await;
    store.refresh().await;
}

#[tokio::test]
async fn sends_a_newly_published_week_with_its_calendar() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    index(&server, WITH_S3).await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3))
        .mount(&server)
        .await;
    expect_dm(&server, "publiée", 1).await;

    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    let state_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = watching(&server, cache_dir.path(), state_dir.path()).await;
    store.refresh().await; // index vide
    store.refresh().await; // S3 apparaît
}

#[tokio::test]
async fn detects_a_change_made_while_the_service_was_stopped() {
    let server = MockServer::start().await;
    index(&server, WITH_S3).await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A3_S2))
        .mount(&server)
        .await;
    expect_dm(&server, "modifiée", 1).await;

    // L'ancienne version est restée sur disque d'une exécution précédente.
    let cache_dir = tempfile::tempdir().expect("répertoire temporaire");
    std::fs::create_dir_all(cache_dir.path().join("A1")).unwrap();
    std::fs::write(cache_dir.path().join("A1").join("A1_S3.pdf"), A1_S3).unwrap();

    let state_dir = tempfile::tempdir().expect("répertoire temporaire");
    let store = watching(&server, cache_dir.path(), state_dir.path()).await;
    store.refresh().await;
}
