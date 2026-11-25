//! Um segredo de uma linha num arquivo: o token do agente.
//!
//! O agente remoto guarda ali o token que o enrollment devolveu; a central
//! grava ali o do agente local, num volume que só os dois montam (ADR 014).

use std::{io::Write, path::Path};

/// Lê o segredo, sem espaços nas pontas. Arquivo ausente ou vazio é `None`.
#[must_use]
pub fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Grava com permissão 0600 desde a criação e troca atômica: quem lê nunca vê
/// meio token nem um arquivo temporário com permissão ampla.
///
/// # Errors
///
/// Falha de escrita.
pub fn write(path: &Path, secret: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("secret");
    let temporary = path.with_file_name(format!(
        ".{file_name}.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let write_result = options
        .open(&temporary)
        .and_then(|mut file| file.write_all(secret.as_bytes()));
    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&temporary);
        return Err(err);
    }

    if let Err(err) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segredo_e_gravado_e_lido_de_volta() {
        let dir = std::env::temp_dir().join(format!("nm-secret-{}", uuid::Uuid::new_v4()));
        let path = dir.join("token");
        assert_eq!(read(&path), None);
        write(&path, "segredo\n").expect("grava");
        assert_eq!(read(&path).as_deref(), Some("segredo"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
