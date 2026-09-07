//! Accès au serveur du département, qui publie un PDF par année et par semaine.

use std::time::Duration;

use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use reqwest::StatusCode;

/// Ce qui permet de savoir si un PDF a changé sans le retélécharger.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Validators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug)]
pub struct Document {
    pub bytes: Vec<u8>,
    pub validators: Validators,
}

#[derive(Debug)]
pub enum Fetch {
    Fresh(Document),
    /// Le serveur confirme que la version en cache est toujours d'actualité.
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct Upstream {
    client: reqwest::Client,
    base_url: String,
}

impl Upstream {
    pub fn new(base_url: String, timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { client, base_url })
    }

    /// Numéros de semaine publiés pour une année, par ordre croissant.
    pub async fn list_weeks(&self, year: u8) -> Result<Vec<u8>, reqwest::Error> {
        let index = self
            .client
            .get(format!("{}/A{year}/", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let mut weeks: Vec<u8> = parse_week_links(&index, year);
        weeks.sort_unstable();
        weeks.dedup();
        Ok(weeks)
    }

    /// Télécharge l'emploi du temps d'une semaine, en évitant le transfert si
    /// les validateurs fournis sont encore valables.
    pub async fn fetch_week(&self, year: u8, week: u8, known: &Validators) -> Result<Fetch, reqwest::Error> {
        let mut request = self.client.get(week_url(&self.base_url, year, week));
        if let Some(etag) = &known.etag {
            request = request.header(IF_NONE_MATCH, etag);
        }
        if let Some(date) = &known.last_modified {
            request = request.header(IF_MODIFIED_SINCE, date);
        }

        let response = request.send().await?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(Fetch::Unchanged);
        }

        let response = response.error_for_status()?;
        let header = |name: reqwest::header::HeaderName| {
            response.headers().get(&name).and_then(|v| v.to_str().ok()).map(str::to_owned)
        };
        let validators = Validators { etag: header(ETAG), last_modified: header(LAST_MODIFIED) };

        Ok(Fetch::Fresh(Document { bytes: response.bytes().await?.to_vec(), validators }))
    }
}

pub fn week_url(base_url: &str, year: u8, week: u8) -> String {
    format!("{base_url}/A{year}/{}", week_file_name(year, week))
}

pub fn week_file_name(year: u8, week: u8) -> String {
    format!("A{year}_S{week}.pdf")
}

/// Relève les `A1_S3.pdf` dans l'index Apache du répertoire.
fn parse_week_links(html: &str, year: u8) -> Vec<u8> {
    let prefix = format!("A{year}_S");
    html.split(&prefix)
        .skip(1)
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            rest[digits.len()..].starts_with(".pdf").then(|| digits.parse().ok())?
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_week_numbers_from_index() {
        let html = r#"<a href="A1_S1.pdf">A1_S1.pdf</a> <a href="G1/">G1/</a>
                      <a href="A1_S12.pdf">A1_S12.pdf</a> <a href="A2_S4.pdf">nope</a>"#;
        let mut weeks = parse_week_links(html, 1);
        weeks.sort_unstable();
        weeks.dedup();
        assert_eq!(weeks, vec![1, 12]);
    }

    #[test]
    fn ignores_unrelated_links() {
        assert!(parse_week_links(r#"<a href="A1_Sx.pdf">x</a>"#, 1).is_empty());
    }
}
