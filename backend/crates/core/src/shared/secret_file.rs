//! Um segredo de uma linha num arquivo: o token do agente.
//!
//! O agente remoto guarda ali o token que o enrollment devolveu; a central
//! grava ali o do agente local, num volume que só os dois montam (ADR 014).

use std::path::Path;

/// Lê o segredo, sem espaços nas pontas. Arquivo ausente ou vazio é `None`.
#[must_use]
pub fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Grava com permissão 0600 e troca atômica: quem lê nunca vê meio token.
///
/// # Errors
///
/// Falha de escrita.
pub fn write(path: &Path, secret: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, secret)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(temporary, path)
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
