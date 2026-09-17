//! Consultation des emplois du temps.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::header::CACHE_CONTROL;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::config::YEARS;
use crate::error::ApiError;
use crate::model::{Course, Period};
use crate::upstream::week_url;

use super::{AppState, PathParams};

/// Les emplois du temps changent au fil de la semaine, sans urgence.
const CACHE_POLICY: &str = "public, max-age=300";

#[derive(Serialize)]
pub struct YearSummary {
    year: u8,
    weeks: Vec<u8>,
}

/// Un client qui recharge explicitement (`fetch(…, { cache: 'reload' })`,
/// Ctrl+Maj+R) envoie `Cache-Control: no-cache` : il veut la version du
/// département, pas celle gardée en mémoire.
fn wants_fresh(headers: &HeaderMap) -> bool {
    headers
        .get_all(CACHE_CONTROL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|directive| matches!(directive.trim().to_ascii_lowercase().as_str(), "no-cache" | "max-age=0"))
}

/// Années publiées, avec les semaines disponibles pour chacune.
pub async fn years(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<YearSummary>>, ApiError> {
    let mut summaries = Vec::with_capacity(YEARS.len());
    for year in YEARS {
        if wants_fresh(&headers) {
            state.store.revalidate_weeks(year).await;
        }
        summaries.push(YearSummary { year, weeks: state.store.weeks(year).await?.to_vec() });
    }
    Ok(Json(summaries))
}

/// Semaines disponibles pour une année.
pub async fn weeks(
    State(state): State<Arc<AppState>>,
    PathParams(year): PathParams<u8>,
    headers: HeaderMap,
) -> Result<Json<YearSummary>, ApiError> {
    if wants_fresh(&headers) {
        state.store.revalidate_weeks(year).await;
    }
    let weeks = state.store.weeks(year).await?.to_vec();
    Ok(Json(YearSummary { year, weeks }))
}

#[derive(Debug, Deserialize)]
pub struct Filters {
    /// Groupe ou demi-groupe (`G1`, `G1a`).
    group: Option<String>,
    /// Jour de la semaine (`lundi`).
    day: Option<String>,
}

#[derive(Serialize)]
pub struct Source {
    url: String,
    #[serde(with = "time::serde::rfc3339")]
    fetched_at: OffsetDateTime,
}

#[derive(Serialize)]
pub struct WeekResponse<'a> {
    year: u8,
    week: u8,
    period: &'a Period,
    source: Source,
    count: usize,
    courses: Vec<&'a Course>,
}

/// Emploi du temps d'une semaine, éventuellement filtré.
pub async fn week(
    State(state): State<Arc<AppState>>,
    PathParams((year, week)): PathParams<(u8, u8)>,
    Query(filters): Query<Filters>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if wants_fresh(&headers) {
        state.store.revalidate_timetable(year, week).await;
    }
    let entry = state.store.timetable(year, week).await?;
    let timetable = &entry.timetable;

    let courses: Vec<&Course> = timetable
        .courses
        .iter()
        .filter(|course| filters.day.as_deref().is_none_or(|day| matches_day(&course.day, day)))
        .filter(|course| {
            filters
                .group
                .as_deref()
                .is_none_or(|wanted| course.groups.iter().any(|group| matches_group(group, wanted)))
        })
        .collect();

    let body = WeekResponse {
        year: timetable.year,
        week: timetable.week,
        period: &timetable.period,
        source: Source { url: week_url(&state.upstream, year, week), fetched_at: entry.fetched_at },
        count: courses.len(),
        courses,
    };

    Ok(([(CACHE_CONTROL, CACHE_POLICY)], Json(body)).into_response())
}

fn matches_day(day: &str, wanted: &str) -> bool {
    day.eq_ignore_ascii_case(wanted.trim())
}

/// Un créneau de `G1` concerne aussi bien `G1a` que `G1b`, et réciproquement :
/// demander `G1a` doit rendre les cours du groupe entier.
fn matches_group(group: &str, wanted: &str) -> bool {
    let (group, wanted) = (group.trim(), wanted.trim());
    let common = group.len().min(wanted.len());
    common > 0
        && group.is_char_boundary(common)
        && wanted.is_char_boundary(common)
        && group[..common].eq_ignore_ascii_case(&wanted[..common])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_filter_spans_half_groups() {
        assert!(matches_group("G1", "G1a"));
        assert!(matches_group("G1a", "G1"));
        assert!(matches_group("G1a", "g1a"));
        assert!(!matches_group("G1a", "G1b"));
        assert!(!matches_group("G2", "G1"));
    }

    #[test]
    fn detects_an_explicit_reload() {
        let mut headers = HeaderMap::new();
        assert!(!wants_fresh(&headers));
        headers.insert(CACHE_CONTROL, "max-age=0".parse().unwrap());
        assert!(wants_fresh(&headers));
        headers.insert(CACHE_CONTROL, "public, No-Cache".parse().unwrap());
        assert!(wants_fresh(&headers));
        headers.insert(CACHE_CONTROL, "max-age=300".parse().unwrap());
        assert!(!wants_fresh(&headers));
    }

    #[test]
    fn day_filter_ignores_case() {
        assert!(matches_day("LUNDI", "lundi"));
        assert!(!matches_day("LUNDI", "mardi"));
    }
}
