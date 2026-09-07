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
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            Self::UnknownWeek { .. } | Self::UnknownYear(_) | Self::UnknownRoute => StatusCode::NOT_FOUND,
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
            Self::UnknownYear(_) => "unknown_year",
            Self::UpstreamUnavailable(_) => "upstream_unavailable",
            Self::UpstreamUnexpected => "upstream_unexpected",
            Self::Unreadable(_) => "unreadable_timetable",
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
