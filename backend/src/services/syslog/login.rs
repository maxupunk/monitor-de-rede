//! O login da ativação automática: digitado na hora ou o salvo no equipamento.
//!
//! A credencial do equipamento mora num lugar só — a aba "Plugins ›
//! Credenciais" ([`crate::services::plugins::credentials`]). A ativação do
//! syslog não tem um cofre próprio: ou usa a credencial salva ali, ou usa a
//! digitada, que só é lembrada lá (usuário no cadastro, senha 30 minutos em
//! memória) quando o operador pede **e** ela funcionou. Lembrar uma senha que
//! o equipamento recusou só adiaria o erro para o próximo plugin.

use sea_orm::ConnectionTrait;

use super::provision::Protocol;
use crate::services::{
    plugins::{
        credentials::{self, CredentialView},
        manifest::TransportKind,
        transport::Login,
    },
    shared::errors::{AppError, AppResult},
};

/// De onde vem o login.
pub enum LoginChoice<'a> {
    /// Usuário e senha digitados nesta ativação.
    Typed {
        username: &'a str,
        password: &'a str,
    },
    /// A credencial salva no equipamento, daquele tipo.
    Saved(TransportKind),
}

/// O login pronto para a sessão.
///
/// # Errors
///
/// Usuário ou senha vazios, tipo que não abre shell (HTTP), ou credencial salva
/// indisponível — ver [`credentials::stored_login`].
pub async fn resolve<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    choice: LoginChoice<'_>,
) -> AppResult<Login> {
    let login = match choice {
        LoginChoice::Typed { username, password } => {
            if password.is_empty() {
                return Err(AppError::validation("Informe a senha de acesso."));
            }
            Login::with_password(username.trim().to_owned(), Some(password.to_owned()))
        }
        LoginChoice::Saved(TransportKind::Http) => {
            return Err(AppError::validation(
                "A credencial HTTP não serve para ativar o log: use a de SSH ou Telnet.",
            ))
        }
        LoginChoice::Saved(kind) => credentials::stored_login(db, device_id, kind).await?,
    };
    if login.username.is_empty() {
        return Err(AppError::validation("Informe o usuário de acesso."));
    }
    Ok(login)
}

/// Lembra o login que acabou de funcionar como a credencial do equipamento.
///
/// A porta só é gravada quando é de fato a do tipo: a do MAC-Telnet (20561) não
/// é a porta Telnet do equipamento, e gravá-la quebraria os plugins por Telnet.
///
/// # Errors
///
/// Os de [`credentials::remember_login`].
pub async fn remember<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    protocol: Protocol,
    login: &Login,
    port: u16,
) -> AppResult<CredentialView> {
    let port = (!protocol.by_mac()).then_some(port);
    credentials::remember_login(db, device_id, protocol.credential_kind(), login, port).await
}
