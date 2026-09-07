//! Routes servies sans dépendre du serveur du département.

use std::sync::Arc;

use axum_test::TestServer;
use edt_unilim_api::config::Config;
use edt_unilim_api::http;
use edt_unilim_api::store::Store;

fn server() -> TestServer {
    let config = Config::from_env().expect("configuration par défaut valide");
    let store = Arc::new(Store::new(&config).expect("client HTTP construit"));
    TestServer::new(http::router(&config, store)).expect("serveur de test")
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
