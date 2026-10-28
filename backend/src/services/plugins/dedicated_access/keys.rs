//! O par de chaves do acesso dedicado: ed25519, um por equipamento.
//!
//! Só ed25519. É o que o Dropbear do OpenWrt (≥ 21.02) e o RouterOS (≥ 7.12)
//! aceitam como chave de usuário sem configuração, e gerá-lo não pede mais que
//! 32 bytes do gerador do sistema.

use rand::{rngs::OsRng, Rng, RngCore};
use russh::keys::{
    ssh_key::{private::Ed25519Keypair, LineEnding},
    PrivateKey,
};

use crate::services::shared::{
    errors::{AppError, AppResult},
    ssh::fingerprint_of,
};

/// Prefixo do comentário da chave. É a marca que localiza a linha no
/// `authorized_keys` para retirá-la depois.
pub const COMMENT_PREFIX: &str = "netmonitor-";

/// Uma chave recém-gerada.
pub struct GeneratedKey {
    /// OpenSSH, para o cofre. Nunca sai do servidor.
    pub private_openssh: String,
    /// `ssh-ed25519 AAAA… netmonitor-<id>`, a linha que vai ao equipamento.
    pub public_openssh: String,
    /// `SHA256:…`, o que a tela mostra.
    pub fingerprint: String,
    /// `netmonitor-<12 hex>` — só hexadecimal, seguro em `grep` e `sed`.
    pub comment: String,
}

fn infra(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(anyhow::anyhow!("chave do acesso dedicado: {error}"))
}

/// # Errors
///
/// Falha ao codificar a chave (não deveria acontecer).
pub fn generate() -> AppResult<GeneratedKey> {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let comment = format!("{COMMENT_PREFIX}{:012x}", OsRng.gen::<u64>() >> 16);
    let mut private = PrivateKey::from(Ed25519Keypair::from_seed(&seed));
    private.set_comment(comment.as_str());
    let private_openssh = private
        .to_openssh(LineEnding::LF)
        .map_err(infra)?
        .to_string();
    let public_openssh = private.public_key().to_openssh().map_err(infra)?;
    Ok(GeneratedKey {
        private_openssh,
        public_openssh,
        fingerprint: fingerprint_of(private.public_key()),
        comment,
    })
}

/// Senha aleatória para o usuário criado no RouterOS. Ninguém a usa — o login
/// é pela chave, e com chave o RouterOS recusa senha no SSH —, mas usuário sem
/// senha seria um convite.
#[must_use]
pub fn throwaway_password() -> String {
    OsRng
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gera_ed25519_com_comentario_marcado_e_chave_legivel() {
        let key = generate().unwrap();
        assert!(key.public_openssh.starts_with("ssh-ed25519 AAAA"));
        assert!(key.public_openssh.ends_with(&key.comment));
        assert!(key.comment.starts_with(COMMENT_PREFIX));
        assert!(key.comment[COMMENT_PREFIX.len()..]
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
        assert!(key.fingerprint.starts_with("SHA256:"));
        let decoded = russh::keys::decode_secret_key(&key.private_openssh, None).unwrap();
        assert_eq!(fingerprint_of(decoded.public_key()), key.fingerprint);
        assert_ne!(
            generate().unwrap().fingerprint,
            key.fingerprint,
            "uma por vez"
        );
    }

    #[test]
    fn senha_descartavel_e_longa_e_alfanumerica() {
        let password = throwaway_password();
        assert_eq!(password.len(), 32);
        assert!(password.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
