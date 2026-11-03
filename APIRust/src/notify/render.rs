//! L'image d'une semaine, pour l'afficher dans un salon sans ouvrir le PDF.
//!
//! Le rendu est confié à `pdftoppm` (poppler-utils) : c'est l'outil le plus
//! fidèle au PDF du département, et il tient dans l'image Docker. Absent ou en
//! échec, l'annonce part sans image — le PDF, lui, est toujours joint.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::warn;

/// Assez net pour lire les salles sur un téléphone, assez léger pour Discord.
const DPI: &str = "130";
/// Un PDF d'une page se rend en une fraction de seconde ; au-delà, c'est bloqué.
const TIMEOUT: Duration = Duration::from_secs(20);

/// La première page du PDF en PNG, ou rien si le rendu est impossible.
pub async fn png(pdf: &[u8]) -> Option<Vec<u8>> {
    match tokio::time::timeout(TIMEOUT, run(pdf)).await {
        Ok(Ok(image)) if image.starts_with(b"\x89PNG") => Some(image),
        Ok(Ok(_)) => {
            warn!("pdftoppm n'a pas produit d'image");
            None
        }
        Ok(Err(error)) => {
            warn!(%error, "rendu du PDF impossible (pdftoppm est-il installé ?)");
            None
        }
        Err(_) => {
            warn!("rendu du PDF trop long, abandonné");
            None
        }
    }
}

async fn run(pdf: &[u8]) -> std::io::Result<Vec<u8>> {
    // `-` : le PDF arrive sur l'entrée standard, l'image sort sur la sortie.
    let mut child = Command::new("pdftoppm")
        .args(["-png", "-r", DPI, "-singlefile", "-f", "1", "-l", "1", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child.stdin.take().expect("entrée standard demandée");
    let input = pdf.to_vec();
    // L'écriture part de son côté : un gros PDF ne doit pas bloquer la lecture.
    let writer = tokio::spawn(async move {
        let _ = stdin.write_all(&input).await;
    });
    let output = child.wait_with_output().await?;
    let _ = writer.await;
    if !output.status.success() {
        return Err(std::io::Error::other(format!("pdftoppm a échoué ({})", output.status)));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn renders_a_week_or_gives_up_quietly() {
        let pdf =
            std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/A3_S2.pdf")).expect("PDF");
        // Sans poppler sur la machine de test, le rendu est simplement absent.
        if let Some(image) = png(&pdf).await {
            assert!(image.len() > 1000);
        }
        assert_eq!(png(b"pas un pdf").await, None);
    }
}
