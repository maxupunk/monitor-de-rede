//! Cadastro de um agente: a linha de `probes` com `role = "agent"`, o nome
//! validado, o vínculo com o dispositivo e o primeiro código de enrollment.
//!
//! Mora no núcleo porque a área VPN também cadastra agentes (o do peer, que
//! já nasce exigindo o túnel) e monta a URL da central vista de dentro dele.
//! O resto do ciclo de vida — enrollment, handshake, remoção — continua no
//! serviço de agentes.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionSession, TransactionTrait,
};
use serde_json::json;

use super::{
    enrollment,
    identity::{is_agent, is_local, ROLE_AGENT},
    install_policy::InstallPolicy,
};
use crate::{
    models::{devices, probes},
    shared::{
        crypto::{random_token, sha256_hex},
        errors::{AppError, AppResult},
        transaction::begin_write,
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
pub async fn ensure_device<C: ConnectionTrait>(
    db: &C,
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
pub async fn create<C: ConnectionTrait>(
    db: &C,
    name: &str,
    site_id: Option<i64>,
    device_id: Option<i64>,
    enforce_tunnel_ip: Option<bool>,
) -> AppResult<(probes::Model, String)> {
    let saved = insert_agent(db, name, site_id, device_id, enforce_tunnel_ip, None).await?;
    let code = enrollment::issue(saved.id);
    Ok((saved, code))
}

async fn insert_agent<C: ConnectionTrait>(
    db: &C,
    name: &str,
    site_id: Option<i64>,
    device_id: Option<i64>,
    enforce_tunnel_ip: Option<bool>,
    policy: Option<&InstallPolicy>,
) -> AppResult<probes::Model> {
    let name = validate_name(name)?;
    ensure_device(db, device_id).await?;
    let mut configuration = json!({
        "role": ROLE_AGENT,
        "enforceTunnelIp": enforce_tunnel_ip.unwrap_or(true),
    });
    if let Some(policy) = policy {
        policy.store(&mut configuration);
    }
    Ok(probes::ActiveModel {
        site_id: Set(site_id),
        name: Set(name),
        token_hash: Set(sha256_hex(&random_token())),
        status: Set(STATUS_PENDING.into()),
        device_id: Set(device_id),
        configuration: Set(Some(configuration)),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Agente criado junto com o peer da VPN: já nasce vinculado ao
/// dispositivo e exigindo o túnel. O código sai na geração do script, com a
/// política escolhida (gravada para o script regerado sair igual).
///
/// # Errors
///
/// Validação do nome ou erro do banco.
pub async fn create_for_device<C: ConnectionTrait + TransactionTrait>(
    db: &C,
    name: &str,
    site_id: Option<i64>,
    device_id: i64,
    policy: &InstallPolicy,
) -> AppResult<probes::Model> {
    let txn = begin_write(db).await?;
    if let Some(existing) = for_device(&txn, device_id).await? {
        txn.commit().await?;
        return Ok(existing);
    }
    let saved = insert_agent(
        &txn,
        name,
        site_id,
        Some(device_id),
        Some(true),
        Some(policy),
    )
    .await?;
    txn.commit().await?;
    Ok(saved)
}

/// Agente já cadastrado que pode passar a falar pelo túnel de um peer novo:
/// o servidor que se conectou por outro caminho (domínio, rede local) e agora
/// entra na VPN sem perder o cadastro nem o histórico.
///
/// # Errors
///
/// `not_found` para um id que não é de agente; `business_rule` para o agente
/// local, um revogado ou um que já tem dispositivo.
pub async fn attachable<C: ConnectionTrait>(db: &C, agent_id: i64) -> AppResult<probes::Model> {
    let agent = probes::Entity::find_by_id(agent_id)
        .one(db)
        .await?
        .filter(is_agent)
        .ok_or_else(|| AppError::not_found("Servidor remoto não encontrado"))?;
    if is_local(&agent) {
        return Err(AppError::business_rule(
            "O agente local fica ao lado da central e não usa a VPN",
        ));
    }
    if agent.status == probes::STATUS_REVOKED {
        return Err(AppError::business_rule(
            "Este servidor foi revogado: cadastre-o de novo para conectá-lo",
        ));
    }
    if agent.device_id.is_some() {
        return Err(AppError::business_rule(
            "Este servidor já está vinculado a um dispositivo",
        ));
    }
    Ok(agent)
}

/// Vincula o agente ao dispositivo do peer e passa a exigir o túnel: daqui em
/// diante a credencial dele só vale vinda do IP daquele peer.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn attach_to_device<C: ConnectionTrait>(
    db: &C,
    mut agent: probes::Model,
    device_id: i64,
    policy: &InstallPolicy,
) -> AppResult<probes::Model> {
    let mut configuration = agent.configuration.take().unwrap_or_else(|| json!({}));
    configuration["enforceTunnelIp"] = serde_json::Value::Bool(true);
    policy.store(&mut configuration);
    let mut active: probes::ActiveModel = agent.into();
    active.device_id = Set(Some(device_id));
    active.configuration = Set(Some(configuration));
    Ok(active.update(db).await?)
}

/// Atualiza a política de instalação gravada no agente, para que rotações de
/// chave e novos códigos saiam com a última escolha do operador.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn save_policy<C: ConnectionTrait>(
    db: &C,
    probe_id: i64,
    policy: &InstallPolicy,
) -> AppResult<Option<probes::Model>> {
    let Some(mut agent) = probes::Entity::find_by_id(probe_id)
        .one(db)
        .await?
        .filter(is_agent)
    else {
        return Ok(None);
    };
    let mut configuration = agent.configuration.take().unwrap_or_else(|| json!({}));
    policy.store(&mut configuration);
    let mut active: probes::ActiveModel = agent.into();
    active.configuration = Set(Some(configuration));
    Ok(Some(active.update(db).await?))
}

/// Agente ativo vinculado ao dispositivo.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn for_device<C: ConnectionTrait>(
    db: &C,
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
    let host = server_vpn_address.trim();
    if host.contains(':') && !host.starts_with('[') {
        format!("http://[{host}]:{port}")
    } else {
        format!("http://{host}:{port}")
    }
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
