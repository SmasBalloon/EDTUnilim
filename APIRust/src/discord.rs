//! Client de l'API Discord : connexion OAuth2, ajout au serveur et messages
//! privés envoyés par le bot.

use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::multipart::{Form, Part};
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

use crate::config::DiscordConfig;

/// Page d'autorisation, toujours servie par discord.com.
const AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";
/// `identify` pour savoir qui se connecte, `guilds.join` pour l'ajouter au
/// serveur du bot : sans serveur commun, Discord refuse les messages privés.
const SCOPES: &str = "identify guilds.join";
/// Nombre de nouvelles tentatives après un 429.
const RATE_LIMIT_RETRIES: usize = 3;

#[derive(Debug, thiserror::Error)]
pub enum DiscordError {
    #[error("Discord injoignable : {0}")]
    Http(#[from] reqwest::Error),
    #[error("Discord a refusé la requête ({status}) : {message}")]
    Refused { status: StatusCode, code: Option<u64>, message: String },
    #[error("réponse Discord illisible : {0}")]
    Unexpected(#[from] serde_json::Error),
}

impl DiscordError {
    /// Discord refuse d'écrire à cette personne (messages privés fermés).
    pub fn is_dm_closed(&self) -> bool {
        matches!(self, Self::Refused { code: Some(50007), .. })
    }
}

#[derive(Debug, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

impl DiscordUser {
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().unwrap_or(&self.username)
    }
}

/// Un bouton sous un message.
#[derive(Debug, Clone)]
pub struct Button {
    /// Ce que le bouton déclenche, relu tel quel à l'appui : `hw:done:<id>`.
    pub id: String,
    pub label: String,
    pub style: ButtonStyle,
}

#[derive(Debug, Clone, Copy)]
pub enum ButtonStyle {
    /// Bleu : l'action attendue.
    Primary,
    /// Vert : ce qui acquiesce.
    Success,
    /// Gris : ce qui n'engage à rien.
    Secondary,
}

impl ButtonStyle {
    fn code(self) -> u8 {
        match self {
            Self::Primary => 1,
            Self::Secondary => 2,
            Self::Success => 3,
        }
    }
}

fn action_row(buttons: &[Button]) -> Vec<serde_json::Value> {
    buttons
        .iter()
        .map(|button| json!({
            "type": 2,
            "style": button.style.code(),
            "label": button.label,
            "custom_id": button.id,
        }))
        .collect()
}

/// Une pièce jointe : nom de fichier et contenu.
pub struct Attachment {
    pub filename: String,
    pub content: Vec<u8>,
}

#[derive(Clone)]
pub struct Discord {
    client: reqwest::Client,
    config: DiscordConfig,
}

impl Discord {
    pub fn new(config: DiscordConfig) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(concat!("DiscordBot (edt-unilim, ", env!("CARGO_PKG_VERSION"), ")"))
            .build()?;
        Ok(Self { client, config })
    }

    /// Adresse de la page d'autorisation Discord.
    pub fn authorize_url(&self, redirect_uri: &str, state: &str) -> String {
        format!(
            "{AUTHORIZE_URL}?response_type=code&client_id={}&scope={}&redirect_uri={}&state={}&prompt=none",
            encode(&self.config.client_id),
            encode(SCOPES),
            encode(redirect_uri),
            encode(state)
        )
    }

    /// Échange le code de retour contre un jeton d'accès de la personne.
    pub async fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<String, DiscordError> {
        #[derive(Deserialize)]
        struct Token {
            access_token: String,
        }
        let request = self
            .client
            .post(format!("{}/oauth2/token", self.config.api_base))
            .basic_auth(&self.config.client_id, Some(&self.config.client_secret))
            .form(&[("grant_type", "authorization_code"), ("code", code), ("redirect_uri", redirect_uri)]);
        let token: Token = parse(self.send(request).await?).await?;
        Ok(token.access_token)
    }

    pub async fn current_user(&self, access_token: &str) -> Result<DiscordUser, DiscordError> {
        let request = self
            .client
            .get(format!("{}/users/@me", self.config.api_base))
            .header(AUTHORIZATION, format!("Bearer {access_token}"));
        parse(self.send(request).await?).await
    }

    /// Serveurs connus dont la personne est membre, dans l'ordre de la configuration.
    pub async fn known_guild_of(&self, user_id: &str) -> Option<&str> {
        for guild_id in &self.config.known_guild_ids {
            let request = self.bot(
                self.client.get(format!("{}/guilds/{guild_id}/members/{user_id}", self.config.api_base)),
            );
            match self.send(request).await {
                Ok(_) => return Some(guild_id),
                // Membre inconnu : elle n'est simplement pas sur ce serveur.
                Err(DiscordError::Refused { status: StatusCode::NOT_FOUND, code: Some(10007), .. }) => {}
                // Bot absent du serveur, Discord indisponible… : on passe au suivant.
                Err(error) => warn!(%error, guild_id, "vérification d'appartenance impossible"),
            }
        }
        None
    }

    /// Ajoute la personne au serveur du bot ; sans effet si elle y est déjà.
    pub async fn join_guild(&self, user_id: &str, access_token: &str) -> Result<(), DiscordError> {
        let request = self
            .bot(
                self.client.put(format!(
                    "{}/guilds/{}/members/{user_id}",
                    self.config.api_base, self.config.guild_id
                )),
            )
            .header(CONTENT_TYPE, "application/json")
            .body(json!({ "access_token": access_token }).to_string());
        self.send(request).await.map(drop)
    }

    /// Envoie un message privé, avec éventuellement un fichier joint.
    pub async fn send_dm(
        &self,
        user_id: &str,
        content: &str,
        attachment: Option<&Attachment>,
    ) -> Result<(), DiscordError> {
        self.send_dm_with(user_id, content, attachment, Vec::new()).await
    }

    /// Le même message privé, avec des boutons sous le texte.
    ///
    /// Un devoir proposé se confirme d'un geste depuis la notification, sans
    /// repasser par le site : c'est là qu'on la lit, autant pouvoir y répondre.
    pub async fn send_dm_with(
        &self,
        user_id: &str,
        content: &str,
        attachment: Option<&Attachment>,
        buttons: Vec<Button>,
    ) -> Result<(), DiscordError> {
        #[derive(Deserialize)]
        struct Channel {
            id: String,
        }
        let request = self
            .bot(self.client.post(format!("{}/users/@me/channels", self.config.api_base)))
            .header(CONTENT_TYPE, "application/json")
            .body(json!({ "recipient_id": user_id }).to_string());
        let channel: Channel = parse(self.send(request).await?).await?;

        let url = format!("{}/channels/{}/messages", self.config.api_base, channel.id);
        // Pas de mention : un nom de groupe ou de salle ne doit notifier personne.
        let mut payload = json!({ "content": content, "allowed_mentions": { "parse": [] } });
        if !buttons.is_empty() {
            payload["components"] = json!([{ "type": 1, "components": action_row(&buttons) }]);
        }

        let mut attempt = 0;
        loop {
            // Un formulaire multipart ne se clone pas : il est reconstruit à chaque essai.
            let request = match attachment {
                Some(file) => {
                    let part = Part::bytes(file.content.clone())
                        .file_name(file.filename.clone())
                        .mime_str("text/calendar; charset=utf-8")?;
                    let form = Form::new().text("payload_json", payload.to_string()).part("files[0]", part);
                    self.bot(self.client.post(&url)).multipart(form)
                }
                None => self
                    .bot(self.client.post(&url))
                    .header(CONTENT_TYPE, "application/json")
                    .body(payload.to_string()),
            };
            let response = request.send().await?;
            if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < RATE_LIMIT_RETRIES {
                attempt += 1;
                wait_rate_limit(response).await;
                continue;
            }
            return check(response).await.map(drop);
        }
    }

    /// Clé publique de l'application, quand les commandes slash sont montées.
    pub fn public_key(&self) -> Option<&str> {
        self.config.public_key.as_deref()
    }

    /// Déclare les commandes slash du bot sur le serveur configuré.
    ///
    /// Un serveur les reçoit sur-le-champ, là où les commandes globales mettent
    /// jusqu'à une heure à se propager.
    pub async fn register_commands(&self, commands: &serde_json::Value) -> Result<(), DiscordError> {
        let url = format!(
            "{}/applications/{}/guilds/{}/commands",
            self.config.api_base, self.config.client_id, self.config.guild_id
        );
        let request = self
            .bot(self.client.put(url))
            .header(CONTENT_TYPE, "application/json")
            .body(commands.to_string());
        self.send(request).await.map(drop)
    }

    fn bot(&self, request: RequestBuilder) -> RequestBuilder {
        request.header(AUTHORIZATION, format!("Bot {}", self.config.bot_token))
    }

    /// Envoie une requête clonable, en patientant sur les limites de débit.
    async fn send(&self, request: RequestBuilder) -> Result<Response, DiscordError> {
        let mut attempt = 0;
        loop {
            let Some(copy) = request.try_clone() else {
                return check(request.send().await?).await;
            };
            let response = copy.send().await?;
            if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < RATE_LIMIT_RETRIES {
                attempt += 1;
                wait_rate_limit(response).await;
                continue;
            }
            return check(response).await;
        }
    }
}

async fn wait_rate_limit(response: Response) {
    #[derive(Deserialize)]
    struct Limited {
        retry_after: f64,
    }
    let delay = parse::<Limited>(response).await.map_or(1.0, |limited| limited.retry_after);
    warn!(delay, "limite de débit Discord atteinte, nouvelle tentative");
    tokio::time::sleep(Duration::from_secs_f64(delay.clamp(0.0, 60.0))).await;
}

async fn check(response: Response) -> Result<Response, DiscordError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    #[derive(Deserialize)]
    struct Refusal {
        code: Option<u64>,
        message: Option<String>,
        error_description: Option<String>,
    }
    let refusal = parse::<Refusal>(response).await.ok();
    Err(DiscordError::Refused {
        status,
        code: refusal.as_ref().and_then(|r| r.code),
        message: refusal
            .and_then(|r| r.message.or(r.error_description))
            .unwrap_or_else(|| status.canonical_reason().unwrap_or("erreur").to_owned()),
    })
}

async fn parse<T: for<'de> Deserialize<'de>>(response: Response) -> Result<T, DiscordError> {
    let bytes = response.bytes().await?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Encodage d'un composant d'URL (RFC 3986).
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_url_components() {
        assert_eq!(encode("identify guilds.join"), "identify%20guilds.join");
        assert_eq!(encode("https://edt.example/api"), "https%3A%2F%2Fedt.example%2Fapi");
    }
}
