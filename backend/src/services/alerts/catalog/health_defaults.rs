//! Aplicação, uma única vez, das regras de saúde ao dispositivo do sistema.
//!
//! # Por que não `ensure_defaults`
//!
//! `ensure_defaults` responde a pergunta "esta instalação é nova?" — e a
//! responde pelo critério "não existe regra alguma". É a pergunta certa para o
//! conjunto básico global, e a errada aqui: uma instalação que já opera há
//! meses ganha o Servidor NetMonitor no upgrade e precisa das regras de saúde
//! dele, mesmo tendo cinquenta regras cadastradas.
//!
//! # Por que um marcador em `system_settings`
//!
//! A pergunta que precisamos responder é "**já** apliquei as regras de saúde
//! deste dispositivo alguma vez?", e ela não pode ser respondida olhando as
//! regras existentes: se o operador apagar de propósito a regra de CPU do
//! servidor, ela não pode ressuscitar no próximo boot. Um marcador é a única
//! forma honesta de distinguir "nunca apliquei" de "apliquei e o usuário
//! removeu".
//!
//! É o mesmo mecanismo que `server_addresses` já usa, e não custa coluna nem
//! tabela.

use sea_orm::ConnectionTrait;

use crate::{
    models::system_settings::Model as SystemSetting,
    services::{
        alerts::catalog::{
            service::{apply_scoped, CatalogApplicationResult, TemplateScope},
            templates,
        },
        shared::errors::AppResult,
    },
};

/// Prefixo da chave. O ID do dispositivo entra no sufixo para que o mecanismo
/// sirva a qualquer dispositivo, e não só ao servidor.
const MARKER_PREFIX: &str = "alerts.health_defaults_applied.device";

fn marker_key(device_id: i64) -> String {
    format!("{MARKER_PREFIX}.{device_id}")
}

/// Aplica os templates de saúde ao dispositivo, uma vez na vida da instalação.
///
/// Idempotente por duas vias independentes, e as duas são necessárias: o
/// marcador impede que uma regra removida pelo usuário reapareça, e a
/// idempotência com escopo do catálogo impede duplicata caso o marcador se
/// perca numa restauração de backup.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn ensure_for_device<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
) -> AppResult<CatalogApplicationResult> {
    let key = marker_key(device_id);
    if SystemSetting::get(db, &key).await?.is_some() {
        return Ok(CatalogApplicationResult::default());
    }

    let keys: Vec<String> = templates::HEALTH_KEYS
        .iter()
        .map(|key| (*key).to_string())
        .collect();
    let result = apply_scoped(db, &keys, TemplateScope::device(device_id)).await?;

    // O marcador é gravado depois da aplicação: se a transação de criação
    // falhar, o próximo boot tenta de novo em vez de desistir para sempre.
    SystemSetting::set(db, &key, Some("1".to_string())).await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chave_do_marcador_e_por_dispositivo() {
        assert_ne!(marker_key(1), marker_key(2));
        assert!(marker_key(7).ends_with(".7"));
    }
}
