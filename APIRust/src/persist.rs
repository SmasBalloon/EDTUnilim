//! Écriture des fichiers d'état, à l'épreuve d'un arrêt brutal.
//!
//! Le contenu est écrit dans un fichier temporaire, forcé sur le disque, puis
//! renommé par-dessus l'ancien : un lecteur voit l'ancienne version ou la
//! nouvelle, jamais un fichier à moitié écrit ou vide après une coupure.

use std::io;
use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

/// Remplace `path` par `bytes` d'un seul coup.
pub async fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let temporary = sibling(path, "tmp");
    let mut file = tokio::fs::File::create(&temporary).await?;
    file.write_all(bytes).await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(&temporary, path).await?;
    sync_parent(path).await
}

/// Comme [`write_atomic`], en gardant la version remplacée à côté
/// (`users.json.bak`) : une erreur de manipulation se rattrape d'une version.
pub async fn write_with_backup(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match tokio::fs::copy(path, sibling(path, "bak")).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    write_atomic(path, bytes).await
}

/// `users.json` → `users.json.<suffix>`
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

/// Le renommage lui-même n'est durable qu'une fois le répertoire synchronisé.
#[cfg(unix)]
async fn sync_parent(path: &Path) -> io::Result<()> {
    match path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        Some(parent) => tokio::fs::File::open(parent).await?.sync_all().await,
        None => Ok(()),
    }
}

#[cfg(not(unix))]
async fn sync_parent(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replaces_the_file_and_leaves_no_temporary_behind() {
        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let path = dir.path().join("sous/dossier/users.json");
        write_atomic(&path, b"un").await.expect("première écriture");
        write_atomic(&path, b"deux").await.expect("seconde écriture");
        assert_eq!(tokio::fs::read(&path).await.expect("relu"), b"deux");
        assert!(!sibling(&path, "tmp").exists());
    }

    #[tokio::test]
    async fn keeps_the_previous_version_as_a_backup() {
        let dir = tempfile::tempdir().expect("répertoire temporaire");
        let path = dir.path().join("users.json");
        write_with_backup(&path, b"un").await.expect("sans version précédente");
        assert!(!sibling(&path, "bak").exists());
        write_with_backup(&path, b"deux").await.expect("avec version précédente");
        assert_eq!(tokio::fs::read(&path).await.expect("relu"), b"deux");
        assert_eq!(tokio::fs::read(sibling(&path, "bak")).await.expect("sauvegarde"), b"un");
    }

    #[test]
    fn names_the_sibling_after_the_whole_file_name() {
        assert_eq!(sibling(Path::new("state/users.json"), "bak"), Path::new("state/users.json.bak"));
        assert_eq!(sibling(Path::new("A1/S3.pdf"), "tmp"), Path::new("A1/S3.pdf.tmp"));
    }
}
