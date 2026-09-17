//! Routes servies sans dépendre du serveur du département, et bout en bout
//! avec un serveur amont simulé.

use std::sync::Arc;
use std::time::Duration;

use axum_test::TestServer;
use edt_unilim_api::config::Config;
use edt_unilim_api::http;
use edt_unilim_api::store::Store;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const A1_S3: &[u8] = include_bytes!("fixtures/A1_S3.pdf");

fn server() -> TestServer {
    let config = Config::from_env().expect("configuration par défaut valide");
    let store = Arc::new(Store::new(&config).expect("client HTTP construit"));
    TestServer::new(http::router(&config, store, None))
}

fn server_with(upstream: &MockServer) -> TestServer {
    let config = Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        upstream: upstream.uri(),
        cache_dir: std::env::temp_dir().join(format!("edt-http-test-{}", uuid())),
        refresh_interval: Duration::from_secs(3600),
        upstream_timeout: Duration::from_secs(5),
        state_dir: std::env::temp_dir(),
        public_url: "http://localhost:8080".into(),
        discord: None,
    };
    let store = Arc::new(Store::new(&config).expect("client HTTP construit"));
    TestServer::new(http::router(&config, store, None))
}

fn uuid() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).expect("horloge cohérente").as_nanos() as u64
}

#[tokio::test]
async fn reports_health() {
    let response = server().get("/health").await;

    response.assert_status_ok();
    response.assert_json_contains(&serde_json::json!({ "status": "ok" }));
}

#[tokio::test]
async fn describes_its_routes() {
    let response = server().get("/").await;

    response.assert_status_ok();
    assert!(response.text().contains("/api/v1/years/{year}/weeks/{week}"));
}

#[tokio::test]
async fn serves_the_resource_catalogue() {
    let response = server().get("/api/v1/resources").await;

    response.assert_status_ok();
    assert!(response.text().contains("R1.01"));
}

#[tokio::test]
async fn rejects_an_unknown_year_in_json() {
    let response = server().get("/api/v1/years/9/weeks/1").await;

    response.assert_status_not_found();
    response.assert_json_contains(&serde_json::json!({ "error": "unknown_year" }));
}

#[tokio::test]
async fn rejects_a_malformed_path_in_json() {
    let response = server().get("/api/v1/years/premiere/weeks/1").await;

    response.assert_status_bad_request();
    response.assert_json_contains(&serde_json::json!({ "error": "bad_request" }));
}

#[tokio::test]
async fn rejects_an_unknown_route_in_json() {
    let response = server().get("/api/v2/whatever").await;

    response.assert_status_not_found();
    response.assert_json_contains(&serde_json::json!({ "error": "unknown_route" }));
}

#[tokio::test]
async fn serves_a_timetable_filtered_by_group_and_day() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S3.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(A1_S3))
        .mount(&upstream)
        .await;

    let response = server_with(&upstream).get("/api/v1/years/1/weeks/3?group=G1a&day=lundi").await;

    response.assert_status_ok();
    response.assert_header("cache-control", "public, max-age=300");

    let body: serde_json::Value = response.json();
    assert_eq!(body["year"], 1);
    assert_eq!(body["week"], 3);
    assert!(body["source"]["url"].as_str().unwrap().ends_with("/A1/A1_S3.pdf"));

    let courses = body["courses"].as_array().expect("liste de créneaux");
    assert_eq!(body["count"], courses.len());
    assert!(!courses.is_empty());
    for course in courses {
        assert_eq!(course["day"], "LUNDI");
        assert!(course["groups"].as_array().unwrap().iter().any(|g| g == "G1" || g == "G1a"));
    }
}

#[tokio::test]
async fn reports_an_unknown_week_as_404_through_the_router() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/A1_S45.pdf"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&upstream)
        .await;

    let response = server_with(&upstream).get("/api/v1/years/1/weeks/45").await;

    response.assert_status_not_found();
    response.assert_json_contains(&serde_json::json!({ "error": "unknown_week" }));
}

#[tokio::test]
async fn lists_available_weeks_for_a_year() {
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/A1/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"<a href="A1_S3.pdf">A1_S3.pdf</a>"#))
        .mount(&upstream)
        .await;

    let response = server_with(&upstream).get("/api/v1/years/1/weeks").await;

    response.assert_status_ok();
    response.assert_json_contains(&serde_json::json!({ "year": 1, "weeks": [3] }));
}
