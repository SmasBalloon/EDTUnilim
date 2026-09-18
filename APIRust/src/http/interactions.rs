//! Les commandes du bot : `/devoirs`, `/devoir ajouter`, et les boutons.
//!
//! Discord n'appelle pas le bot, il appelle cette route : chaque commande
//! arrive en HTTP, signée, et la réponse part dans le même échange. Pas de
//! connexion permanente à tenir, donc, mais une signature à vérifier — Discord
//! envoie régulièrement de fausses requêtes pour s'assurer qu'on la refuse.
//!
//! Les réponses sont *éphémères* : elles ne s'affichent que pour la personne
//! qui a tapé la commande, même dans un salon partagé. Une liste de devoirs
//! n'a pas à s'étaler dans le fil.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use serde_json::{json, Value};
use time::{Date, Duration, OffsetDateTime};
use tracing::{info, warn};

use crate::error::ApiError;
use crate::homework::{Audience, Homework, NewHomework};
use crate::notify::describe_homework;
use crate::users::User;

use super::homework::set_done;
use super::AppState;

/// Réponse visible de la seule personne qui a lancé la commande.
const EPHEMERAL: u64 = 64;
/// Nombre de devoirs listés d'un coup : au-delà, la liste ne se lit plus.
const LISTED: usize = 15;
/// Discord n'accepte pas plus de cinq boutons par ligne.
const BUTTONS: usize = 5;

/// Ce que Discord envoie. On n'y lit que ce dont on se sert.
#[derive(Debug, Deserialize)]
struct Interaction {
    #[serde(rename = "type")]
    kind: u8,
    #[serde(default)]
    data: Option<Value>,
    /// En message privé : la personne. En serveur : elle est dans `member`.
    #[serde(default)]
    user: Option<DiscordUser>,
    #[serde(default)]
    member: Option<Member>,
}

#[derive(Debug, Deserialize)]
struct Member {
    user: DiscordUser,
}

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
}

impl Interaction {
    fn author(&self) -> Option<&str> {
        self.user.as_ref().or(self.member.as_ref().map(|member| &member.user)).map(|user| user.id.as_str())
    }
}

/// La définition des commandes, telle qu'elle est déclarée à Discord.
pub fn commands() -> Value {
    json!([
        {
            "name": "devoirs",
            "description": "Ce qu'il y a à faire",
            // Dans un serveur, dans le message privé du bot, et dans un groupe.
            "contexts": [0, 1, 2],
            "options": [
                {
                    "type": 3,
                    "name": "matiere",
                    "description": "Filtrer sur une ressource : R1.05",
                    "required": false,
                },
                {
                    "type": 3,
                    "name": "etat",
                    "description": "À faire, faits, ou tout",
                    "required": false,
                    "choices": [
                        { "name": "à faire", "value": "todo" },
                        { "name": "faits", "value": "done" },
                        { "name": "tout", "value": "all" },
                    ],
                },
            ],
        },
        {
            "name": "devoir",
            "description": "Noter un devoir",
            "contexts": [0, 1, 2],
            "options": [{
                "type": 1,
                "name": "ajouter",
                "description": "Noter un devoir pour toi ou pour ta classe",
                "options": [
                    { "type": 3, "name": "intitule", "description": "Ce qu'il y a à faire", "required": true },
                    { "type": 3, "name": "matiere", "description": "La ressource : R1.05", "required": false },
                    { "type": 3, "name": "pour", "description": "La date de rendu : 21/09", "required": false },
                    {
                        "type": 3,
                        "name": "portee",
                        "description": "Qui doit le voir",
                        "required": false,
                        "choices": [
                            { "name": "pour moi seul", "value": "personal" },
                            { "name": "mon demi-groupe", "value": "half" },
                            { "name": "mon groupe", "value": "group" },
                            { "name": "toute l'année", "value": "year" },
                        ],
                    },
                    { "type": 3, "name": "details", "description": "Précisions", "required": false },
                ],
            }],
        },
    ])
}

/// Point d'entrée déclaré dans le portail Discord.
pub async fn handle(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let account = state.account()?;
    let key = account.discord.public_key().ok_or(ApiError::AuthDisabled)?;
    if !verified(key, &headers, &body) {
        // Discord exige un 401 : c'est ainsi qu'il vérifie qu'on ne gobe pas
        // n'importe quoi. Une requête non signée n'est pas une erreur du site.
        return Ok((StatusCode::UNAUTHORIZED, "signature invalide").into_response());
    }

    let interaction: Interaction = serde_json::from_slice(&body)
        .map_err(|error| ApiError::BadRequest(format!("interaction illisible : {error}")))?;

    match interaction.kind {
        // Discord vérifie la route au moment où on la déclare.
        1 => Ok(Json(json!({ "type": 1 })).into_response()),
        2 => Ok(Json(command(&state, &interaction).await).into_response()),
        3 => Ok(Json(button(&state, &interaction).await).into_response()),
        other => {
            warn!(kind = other, "interaction Discord inattendue");
            Ok(Json(reply("Je ne sais pas répondre à cela.")).into_response())
        }
    }
}

/// La signature d'une requête Discord : l'horodatage suivi du corps, le tout
/// signé avec la clé de l'application.
fn verified(public_key: &str, headers: &HeaderMap, body: &[u8]) -> bool {
    let Some(signature) = headers.get("X-Signature-Ed25519").and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let Some(timestamp) = headers.get("X-Signature-Timestamp").and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let (Some(key), Some(signature)) = (from_hex::<32>(public_key), from_hex::<64>(signature)) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key) else { return false };

    let mut message = Vec::with_capacity(timestamp.len() + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(body);
    key.verify(&message, &Signature::from_bytes(&signature)).is_ok()
}

fn from_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 {
        return None;
    }
    let mut bytes = [0u8; N];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair).ok()?;
        bytes[index] = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(bytes)
}

// --- Commandes --------------------------------------------------------------

async fn command(state: &Arc<AppState>, interaction: &Interaction) -> Value {
    let Some(data) = &interaction.data else { return reply("Commande vide.") };
    let name = data.get("name").and_then(Value::as_str).unwrap_or_default();

    let Ok(user) = who(state, interaction).await else {
        return reply(&format!(
            "Je ne te connais pas encore. Connecte-toi avec Discord sur {} et choisis ta classe.",
            state.account().map(|account| account.public_url.as_str()).unwrap_or("le site"),
        ));
    };

    match name {
        "devoirs" => list_command(state, &user, data).await,
        "devoir" => add_command(state, &user, data).await,
        other => {
            warn!(command = other, "commande inconnue");
            reply("Cette commande n'existe pas.")
        }
    }
}

/// `/devoirs [matiere] [etat]`
async fn list_command(state: &Arc<AppState>, user: &User, data: &Value) -> Value {
    let resource = option(data, "matiere").map(|code| code.trim().to_uppercase());
    let state_filter = option(data, "etat").unwrap_or_else(|| "todo".to_owned());

    let Ok(account) = state.account() else { return reply("Les comptes sont éteints ici.") };
    let items: Vec<Homework> = account
        .homework
        .visible_to(user)
        .await
        .into_iter()
        .filter(|item| match resource.as_deref() {
            Some(code) => item.resource.as_deref() == Some(code),
            None => true,
        })
        .filter(|item| match state_filter.as_str() {
            "done" => item.is_done_by(&user.id),
            "all" => true,
            _ => !item.is_done_by(&user.id),
        })
        .collect();

    if items.is_empty() {
        return reply("Rien à faire. 🎉");
    }

    let lines: Vec<String> = items
        .iter()
        .take(LISTED)
        .map(|item| {
            let mark = if item.is_done_by(&user.id) { "✅" } else { "▫️" };
            let pending = if item.is_pending() { " *(à confirmer)*" } else { "" };
            format!("{mark} {}{pending} — *{}*", describe_homework(item), item.audience.label(item.year))
        })
        .collect();

    let mut text = lines.join("\n");
    if items.len() > LISTED {
        text.push_str(&format!("\n… et {} autres.", items.len() - LISTED));
    }

    // Les premiers devoirs non faits se cochent d'un bouton.
    let buttons: Vec<Value> = items
        .iter()
        .filter(|item| !item.is_done_by(&user.id) && !item.is_pending())
        .take(BUTTONS)
        .map(|item| {
            json!({
                "type": 2,
                "style": 1,
                "label": trim_label(&item.summary()),
                "custom_id": format!("hw:done:{}", item.id),
            })
        })
        .collect();

    with_buttons(&text, buttons)
}

/// `/devoir ajouter intitule:… [matiere] [pour] [portee] [details]`
async fn add_command(state: &Arc<AppState>, user: &User, data: &Value) -> Value {
    let Some(sub) = data
        .get("options")
        .and_then(Value::as_array)
        .and_then(|options| options.iter().find(|option| option["name"] == "ajouter"))
    else {
        return reply("Utilisation : `/devoir ajouter intitule:…`");
    };

    let Some((year, group)) = user.class() else {
        return reply("Choisis d'abord ta classe sur le site : un devoir s'adresse à une année.");
    };
    let Some(title) = option(sub, "intitule") else {
        return reply("Il manque l'intitulé du devoir.");
    };

    let due = match option(sub, "pour") {
        Some(text) => match parse_due(&text, OffsetDateTime::now_utc().date()) {
            Some(date) => Some(date),
            None => return reply(&format!("Je ne comprends pas la date « {text} ». Essaie `21/09`.")),
        },
        None => None,
    };

    let audience = match option(sub, "portee").as_deref() {
        Some("group") => Audience::Class { group: Some(base_group(group)) },
        Some("year") => Audience::Class { group: None },
        Some("half") => Audience::Class { group: Some(group.to_owned()) },
        // Sans précision, ce qu'on note ne concerne que soi : c'est le choix
        // qui n'engage personne d'autre.
        _ => Audience::Personal,
    };

    let new = NewHomework {
        year,
        audience,
        title: title.trim().to_owned(),
        details: option(sub, "details"),
        resource: option(sub, "matiere").map(|code| code.trim().to_uppercase()),
        due,
        course: None,
    };
    let personal = new.audience.is_personal();

    let Ok(account) = state.account() else { return reply("Les comptes sont éteints ici.") };
    match account.homework.create(user, new).await {
        Ok(item) => {
            info!(id = item.id, author = user.id, "devoir noté depuis Discord");
            if !personal {
                account.notifier.homework_proposed(&item).await;
                reply(&format!(
                    "📝 Noté {} : {}\nLa classe est prévenue ; il apparaîtra chez tout le monde dès que quelqu'un l'aura confirmé.",
                    item.audience.label(item.year),
                    describe_homework(&item),
                ))
            } else {
                reply(&format!("📝 Noté pour toi : {}", describe_homework(&item)))
            }
        }
        Err(error) => {
            warn!(%error, "devoir non enregistré");
            reply("Je n'ai pas réussi à l'enregistrer. Réessaie dans un instant.")
        }
    }
}

// --- Boutons ----------------------------------------------------------------

async fn button(state: &Arc<AppState>, interaction: &Interaction) -> Value {
    let Some(id) = interaction.data.as_ref().and_then(|data| data["custom_id"].as_str()) else {
        return reply("Ce bouton ne dit pas ce qu'il fait.");
    };
    let Ok(user) = who(state, interaction).await else {
        return reply("Je ne te connais pas encore : connecte-toi sur le site.");
    };
    let Ok(account) = state.account() else { return reply("Les comptes sont éteints ici.") };

    let mut parts = id.split(':');
    let (Some("hw"), Some(action), Some(homework_id)) = (parts.next(), parts.next(), parts.next()) else {
        return reply("Bouton inconnu.");
    };
    let Some(item) = account.homework.get(homework_id).await else {
        return reply("Ce devoir n'existe plus.");
    };

    match action {
        "done" => {
            if !item.visible_to(&user) {
                return reply("Ce devoir ne s'adresse pas à ta classe.");
            }
            match account.homework.update(homework_id, |item| set_done(item, &user.id, true)).await {
                Ok(_) => reply(&format!("✅ C'est coché : {}", item.summary())),
                Err(_) => reply("Je n'ai pas réussi à le cocher."),
            }
        }
        "confirm" => {
            if !item.confirmable_by(&user) {
                return reply("Ce devoir est déjà confirmé, ou c'est toi qui l'as noté.");
            }
            let confirmed = account
                .homework
                .update(homework_id, |item| {
                    item.confirmed_by = Some(user.id.clone());
                    item.confirmed_name = Some(user.name.clone());
                })
                .await;
            match confirmed {
                Ok(Some(item)) => {
                    info!(id = item.id, by = user.id, "devoir confirmé depuis Discord");
                    account.notifier.homework_confirmed(&item).await;
                    reply(&format!("✅ Confirmé : {}. La classe est prévenue.", item.summary()))
                }
                _ => reply("Je n'ai pas réussi à le confirmer."),
            }
        }
        other => {
            warn!(action = other, "bouton inconnu");
            reply("Bouton inconnu.")
        }
    }
}

// --- Petites mains ----------------------------------------------------------

/// La personne connectée derrière cet identifiant Discord.
async fn who(state: &Arc<AppState>, interaction: &Interaction) -> Result<User, ()> {
    let account = state.account().map_err(|_| ())?;
    let id = interaction.author().ok_or(())?;
    account.users.get(id).await.ok_or(())
}

/// Une option de commande, par son nom.
fn option(data: &Value, name: &str) -> Option<String> {
    data.get("options")?
        .as_array()?
        .iter()
        .find(|option| option["name"] == name)?
        .get("value")?
        .as_str()
        .map(str::to_owned)
}

/// `G1a` → `G1` : la portée « mon groupe » vise les deux moitiés.
fn base_group(group: &str) -> String {
    match group.chars().last() {
        Some(last) if last.is_ascii_alphabetic() && group.len() > 2 => group[..group.len() - 1].to_owned(),
        _ => group.to_owned(),
    }
}

/// `21/09`, `21/09/2026`, ou `2026-09-21`.
///
/// Sans année, c'est la prochaine occurrence : en décembre, « 05/01 » désigne
/// janvier prochain, pas celui qui vient de passer.
fn parse_due(text: &str, today: Date) -> Option<Date> {
    let text = text.trim();
    if let Ok(date) = Date::parse(text, &crate::model::iso_date::FORMAT) {
        return Some(date);
    }

    let mut parts = text.split(['/', '.', '-']);
    let day: u8 = parts.next()?.trim().parse().ok()?;
    let month: u8 = parts.next()?.trim().parse().ok()?;
    let month = time::Month::try_from(month).ok()?;

    match parts.next() {
        Some(year) => {
            let year: i32 = year.trim().parse().ok()?;
            let year = if year < 100 { 2000 + year } else { year };
            Date::from_calendar_date(year, month, day).ok()
        }
        None => {
            let this_year = Date::from_calendar_date(today.year(), month, day).ok()?;
            Some(if this_year < today - Duration::days(1) {
                Date::from_calendar_date(today.year() + 1, month, day).ok()?
            } else {
                this_year
            })
        }
    }
}

/// Un intitulé assez court pour tenir sur un bouton Discord (80 caractères).
fn trim_label(summary: &str) -> String {
    let label = format!("Fait : {summary}");
    match label.char_indices().nth(78) {
        Some((cut, _)) => format!("{}…", &label[..cut]),
        None => label,
    }
}

fn reply(text: &str) -> Value {
    json!({ "type": 4, "data": { "content": text, "flags": EPHEMERAL } })
}

fn with_buttons(text: &str, buttons: Vec<Value>) -> Value {
    let mut data = json!({ "content": text, "flags": EPHEMERAL });
    if !buttons.is_empty() {
        data["components"] = json!([{ "type": 1, "components": buttons }]);
    }
    json!({ "type": 4, "data": data })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn reads_the_next_occurrence_of_a_day() {
        let today = date!(2026 - 09 - 18);
        assert_eq!(parse_due("21/09", today), Some(date!(2026 - 09 - 21)));
        assert_eq!(parse_due("21/09/2027", today), Some(date!(2027 - 09 - 21)));
        assert_eq!(parse_due("2026-10-02", today), Some(date!(2026 - 10 - 02)));
        // Une date déjà passée cette année désigne l'année suivante.
        assert_eq!(parse_due("05/01", today), Some(date!(2027 - 01 - 05)));
        // La veille reste acceptée : on note parfois un devoir en retard.
        assert_eq!(parse_due("17/09", today), Some(date!(2026 - 09 - 17)));
        assert_eq!(parse_due("demain", today), None);
    }

    #[test]
    fn groups_lose_their_half() {
        assert_eq!(base_group("G1a"), "G1");
        assert_eq!(base_group("G1"), "G1");
        assert_eq!(base_group("G10b"), "G10");
    }

    #[test]
    fn labels_fit_on_a_button() {
        assert_eq!(trim_label("R1.05 — court"), "Fait : R1.05 — court");
        assert!(trim_label(&"x".repeat(200)).chars().count() <= 79);
    }

    #[test]
    fn refuses_what_is_not_signed() {
        let headers = HeaderMap::new();
        assert!(!verified(&"ab".repeat(32), &headers, b"{}"), "sans en-tête, rien ne passe");
    }

    #[test]
    fn reads_hexadecimal_keys() {
        assert_eq!(from_hex::<2>("00ff"), Some([0, 255]));
        assert_eq!(from_hex::<2>("00f"), None);
        assert_eq!(from_hex::<2>("00zz"), None);
    }
}
