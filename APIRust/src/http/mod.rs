//! Couche HTTP : routage et état partagé.

mod account;
mod analysis;
mod homework;
mod interactions;
mod meta;
mod timetable;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::rejection::PathRejection;
use axum::extract::{DefaultBodyLimit, FromRequestParts, Path, Request};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::Router;
use serde::de::DeserializeOwned;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tracing::Instrument;

use crate::config::Config;
use crate::error::ApiError;
use crate::store::Store;

/// Délai au-delà duquel une requête est abandonnée.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Extracteur de chemin dont les rejets restent au format JSON de l'API.
pub struct PathParams<T>(pub T);

impl<S, T> FromRequestParts<S> for PathParams<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Path::<T>::from_request_parts(parts, state).await {
            Ok(Path(value)) => Ok(Self(value)),
            Err(rejection) => Err(match rejection {
                PathRejection::FailedToDeserializePathParams(error) => {
                    ApiError::BadRequest(error.body_text())
                }
                other => ApiError::BadRequest(other.body_text()),
            }),
        }
    }
}

pub use account::Account;
pub use interactions::commands;

pub struct AppState {
    pub store: Arc<Store>,
    pub upstream: String,
    /// Connexion Discord ; absente, les routes du compte répondent `auth_disabled`.
    pub account: Option<Arc<Account>>,
}

impl AppState {
    /// Les fonctions qui demandent un compte, ou la raison de leur absence.
    pub fn account(&self) -> Result<&Arc<Account>, ApiError> {
        self.account.as_ref().ok_or(ApiError::AuthDisabled)
    }
}

pub fn router(config: &Config, store: Arc<Store>, account: Option<Arc<Account>>) -> Router {
    let state = Arc::new(AppState { store, upstream: config.upstream.clone(), account });

    Router::new()
        .route("/", get(meta::index))
        .route("/health", get(meta::health))
        .route("/api/v1/resources", get(meta::resources))
        .route("/api/v1/teachers", get(meta::teachers))
        .route("/api/v1/years", get(timetable::years))
        .route("/api/v1/years/{year}/weeks", get(timetable::weeks))
        .route("/api/v1/years/{year}/weeks/{week}", get(timetable::week))
        .route("/api/v1/years/{year}/calendar/{group}", get(timetable::calendar))
        .route("/api/v1/analysis", post(analysis::analyse).layer(DefaultBodyLimit::max(analysis::MAX_BODY)))
        .route("/api/v1/auth/login", get(account::login))
        .route("/api/v1/auth/callback", get(account::callback))
        .route("/api/v1/auth/logout", post(account::logout))
        .route("/api/v1/me", get(account::me).put(account::update).delete(account::delete))
        .route("/api/v1/homework", get(homework::list).post(homework::create))
        .route("/api/v1/homework/{id}", put(homework::edit).delete(homework::delete))
        .route("/api/v1/homework/{id}/confirm", post(homework::confirm))
        .route("/api/v1/homework/{id}/done", put(homework::done))
        .route("/api/v1/discord/interactions", post(interactions::handle))
        .fallback(|| async { ApiError::UnknownRoute })
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TimeoutLayer::with_status_code(StatusCode::GATEWAY_TIMEOUT, REQUEST_TIMEOUT))
        // En dernier, donc à l'extérieur : les 504 du délai sont journalisés aussi.
        .layer(middleware::from_fn(log_request))
        .with_state(state)
}

/// Journalise chaque requête : méthode, chemin, statut, durée et client.
///
/// Seul le chemin est écrit, jamais la requête : le retour OAuth y porte un
/// code d'autorisation. La sonde `/health` de Docker, toutes les 30 s, ne
/// sort qu'en `debug`.
async fn log_request(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    // nginx relaie l'adresse réelle ; en direct, il n'y a rien à montrer.
    let client = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map_or_else(|| "-".to_owned(), |value| value.trim().to_owned());

    // Les journaux écrits pendant le traitement portent la méthode et le chemin.
    let span = tracing::info_span!("requête", %method, %path);
    let response = next.run(request).instrument(span).await;

    let status = response.status().as_u16();
    let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if path == "/health" {
        tracing::debug!(%method, %path, status, ms, "sonde de santé");
    } else if response.status().is_server_error() {
        tracing::error!(%method, %path, status, ms, %client, "requête servie");
    } else {
        tracing::info!(%method, %path, status, ms, %client, "requête servie");
    }
    response
}
