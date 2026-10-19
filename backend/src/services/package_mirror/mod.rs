//! **Espelho de pacotes do OpenWrt pela central** (ADR 015 §10).
//!
//! Quando o roteador não consegue baixar do repositório oficial — DNS do
//! próprio roteador quebrado, relógio atrasado invalidando o HTTPS, sem saída
//! para a internet —, o `apk`/`opkg` dele busca os mesmos arquivos pela
//! central: `GET /api/package-mirror/openwrt/{token}/{caminho}` na porta da
//! API, que repassa de `downloads.openwrt.org`.
//!
//! **Não é um proxy aberto.** Só caminhos de `releases/` e `snapshots/` do
//! repositório oficial ([`policy`]), só com um token emitido para uma execução
//! de plugin e válido por poucos minutos ([`grants`]). O que passa por aqui é
//! público, e o gerenciador do roteador continua conferindo a assinatura do
//! OpenWrt no índice e em cada pacote: a central não pode trocar um pacote sem
//! o roteador recusar.

pub mod fetch;
pub mod grants;
pub mod policy;

pub use fetch::{fetch, Upstream};
pub use grants::{issue, MirrorAccess};

use crate::services::shared::errors::{AppError, AppResult};

/// Confere o token e o caminho e devolve a URL de origem.
///
/// # Errors
///
/// `forbidden` para token desconhecido ou vencido; `not_found` para caminho
/// fora da política (o espelho não revela por que um caminho não existe).
pub fn authorize(token: &str, path: &str) -> AppResult<String> {
    if !grants::is_valid(token) {
        return Err(AppError::forbidden(
            "Autorização do espelho de pacotes inválida ou vencida.",
        ));
    }
    policy::upstream_url(path)
        .ok_or_else(|| AppError::not_found("Arquivo fora do espelho de pacotes."))
}
