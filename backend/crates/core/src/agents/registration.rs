//! Cadastro de um agente: a linha de `probes` com `role = "agent"`, o nome
//! validado, o vínculo com o dispositivo e o primeiro código de enrollment.
//!
//! Mora no núcleo porque a área VPN também cadastra agentes (o do peer, que
//! já nasce exigindo o túnel) e monta a URL da central vista de dentro dele.
//! O resto do ciclo de vida — enrollment, handshake, remoção — continua no
//! serviço de agentes.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::json;

use super::{
    enrollment,
    identity::{is_agent, ROLE_AGENT},
};
use crate::{
    models::{devices, probes},
    shared::{
        crypto::{random_token, sha256_hex},
        errors::{AppError, AppResult},
    },
};

const STATUS_PENDING: &str = "pending";
const MAX_NAME: usize = 120;

/// Nome do agente sem espaços nas pontas, com 1 a 120 caracteres e sem
/// caractere de controle.
///
/// # Errors
///
/// `validation` quando o nome não cabe na regra.
pub fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(AppError::validation(
            "Nome do agente deve ter entre 1 e 120 caracteres",
        ));
    }
    Ok(name.to_string())
}

/// O dispositivo informado existe?
///
/// # Errors
///
/// `validation` quando não existe; erro do banco.
pub async fn ensure_device(
    db: &DatabaseConnection,
    device_id: Option<i64>,
) -> AppResult<Option<devices::Model>> {
    let Some(device_id) = device_id else {
        return Ok(None);
    };
    devices::Entity::find_by_id(device_id)
        .one(db)
        .await?
        .map(Some)
        .ok_or_else(|| AppError::validation("Dispositivo informado não existe"))
}

/// Cadastra o agente e emite o primeiro código de enrollment.
///
/// O `token_hash` inicial é de um token que ninguém conhece: até o
/// enrollment, nenhuma credencial autentica este probe.
///
/// # Errors
///
/// Validação do nome ou do dispositivo; erro do banco.
pub async fn create(
    db: &DatabaseConnection,
    name: &str,
    site_id: Option<i64>,
    device_id: Option<i64>,
    enforce_tunnel_ip: Option<bool>,
) -> AppResult<(probes::Model, String)> {
    let name = validate_name(name)?;
    ensure_device(db, device_id).await?;
    let saved = probes::ActiveModel {
        site_id: Set(site_id),
        name: Set(name),
        token_hash: Set(sha256_hex(&random_token())),
        status: Set(STATUS_PENDING.into()),
        device_id: Set(device_id),
        configuration: Set(Some(json!({
            "role": ROLE_AGENT,
            "enforceTunnelIp": enforce_tunnel_ip.unwrap_or(true),
        }))),
        ..Default::default()
    }
    .insert(db)
    .await?;
    let code = enrollment::issue(saved.id);
    Ok((saved, code))
}

/// Agente criado junto com o peer da VPN: já nasce vinculado ao
/// dispositivo e exigindo o túnel. O código sai na geração do script.
///
/// # Errors
///
/// Validação do nome ou erro do banco.
pub async fn create_for_device(
    db: &DatabaseConnection,
    name: &str,
    site_id: Option<i64>,
    device_id: i64,
) -> AppResult<probes::Model> {
    if let Some(existing) = for_device(db, device_id).await? {
        return Ok(existing);
    }
    let (probe, _code) = create(db, name, site_id, Some(device_id), Some(true)).await?;
    Ok(probe)
}

/// Agente ativo vinculado ao dispositivo.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn for_device(
    db: &DatabaseConnection,
    device_id: i64,
) -> AppResult<Option<probes::Model>> {
    Ok(probes::Entity::find()
        .filter(probes::Column::DeviceId.eq(device_id))
        .filter(probes::Column::Status.ne(probes::STATUS_REVOKED))
        .order_by_asc(probes::Column::Id)
        .all(db)
        .await?
        .into_iter()
        .find(is_agent))
}

/// URL da central vista de dentro do túnel: `AGENT_SERVER_URL` explícita ou
/// o IP da central na VPN com a porta da API (`APP_PORT`, 3333 por padrão).
#[must_use]
pub fn tunnel_server_url(server_vpn_address: &str) -> String {
    if let Ok(url) = std::env::var("AGENT_SERVER_URL") {
        let url = url.trim().trim_end_matches('/').to_string();
        if !url.is_empty() {
            return url;
        }
    }
    let port = std::env::var("APP_PORT")
        .ok()
        .and_then(|port| port.trim().parse::<u16>().ok())
        .unwrap_or(3333);
    format!("http://{server_vpn_address}:{port}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_vazio_ou_com_controle_e_recusado() {
        assert!(validate_name("  ").is_err());
        assert!(validate_name("srv\u{0}").is_err());
        assert_eq!(validate_name(" srv-01 ").expect("nome"), "srv-01");
    }
}
