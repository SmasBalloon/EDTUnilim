//! Couche HTTP : routage et état partagé.

mod account;
mod meta;
mod timetable;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::rejection::PathRejection;
use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use serde::de::DeserializeOwned;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

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

pub struct AppState {
    pub store: Arc<Store>,
    pub upstream: String,
    /// Connexion Discord ; absente, les routes du compte répondent `auth_disabled`.
    pub account: Option<Arc<Account>>,
}

pub fn router(config: &Config, store: Arc<Store>, account: Option<Arc<Account>>) -> Router {
    let state = Arc::new(AppState { store, upstream: config.upstream.clone(), account });

    Router::new()
        .route("/", get(meta::index))
        .route("/health", get(meta::health))
        .route("/api/v1/resources", get(meta::resources))
        .route("/api/v1/years", get(timetable::years))
        .route("/api/v1/years/{year}/weeks", get(timetable::weeks))
        .route("/api/v1/years/{year}/weeks/{week}", get(timetable::week))
        .route("/api/v1/auth/login", get(account::login))
        .route("/api/v1/auth/callback", get(account::callback))
        .route("/api/v1/auth/logout", post(account::logout))
        .route("/api/v1/me", get(account::me).put(account::update).delete(account::delete))
        .fallback(|| async { ApiError::UnknownRoute })
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TimeoutLayer::with_status_code(StatusCode::GATEWAY_TIMEOUT, REQUEST_TIMEOUT))
        .with_state(state)
}
