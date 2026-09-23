//! Points d'entrée de service : présentation, santé, référentiel.

use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};

use crate::catalog::{self, CatalogEntry};
use crate::teachers::{self, Teacher};

/// Présente les routes disponibles.
pub async fn index() -> Json<Value> {
    Json(json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "routes": {
            "GET /health": "état du service",
            "GET /api/v1/years": "années publiées et semaines disponibles",
            "GET /api/v1/years/{year}/weeks": "semaines disponibles pour une année",
            "GET /api/v1/years/{year}/weeks/{week}": "emploi du temps ; filtres ?group=G1a et ?day=lundi",
            "GET /api/v1/years/{year}/calendar/{group}.ics":
                "abonnement iCalendar (webcal) d'un demi-groupe, toutes semaines publiées",
            "GET /api/v1/resources": "référentiel des ressources et SAÉ",
            "GET /api/v1/teachers": "enseignants connus, et leurs autres écritures",
            "GET /api/v1/auth/login": "connexion avec Discord",
            "POST /api/v1/auth/logout": "déconnexion",
            "GET|PUT|DELETE /api/v1/me": "compte : classe suivie et messages privés",
        },
    }))
}

#[derive(Serialize)]
pub struct Health {
    status: &'static str,
    version: &'static str,
}

pub async fn health() -> Json<Health> {
    Json(Health { status: "ok", version: env!("CARGO_PKG_VERSION") })
}

pub async fn resources() -> Json<&'static [CatalogEntry]> {
    Json(catalog::entries())
}

pub async fn teachers() -> Json<&'static [Teacher]> {
    Json(teachers::entries())
}
