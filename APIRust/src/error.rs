//! Erreurs de l'API et leur rendu en JSON.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("aucun emploi du temps pour l'année {year}, semaine {week}")]
    UnknownWeek { year: u8, week: u8 },
    #[error("année inconnue : {0}")]
    UnknownYear(u8),
    #[error("le serveur de l'IUT est injoignable")]
    UpstreamUnavailable(#[source] reqwest::Error),
    #[error("le serveur de l'IUT a renvoyé une réponse incohérente")]
    UpstreamUnexpected,
    #[error("l'emploi du temps publié est illisible")]
    Unreadable(#[source] crate::pdf::PdfError),
    #[error("{0}")]
    BadRequest(String),
    #[error("cette route n'existe pas")]
    UnknownRoute,
    #[error("connexion requise")]
    Unauthenticated,
    #[error("{0}")]
    Forbidden(&'static str),
    #[error("la connexion Discord n'est pas configurée sur ce serveur")]
    AuthDisabled,
    #[error("enregistrement impossible")]
    Storage(#[from] std::io::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            Self::UnknownWeek { .. } | Self::UnknownYear(_) | Self::UnknownRoute | Self::AuthDisabled => {
                StatusCode::NOT_FOUND
            }
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::UpstreamUnavailable(_) | Self::UpstreamUnexpected => StatusCode::BAD_GATEWAY,
            Self::Unreadable(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::UnknownWeek { .. } => "unknown_week",
            Self::BadRequest(_) => "bad_request",
            Self::UnknownRoute => "unknown_route",
            Self::Forbidden(_) => "forbidden",
            Self::UnknownYear(_) => "unknown_year",
            Self::UpstreamUnavailable(_) => "upstream_unavailable",
            Self::UpstreamUnexpected => "upstream_unexpected",
            Self::Unreadable(_) => "unreadable_timetable",
            Self::Unauthenticated => "unauthenticated",
            Self::AuthDisabled => "auth_disabled",
            Self::Storage(_) => "storage",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // La cause technique n'a d'intérêt que dans les journaux.
        if let Some(source) = std::error::Error::source(&self) {
            tracing::warn!(error = %self, cause = %source, "requête en échec");
        }
        let body = ErrorBody { error: self.code(), message: self.to_string() };
        (self.status(), Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;

    use super::*;

    async fn body_json(error: ApiError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("corps lisible");
        (status, serde_json::from_slice(&bytes).expect("corps JSON"))
    }

    #[tokio::test]
    async fn renders_an_unknown_year_as_404() {
        let (status, body) = body_json(ApiError::UnknownYear(9)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "unknown_year");
        assert!(body["message"].as_str().unwrap().contains('9'));
    }

    #[tokio::test]
    async fn renders_an_unknown_week_as_404() {
        let (status, body) = body_json(ApiError::UnknownWeek { year: 1, week: 45 }).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "unknown_week");
    }

    #[tokio::test]
    async fn renders_an_unknown_route_as_404() {
        let (status, body) = body_json(ApiError::UnknownRoute).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "unknown_route");
    }

    #[tokio::test]
    async fn renders_a_bad_request_as_400() {
        let (status, body) = body_json(ApiError::BadRequest("semaine invalide".to_owned())).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "bad_request");
        assert_eq!(body["message"], "semaine invalide");
    }

    #[tokio::test]
    async fn renders_an_upstream_unexpected_response_as_502() {
        let (status, body) = body_json(ApiError::UpstreamUnexpected).await;

        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body["error"], "upstream_unexpected");
    }
}
