//! Connexion avec Discord et compte : classe suivie, messages privés.

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Query, State};
use axum::http::header::{LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::{info, warn};

use crate::config::YEARS;
use crate::discord::Discord;
use crate::error::ApiError;
use crate::homework::HomeworkStore;
use crate::model::normalise_group;
use crate::notify::Notifier;
use crate::session::{self, Signer, OAUTH_COOKIE, OAUTH_TTL, SESSION_COOKIE, SESSION_TTL};
use crate::users::{User, UserStore};

use super::AppState;

/// Ce qu'il faut pour connecter les gens et leur écrire.
pub struct Account {
    pub discord: Discord,
    pub users: Arc<UserStore>,
    /// Les devoirs notés par les personnes connectées.
    pub homework: Arc<HomeworkStore>,
    pub notifier: Notifier,
    pub signer: Signer,
    /// Adresse publique du site, sans barre oblique finale.
    pub public_url: String,
    /// Cookies `Secure` quand le site est en HTTPS.
    pub secure: bool,
    /// Comptes qui gèrent les PDF en cache (`EDT_DISCORD_ADMINS`).
    pub admins: Vec<String>,
}

impl Account {
    fn redirect_uri(&self) -> String {
        format!("{}/api/v1/auth/callback", self.public_url)
    }

    /// Retour vers la page du compte, avec un code d'erreur éventuel.
    fn back_to_account(&self, error: Option<&str>, cookies: &[String]) -> Response {
        let target = match error {
            Some(code) => format!("/#compte?erreur={code}"),
            None => "/#compte".to_owned(),
        };
        let mut response = (StatusCode::SEE_OTHER, [(LOCATION, target)]).into_response();
        for cookie in cookies {
            response.headers_mut().append(SET_COOKIE, cookie.parse().expect("cookie ASCII"));
        }
        response
    }

    /// La personne connectée, d'après le cookie de session.
    pub async fn user_from(&self, headers: &HeaderMap) -> Result<User, ApiError> {
        let token = session::read_cookie(headers, SESSION_COOKIE).ok_or(ApiError::Unauthenticated)?;
        let id = self.signer.verify(&token).ok_or(ApiError::Unauthenticated)?;
        self.users.get(&id).await.ok_or(ApiError::Unauthenticated)
    }

    /// La personne connectée, si elle gère les PDF en cache.
    pub async fn admin_from(&self, headers: &HeaderMap) -> Result<User, ApiError> {
        let user = self.user_from(headers).await?;
        if self.is_admin(&user) {
            Ok(user)
        } else {
            Err(ApiError::Forbidden("réservé aux administrateurs"))
        }
    }

    fn is_admin(&self, user: &User) -> bool {
        self.admins.contains(&user.id)
    }

    fn view(&self, user: User) -> Me {
        let admin = self.is_admin(&user);
        Me { admin, ..Me::from(user) }
    }

    fn clear_session(&self) -> String {
        session::cookie(SESSION_COOKIE, "", 0, self.secure)
    }
}

fn account(state: &AppState) -> Result<&Account, ApiError> {
    state.account.as_deref().ok_or(ApiError::AuthDisabled)
}

/// Part vers la page d'autorisation Discord.
pub async fn login(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    let account = account(&state)?;
    let nonce = session::nonce();
    let cookie =
        session::cookie(OAUTH_COOKIE, &account.signer.sign(&nonce, OAUTH_TTL), OAUTH_TTL, account.secure);
    let target = account.discord.authorize_url(&account.redirect_uri(), &nonce);
    Ok((StatusCode::SEE_OTHER, [(LOCATION, target), (SET_COOKIE, cookie)]).into_response())
}

#[derive(Deserialize)]
pub struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// Retour de Discord : vérifie l'aller-retour, ouvre la session.
pub async fn callback(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Result<Response, ApiError> {
    let account = account(&state)?;
    let clear_oauth = session::cookie(OAUTH_COOKIE, "", 0, account.secure);

    // Le nonce du cookie doit être celui renvoyé par Discord : sans cela, un
    // tiers pourrait faire atterrir quelqu'un dans son propre compte.
    let expected =
        session::read_cookie(&headers, OAUTH_COOKIE).and_then(|token| account.signer.verify(&token));
    if expected.is_none() || expected != query.state {
        return Ok(account.back_to_account(Some("session"), &[clear_oauth]));
    }
    if query.error.is_some() {
        return Ok(account.back_to_account(Some("annule"), &[clear_oauth]));
    }
    let Some(code) = query.code else {
        return Ok(account.back_to_account(Some("session"), &[clear_oauth]));
    };

    let redirect_uri = account.redirect_uri();
    let signed_in = async {
        let token = account.discord.exchange_code(&code, &redirect_uri).await?;
        let profile = account.discord.current_user(&token).await?;
        // Sans serveur commun, le bot ne pourra pas écrire. Une personne déjà
        // membre d'un serveur connu du bot n'est pas ajoutée à un autre ; pour
        // les autres, l'échec de l'ajout est signalé sans bloquer la connexion.
        if let Some(guild_id) = account.discord.known_guild_of(&profile.id).await {
            info!(user = profile.id, guild_id, "déjà membre d'un serveur du bot, pas d'ajout");
        } else if let Err(error) = account.discord.join_guild(&profile.id, &token).await {
            warn!(%error, user = profile.id, "ajout au serveur Discord impossible");
        }
        Ok::<_, crate::discord::DiscordError>(profile)
    }
    .await;

    let profile = match signed_in {
        Ok(profile) => profile,
        Err(error) => {
            warn!(%error, "connexion Discord en échec");
            return Ok(account.back_to_account(Some("discord"), &[clear_oauth]));
        }
    };

    let user = account.users.sign_in(&profile.id, profile.display_name(), profile.avatar.clone()).await?;
    info!(user = user.id, "connexion");
    let session = session::cookie(
        SESSION_COOKIE,
        &account.signer.sign(&user.id, SESSION_TTL),
        SESSION_TTL,
        account.secure,
    );
    Ok(account.back_to_account(None, &[clear_oauth, session]))
}

#[derive(Serialize)]
pub struct Me {
    id: String,
    name: String,
    avatar_url: Option<String>,
    year: Option<u8>,
    group: Option<String>,
    notify: bool,
    /// Gère les PDF en cache : la page d'import lui est ouverte.
    admin: bool,
}

impl From<User> for Me {
    fn from(user: User) -> Self {
        let avatar_url = user
            .avatar
            .as_ref()
            .map(|hash| format!("https://cdn.discordapp.com/avatars/{}/{hash}.png?size=96", user.id));
        Self {
            id: user.id,
            name: user.name,
            avatar_url,
            year: user.year,
            group: user.group,
            notify: user.notify,
            admin: false,
        }
    }
}

/// La personne connectée.
pub async fn me(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let account = account(&state)?;
    let user = account.user_from(&headers).await?;
    Ok(([(axum::http::header::CACHE_CONTROL, "no-store")], Json(account.view(user))).into_response())
}

#[derive(Deserialize)]
pub struct Update {
    year: Option<u8>,
    group: Option<String>,
    notify: bool,
}

/// Issue du message privé de confirmation.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// Aucun message à envoyer : rien n'a changé, ou les messages sont coupés.
    None,
    Sent,
    /// La personne n'accepte pas les messages privés du serveur.
    Closed,
    Failed,
}

#[derive(Serialize)]
pub struct Updated {
    user: Me,
    dm: Delivery,
}

/// Choisit la classe et les messages privés.
pub async fn update(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Result<Json<Update>, JsonRejection>,
) -> Result<Json<Updated>, ApiError> {
    let account = account(&state)?;
    let Json(body) = body.map_err(|rejection| ApiError::BadRequest(rejection.body_text()))?;
    let before = account.user_from(&headers).await?;

    if body.year.is_some_and(|year| !YEARS.contains(&year)) {
        return Err(ApiError::BadRequest("année inconnue".to_owned()));
    }
    let group = match body.group.as_deref() {
        None => None,
        Some(raw) => Some(
            normalise_group(raw).ok_or_else(|| ApiError::BadRequest(format!("groupe invalide : {raw}")))?,
        ),
    };
    if body.year.is_some() != group.is_some() {
        return Err(ApiError::BadRequest("l'année et le groupe vont ensemble".to_owned()));
    }

    let user = account
        .users
        .update(&before.id, |user| {
            user.year = body.year;
            user.group = group.clone();
            user.notify = body.notify;
        })
        .await?
        .ok_or(ApiError::Unauthenticated)?;

    // Confirmation seulement quand l'abonnement commence ou change de classe.
    let subscribed = user.notify && user.class().is_some();
    let changed = !before.notify || before.class() != user.class();
    let dm = if subscribed && changed {
        let (year, _) = user.class().expect("classe vérifiée");
        let today = OffsetDateTime::now_utc().date();
        let current = state.store.upcoming_week(year, today).await;
        match account.notifier.welcome(&user, current.as_ref().map(|entry| &entry.timetable)).await {
            Ok(()) => Delivery::Sent,
            Err(error) if error.is_dm_closed() => Delivery::Closed,
            Err(error) => {
                warn!(%error, user = user.id, "message de bienvenue impossible");
                Delivery::Failed
            }
        }
    } else {
        Delivery::None
    };

    Ok(Json(Updated { user: account.view(user), dm }))
}

/// Ferme la session.
pub async fn logout(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    let account = account(&state)?;
    Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, account.clear_session())]).into_response())
}

/// Supprime le compte et ferme la session.
pub async fn delete(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let account = account(&state)?;
    let user = account.user_from(&headers).await?;
    account.users.delete(&user.id).await?;
    info!(user = user.id, "compte supprimé");
    Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, account.clear_session())]).into_response())
}
