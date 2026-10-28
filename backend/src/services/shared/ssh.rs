//! Peças SSH comuns ao SFTP dos backups e ao acesso aos equipamentos.
//!
//! As duas pontas conferem a identidade do servidor do mesmo jeito (TOFU): na
//! primeira conexão a impressão digital é anotada; dali em diante, outra chave
//! derruba a conexão antes da autenticação. E as duas autenticam por chave do
//! mesmo jeito.

use std::sync::{Arc, Mutex};

use russh::{
    client,
    keys::{
        ssh_key::{HashAlg, PublicKey},
        PrivateKeyWithHashAlg,
    },
};

/// Handler do cliente SSH: confere a chave do servidor contra a esperada.
///
/// Sem impressão digital esperada (primeiro uso), aceita e anota a que viu —
/// quem chamou decide gravá-la depois que a conexão inteira deu certo.
pub struct PinnedHostKey {
    pub expected: Option<String>,
    pub observed: Arc<Mutex<Option<String>>>,
}

impl PinnedHostKey {
    #[must_use]
    pub fn new(expected: Option<String>) -> Self {
        Self {
            expected,
            observed: Arc::default(),
        }
    }
}

impl client::Handler for PinnedHostKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = fingerprint_of(server_public_key);
        if let Ok(mut observed) = self.observed.lock() {
            *observed = Some(fingerprint.clone());
        }
        Ok(self
            .expected
            .as_deref()
            .is_none_or(|expected| expected == fingerprint))
    }
}

/// `SHA256:<base64>`, o mesmo formato do `ssh-keygen -lf`.
#[must_use]
pub fn fingerprint_of(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// A impressão digital anotada pelo handler, se a negociação chegou a vê-la.
#[must_use]
pub fn observed(cell: &Mutex<Option<String>>) -> Option<String> {
    cell.lock().ok().and_then(|value| value.clone())
}

/// Autentica por chave privada (OpenSSH ou PEM, com passphrase opcional).
///
/// `Ok(false)` é chave recusada pelo servidor; `Err` é chave ilegível ou falha
/// de transporte, já em texto para a mensagem do chamador.
///
/// # Errors
///
/// Chave inválida, passphrase errada ou falha na conversa SSH.
pub async fn authenticate_key<H: client::Handler>(
    session: &mut client::Handle<H>,
    user: &str,
    key: &str,
    passphrase: Option<&str>,
) -> Result<bool, String> {
    let decoded = russh::keys::decode_secret_key(key, passphrase)
        .map_err(|err| format!("Chave privada inválida ou passphrase errada: {err}"))?;
    let hash = session
        .best_supported_rsa_hash()
        .await
        .ok()
        .flatten()
        .flatten();
    session
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(decoded), hash))
        .await
        .map(|result| result.success())
        .map_err(|err| format!("Falha na autenticação: {err}"))
}
