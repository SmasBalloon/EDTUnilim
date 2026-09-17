//! Jetons signés portés par les cookies : session et état OAuth.
//!
//! Un jeton s'écrit `valeur.expiration.signature`, la signature étant un
//! HMAC-SHA256 des deux premières parties. Rien n'est conservé côté serveur :
//! la signature suffit à prouver que le jeton a été émis ici.

use axum::http::header::COOKIE;
use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use time::OffsetDateTime;

type HmacSha256 = Hmac<Sha256>;

/// Cookie de session : l'identifiant Discord de la personne connectée.
pub const SESSION_COOKIE: &str = "edt_session";
/// Cookie de l'aller-retour OAuth : le nonce attendu au retour.
pub const OAUTH_COOKIE: &str = "edt_oauth";

/// Durée de validité d'une session.
pub const SESSION_TTL: i64 = 30 * 24 * 3600;
/// Durée accordée pour revenir de la page d'autorisation Discord.
pub const OAUTH_TTL: i64 = 10 * 60;

#[derive(Clone)]
pub struct Signer {
    key: Vec<u8>,
}

impl Signer {
    pub fn new(secret: &str) -> Self {
        Self { key: secret.as_bytes().to_vec() }
    }

    fn mac(&self, payload: &str) -> HmacSha256 {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC accepte toute longueur de clé");
        mac.update(payload.as_bytes());
        mac
    }

    /// Jeton valable `ttl` secondes pour `value`, qui ne doit pas contenir de point.
    pub fn sign(&self, value: &str, ttl: i64) -> String {
        let expires = OffsetDateTime::now_utc().unix_timestamp() + ttl;
        let payload = format!("{value}.{expires}");
        let signature = hex(&self.mac(&payload).finalize().into_bytes());
        format!("{payload}.{signature}")
    }

    /// La valeur d'un jeton intact et non expiré.
    pub fn verify(&self, token: &str) -> Option<String> {
        let (payload, signature) = token.rsplit_once('.')?;
        let (value, expires) = payload.rsplit_once('.')?;
        let signature = unhex(signature)?;
        // `verify_slice` compare en temps constant.
        self.mac(payload).verify_slice(&signature).ok()?;
        let expires: i64 = expires.parse().ok()?;
        (expires > OffsetDateTime::now_utc().unix_timestamp()).then(|| value.to_owned())
    }
}

/// Un nonce aléatoire de 128 bits, en hexadécimal.
pub fn nonce() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("source d'aléa du système disponible");
    hex(&bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

/// Valeur d'un cookie de la requête.
pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_owned())
}

/// En-tête `Set-Cookie` d'un cookie protégé ; `max_age` à 0 l'efface.
pub fn cookie(name: &str, value: &str, max_age: i64, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}")
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn accepts_its_own_tokens() {
        let signer = Signer::new("secret");
        let token = signer.sign("123456", 60);
        assert_eq!(signer.verify(&token).as_deref(), Some("123456"));
    }

    #[test]
    fn rejects_forged_expired_or_foreign_tokens() {
        let signer = Signer::new("secret");
        let token = signer.sign("123456", 60);

        let forged = token.replacen("123456", "999999", 1);
        assert_eq!(signer.verify(&forged), None);
        assert_eq!(Signer::new("autre").verify(&token), None);
        assert_eq!(signer.verify(&signer.sign("123456", -1)), None);
        assert_eq!(signer.verify("n'importe quoi"), None);
    }

    #[test]
    fn reads_a_cookie_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("a=1; edt_session=abc.1.ff; b=2"));
        assert_eq!(read_cookie(&headers, SESSION_COOKIE).as_deref(), Some("abc.1.ff"));
        assert_eq!(read_cookie(&headers, OAUTH_COOKIE), None);
    }

    #[test]
    fn nonces_differ() {
        assert_ne!(nonce(), nonce());
        assert_eq!(nonce().len(), 32);
    }
}
