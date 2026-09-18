//! Les devoirs : lecture, écriture, confirmation et pointage.
//!
//! Toutes ces routes demandent d'être connecté : un devoir appartient à
//! quelqu'un, et sa portée se juge à la classe de celui qui le lit.

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use time::Date;
use tracing::info;

use crate::error::ApiError;
use crate::homework::{apply, Audience, CourseRef, Edit, Homework, NewHomework};
use crate::model::matches_group;
use crate::users::User;

use super::{AppState, PathParams};

/// Longueurs au-delà desquelles ce n'est plus un devoir mais un roman.
const TITLE_MAX: usize = 200;
const DETAILS_MAX: usize = 2000;

/// Un devoir tel qu'il se lit, avec ce que cette personne peut en faire.
#[derive(Debug, Serialize)]
pub struct View {
    #[serde(flatten)]
    item: Homework,
    /// En attente d'une confirmation par quelqu'un d'autre.
    pending: bool,
    /// Coché « fait » par la personne qui lit.
    done_by_me: bool,
    /// Elle l'a noté elle-même : elle peut le corriger et l'effacer.
    mine: bool,
    /// Elle peut le confirmer.
    can_confirm: bool,
}

impl View {
    fn of(item: Homework, user: &User) -> Self {
        Self {
            pending: item.is_pending(),
            done_by_me: item.is_done_by(&user.id),
            mine: item.editable_by(user),
            can_confirm: item.confirmable_by(user),
            item,
        }
    }
}

/// Ce que le site envoie pour noter ou corriger un devoir.
#[derive(Debug, Deserialize)]
pub struct Body {
    pub title: String,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default, with = "crate::model::opt_iso_date")]
    pub due: Option<Date>,
    pub audience: Audience,
    /// Créneau de rattachement ; `null` pour le laisser à plus tard.
    #[serde(default)]
    pub course: Option<CourseRef>,
}

#[derive(Debug, Deserialize)]
pub struct DoneBody {
    pub done: bool,
}

/// Les devoirs que cette personne voit.
pub async fn list(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<View>>, ApiError> {
    let account = state.account()?;
    let user = account.user_from(&headers).await?;
    let items = account.homework.visible_to(&user).await;
    Ok(Json(items.into_iter().map(|item| View::of(item, &user)).collect()))
}

/// Note un devoir.
pub async fn create(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Result<Json<Body>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<View>, ApiError> {
    let account = state.account()?;
    let user = account.user_from(&headers).await?;
    let Json(body) = body.map_err(|error| ApiError::BadRequest(error.body_text()))?;
    let (year, _) = user.class().ok_or_else(|| {
        ApiError::BadRequest("choisis d'abord ta classe : un devoir s'adresse à une année".into())
    })?;

    let new = NewHomework {
        year,
        audience: check_audience(body.audience, &user)?,
        title: check_title(&body.title)?,
        details: check_details(body.details)?,
        resource: clean_resource(body.resource),
        due: body.due,
        course: body.course,
    };
    let personal = new.audience.is_personal();
    let item = account.homework.create(&user, new).await?;
    info!(id = item.id, author = user.id, personal, "devoir noté");

    // Un devoir de classe cherche sa confirmation dès qu'il est noté.
    if !personal {
        account.notifier.homework_proposed(&item).await;
    }
    Ok(Json(View::of(item, &user)))
}

/// Corrige un devoir : seul son auteur le peut.
pub async fn edit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    PathParams(id): PathParams<String>,
    body: Result<Json<Body>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<View>, ApiError> {
    let account = state.account()?;
    let user = account.user_from(&headers).await?;
    let Json(body) = body.map_err(|error| ApiError::BadRequest(error.body_text()))?;

    let item = account.homework.get(&id).await.ok_or(ApiError::UnknownRoute)?;
    if !item.editable_by(&user) {
        return Err(ApiError::Forbidden("ce devoir a été noté par quelqu'un d'autre"));
    }

    let edit = Edit {
        title: Some(check_title(&body.title)?),
        details: Some(check_details(body.details)?),
        resource: Some(clean_resource(body.resource)),
        due: Some(body.due),
        course: Some(body.course),
        audience: Some(check_audience(body.audience, &user)?),
    };
    let updated =
        account.homework.update(&id, |item| apply(item, edit)).await?.ok_or(ApiError::UnknownRoute)?;

    // Devenu un devoir de classe, il repart en attente de confirmation.
    if updated.is_pending() && !item.is_pending() {
        account.notifier.homework_proposed(&updated).await;
    }
    Ok(Json(View::of(updated, &user)))
}

/// Efface un devoir : seul son auteur le peut.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    PathParams(id): PathParams<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let account = state.account()?;
    let user = account.user_from(&headers).await?;
    let item = account.homework.get(&id).await.ok_or(ApiError::UnknownRoute)?;
    if !item.editable_by(&user) {
        return Err(ApiError::Forbidden("ce devoir a été noté par quelqu'un d'autre"));
    }
    account.homework.delete(&id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Confirme un devoir de classe : il devient officiel.
pub async fn confirm(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    PathParams(id): PathParams<String>,
) -> Result<Json<View>, ApiError> {
    let account = state.account()?;
    let user = account.user_from(&headers).await?;
    let item = account.homework.get(&id).await.ok_or(ApiError::UnknownRoute)?;
    if !item.confirmable_by(&user) {
        return Err(ApiError::Forbidden("ce devoir est déjà confirmé, ou c'est toi qui l'as noté"));
    }

    let confirmed = account
        .homework
        .update(&id, |item| {
            item.confirmed_by = Some(user.id.clone());
            item.confirmed_name = Some(user.name.clone());
        })
        .await?
        .ok_or(ApiError::UnknownRoute)?;
    info!(id = confirmed.id, by = user.id, "devoir confirmé");
    account.notifier.homework_confirmed(&confirmed).await;
    Ok(Json(View::of(confirmed, &user)))
}

/// Coche ou décoche un devoir, pour soi seul.
pub async fn done(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    PathParams(id): PathParams<String>,
    body: Result<Json<DoneBody>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<View>, ApiError> {
    let account = state.account()?;
    let user = account.user_from(&headers).await?;
    let Json(body) = body.map_err(|error| ApiError::BadRequest(error.body_text()))?;
    let item = account.homework.get(&id).await.ok_or(ApiError::UnknownRoute)?;
    if !item.visible_to(&user) {
        return Err(ApiError::Forbidden("ce devoir ne s'adresse pas à ta classe"));
    }

    let updated = account
        .homework
        .update(&id, |item| set_done(item, &user.id, body.done))
        .await?
        .ok_or(ApiError::UnknownRoute)?;
    Ok(Json(View::of(updated, &user)))
}

/// Coche ou décoche, sans jamais compter deux fois la même personne.
pub fn set_done(item: &mut Homework, user_id: &str, done: bool) {
    item.done.retain(|id| id != user_id);
    if done {
        item.done.push(user_id.to_owned());
    }
}

// --- Contrôles --------------------------------------------------------------

fn check_title(title: &str) -> Result<String, ApiError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ApiError::BadRequest("un devoir a besoin d'un intitulé".into()));
    }
    if title.chars().count() > TITLE_MAX {
        return Err(ApiError::BadRequest(format!("l'intitulé dépasse {TITLE_MAX} caractères")));
    }
    Ok(title.to_owned())
}

fn check_details(details: Option<String>) -> Result<Option<String>, ApiError> {
    let details = details.map(|text| text.trim().to_owned()).filter(|text| !text.is_empty());
    match &details {
        Some(text) if text.chars().count() > DETAILS_MAX => {
            Err(ApiError::BadRequest(format!("les précisions dépassent {DETAILS_MAX} caractères")))
        }
        _ => Ok(details),
    }
}

/// `r1.05` → `R1.05`. Le code n'est pas vérifié au référentiel : le
/// département en invente parfois, et un devoir ne doit pas s'en trouver bloqué.
fn clean_resource(resource: Option<String>) -> Option<String> {
    resource
        .map(|code| code.trim().to_uppercase())
        .filter(|code| !code.is_empty() && code.chars().count() <= 16)
}

/// On ne note un devoir que pour sa propre classe.
///
/// Sans quoi n'importe qui pourrait remplir la liste du groupe d'à côté — et
/// personne là-bas ne saurait d'où ça sort.
fn check_audience(audience: Audience, user: &User) -> Result<Audience, ApiError> {
    let Audience::Class { group: Some(group) } = &audience else { return Ok(audience) };
    let group = group.trim().to_owned();
    let theirs = user.group.as_deref().unwrap_or_default();
    if !matches_group(&group, theirs) {
        return Err(ApiError::BadRequest(format!(
            "tu es en {} : tu ne peux pas noter un devoir pour {}",
            theirs.to_uppercase(),
            group.to_uppercase(),
        )));
    }
    Ok(Audience::Class { group: Some(group) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn user(group: &str) -> User {
        User {
            id: "1".into(),
            name: "Ada".into(),
            avatar: None,
            year: Some(1),
            group: Some(group.into()),
            notify: true,
            created_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn a_note_stays_within_ones_own_class() {
        let ada = user("G1a");
        // Son demi-groupe, son groupe et son année : tout est permis.
        for group in [Some("G1a"), Some("G1"), None] {
            let audience = Audience::Class { group: group.map(str::to_owned) };
            assert!(check_audience(audience, &ada).is_ok());
        }
        // Le groupe d'à côté, non.
        let other = Audience::Class { group: Some("G2".into()) };
        assert!(check_audience(other, &ada).is_err());
    }

    #[test]
    fn titles_are_trimmed_and_bounded() {
        assert_eq!(check_title("  Exercices 3 à 7 ").expect("intitulé"), "Exercices 3 à 7");
        assert!(check_title("   ").is_err());
        assert!(check_title(&"a".repeat(TITLE_MAX + 1)).is_err());
    }

    #[test]
    fn resource_codes_are_normalised() {
        assert_eq!(clean_resource(Some(" r1.05 ".into())), Some("R1.05".into()));
        assert_eq!(clean_resource(Some("  ".into())), None);
        assert_eq!(clean_resource(None), None);
    }

    #[test]
    fn ticking_twice_counts_once() {
        let mut item = Homework {
            id: "x".into(),
            author: "1".into(),
            author_name: "Ada".into(),
            year: 1,
            audience: Audience::Personal,
            title: "Exercices".into(),
            details: None,
            resource: None,
            due: None,
            course: None,
            created_at: OffsetDateTime::now_utc(),
            confirmed_by: None,
            confirmed_name: None,
            done: Vec::new(),
        };
        set_done(&mut item, "1", true);
        set_done(&mut item, "1", true);
        assert_eq!(item.done, vec!["1".to_owned()]);
        set_done(&mut item, "1", false);
        assert!(item.done.is_empty());
    }
}
