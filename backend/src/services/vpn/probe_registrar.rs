//! Registro idempotente do `vpn-probe` (§8.10.4).
//!
//! É o agente que compartilha o namespace de rede do container WireGuard e
//! executa ICMP/SNMP **dentro** do túnel. O token vem de `VPN_PROBE_TOKEN` (o
//! mesmo valor usado pelo container), de modo que a inicialização do servidor já
//! deixa o probe pronto para o heartbeat.

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};

use crate::{
    models::probes,
    services::{
        probes::DEFAULT_VPN_PROBE_TOKEN,
        shared::{crypto::sha256_hex, errors::AppResult},
        vpn::monitor_provisioner::vpn_probe_name,
    },
};

pub struct VpnProbeRegistration {
    pub probe: probes::Model,
    pub created: bool,
    /// Preenchido apenas quando o token foi gerado aqui.
    pub token: Option<String>,
}

/// Cria ou atualiza o probe dedicado.
///
/// Precedência do token: o informado, `VPN_PROBE_TOKEN` do ambiente e, por fim,
/// o [`DEFAULT_VPN_PROBE_TOKEN`]. ⚠️ **O último degrau nunca pode ser removido**
/// — é ele que permite o container `vpn-probe` subir sem configuração alguma
/// (matriz de paridade #43).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn register<C: ConnectionTrait>(
    db: &C,
    raw_token: Option<&str>,
) -> AppResult<VpnProbeRegistration> {
    let token = raw_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            std::env::var("VPN_PROBE_TOKEN")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| DEFAULT_VPN_PROBE_TOKEN.to_string());
    let token_hash = sha256_hex(&token);
    let name = vpn_probe_name();

    if let Some(existing) = probes::Entity::find()
        .filter(probes::Column::Name.eq(name.clone()))
        .one(db)
        .await?
    {
        let was_revoked = existing.status == probes::STATUS_REVOKED;
        let mut active: probes::ActiveModel = existing.into();
        active.token_hash = Set(token_hash);
        // Um `vpn-probe` revogado à mão volta a `pending` quando o servidor o
        // registra de novo: sem isso, o container subiria e nunca autenticaria.
        if was_revoked {
            active.status = Set("pending".into());
        }
        return Ok(VpnProbeRegistration {
            probe: active.update(db).await?,
            created: false,
            token: None,
        });
    }

    let probe = probes::ActiveModel {
        name: Set(name),
        token_hash: Set(token_hash),
        status: Set("pending".into()),
        registered_at: Set(Some(Utc::now().into())),
        configuration: Set(Some(
            serde_json::json!({ "role": "vpn", "network": "wireguard" }),
        )),
        ..Default::default()
    }
    .insert(db)
    .await?;

    Ok(VpnProbeRegistration {
        probe,
        created: true,
        token: None,
    })
}

/// Versão para CLI: gera o token quando não houver um configurado no ambiente.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn register_with_generated_token<C: ConnectionTrait>(
    db: &C,
) -> AppResult<VpnProbeRegistration> {
    let env_token = std::env::var("VPN_PROBE_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let generated = env_token
        .is_none()
        .then(crate::tasks::probe_register::generate_token);
    let token = env_token
        .clone()
        .or_else(|| generated.clone())
        .unwrap_or_else(|| DEFAULT_VPN_PROBE_TOKEN.to_string());

    let mut registration = register(db, Some(&token)).await?;
    // Só devolve o token quando ele foi gerado aqui: o do ambiente o operador
    // já tem, e imprimi-lo no log da CLI seria vazamento sem ganho.
    registration.token = generated;
    Ok(registration)
}
