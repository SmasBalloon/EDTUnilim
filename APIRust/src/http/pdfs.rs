//! Les PDF en cache, pour les administrateurs (`EDT_DISCORD_ADMINS`) : les
//! consulter, et en déposer quand le serveur du département est injoignable.

use std::sync::Arc;

use axum::extract::multipart::{Multipart, MultipartError, MultipartRejection};
use axum::extract::State;
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::config::YEARS;
use crate::error::ApiError;
use crate::model::Period;
use crate::store::CachedPdf;
use crate::upstream::{parse_week_file_name, week_file_name};

use super::{AppState, PathParams};

/// Un dépôt : un PDF du département pèse une quinzaine de Ko, et nginx refuse
/// de toute façon au-delà d'un Mo.
pub const MAX_BODY: usize = 1024 * 1024;

#[derive(Serialize)]
pub struct YearPdfs {
    year: u8,
    pdfs: Vec<CachedPdf>,
}

/// `GET /api/v1/admin/pdfs` : les PDF en cache, par année.
pub async fn list(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    state.account()?.admin_from(&headers).await?;
    let mut years = Vec::with_capacity(YEARS.len());
    for year in YEARS {
        years.push(YearPdfs { year, pdfs: state.store.cached_pdfs(year).await });
    }
    Ok(([(CACHE_CONTROL, "no-store")], Json(years)).into_response())
}

/// `GET /api/v1/admin/pdfs/{year}/{week}` : le PDF tel qu'il est en cache.
pub async fn download(
    State(state): State<Arc<AppState>>,
    PathParams((year, week)): PathParams<(u8, u8)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    state.account()?.admin_from(&headers).await?;
    let bytes = state.store.cached_pdf(year, week).await.ok_or(ApiError::UnknownWeek { year, week })?;
    let disposition = format!("inline; filename=\"{}\"", week_file_name(year, week));
    Ok((
        [
            (CONTENT_TYPE, "application/pdf".to_owned()),
            (CONTENT_DISPOSITION, disposition),
            (CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Debug, Serialize)]
pub struct Imported {
    file: String,
    year: u8,
    week: u8,
    period: Period,
    courses: usize,
}

/// `POST /api/v1/admin/pdfs` : un champ par PDF, nommé d'après sa semaine
/// (`A1_S12`). Chaque PDF remplace celui de sa semaine.
pub async fn upload(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> Result<Json<Vec<Imported>>, ApiError> {
    let user = state.account()?.admin_from(&headers).await?;
    let mut multipart = multipart.map_err(|rejection| ApiError::BadRequest(rejection.body_text()))?;

    let mut imported = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(rejected)? {
        let name = field.name().unwrap_or_default().to_owned();
        let (year, week) = parse_week_file_name(&format!("{name}.pdf"))
            .filter(|(year, _)| YEARS.contains(year))
            .ok_or_else(|| {
                ApiError::BadRequest(format!("semaine inconnue : « {name} » (A1_S12, par exemple)"))
            })?;
        let file = field.file_name().unwrap_or("PDF sans nom").to_owned();
        let bytes = field.bytes().await.map_err(rejected)?;
        let entry = state.store.import(year, week, &file, &bytes).await?;
        tracing::info!(user = user.id, year, week, "PDF déposé");
        imported.push(Imported {
            file,
            year,
            week,
            period: entry.timetable.period.clone(),
            courses: entry.timetable.courses.len(),
        });
    }
    if imported.is_empty() {
        return Err(ApiError::BadRequest("dépose au moins un PDF".into()));
    }
    Ok(Json(imported))
}

/// Un formulaire trop lourd ou mal formé.
fn rejected(error: MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::TooLarge { max_kb: MAX_BODY / 1024 }
    } else {
        ApiError::BadRequest(format!("formulaire illisible : {}", error.body_text()))
    }
}
