//! O `.env` da stack, gravado de forma atômica.
//!
//! Escreve num arquivo temporário do mesmo diretório e renomeia: quem lê
//! (o compose de um deploy anterior ainda rodando) nunca vê metade do arquivo.
//! No Unix o arquivo nasce `0600` — o `.env` carrega as senhas da aplicação.

use std::path::Path;

use tokio::io::AsyncWriteExt;

use super::super::DockerError;

pub const FILE_NAME: &str = ".env";

/// Grava `content` em `dir/.env`.
///
/// # Errors
///
/// `Validation` com o motivo quando o disco recusa.
pub async fn write(dir: &Path, content: &str) -> Result<(), DockerError> {
    let target = dir.join(FILE_NAME);
    let temporary = dir.join(format!(".env.netmonitor-{}", uuid::Uuid::new_v4()));
    let failed = |error: std::io::Error| {
        DockerError::Validation(format!("Não foi possível gravar o .env: {error}"))
    };
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let result = async {
        let mut file = options.open(&temporary).await?;
        file.write_all(content.as_bytes()).await?;
        if !content.ends_with('\n') && !content.is_empty() {
            file.write_all(b"\n").await?;
        }
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temporary, &target).await
    }
    .await;
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(failed(error));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn grava_e_substitui_sem_sobras() {
        let dir = tempfile::tempdir().expect("tmp");
        write(dir.path(), "A=1").await.expect("primeira");
        write(dir.path(), "A=2\nB=3\n").await.expect("segunda");
        let content = tokio::fs::read_to_string(dir.path().join(FILE_NAME))
            .await
            .expect("lido");
        assert_eq!(content, "A=2\nB=3\n");
        let mut entries = tokio::fs::read_dir(dir.path()).await.expect("dir");
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await.expect("entrada") {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
        assert_eq!(names, vec![FILE_NAME.to_string()], "temporário esquecido");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn nasce_so_para_o_dono() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tmp");
        write(dir.path(), "SENHA=x").await.expect("gravado");
        let mode = std::fs::metadata(dir.path().join(FILE_NAME))
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
