//! Orquestração do diagnóstico rápido: resolve o alvo, trava um por endereço,
//! coleta e estima. Sem HTTP — o controller só extrai e serializa.

use std::net::IpAddr;

use sea_orm::{ConnectionTrait, EntityTrait};
use tokio_util::sync::CancellationToken;

use super::{
    estimator::estimate,
    model::{QuickPingInput, QuickPingReport},
    runner::{collect_samples, QuickPingPlan},
};
use crate::{
    models::devices,
    services::{
        monitoring::{checkers::ping::PingClient, execution_guard::try_acquire_quick_ping},
        plugins::transport::is_reachable_target,
        shared::errors::{AppError, AppResult},
    },
};

/// O endereço a medir: o IP do dispositivo ou um IP avulso de rede privada.
pub async fn resolve_target<C: ConnectionTrait>(
    db: &C,
    input: &QuickPingInput,
) -> AppResult<IpAddr> {
    if let Some(device_id) = input.device_id {
        let device = devices::Entity::find_by_id(device_id)
            .one(db)
            .await?
            .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
        let address = device
            .ip_address
            .filter(|ip| !ip.trim().is_empty())
            .ok_or_else(|| {
                AppError::validation(
                    "O dispositivo não tem endereço IP — o diagnóstico rápido precisa de um IP.",
                )
            })?;
        return address
            .trim()
            .parse()
            .map_err(|_| AppError::validation("Endereço IP do dispositivo inválido"));
    }
    let Some(raw) = input
        .ip
        .as_deref()
        .map(str::trim)
        .filter(|ip| !ip.is_empty())
    else {
        return Err(AppError::validation(
            "Informe o dispositivo ou o endereço IP",
        ));
    };
    let ip: IpAddr = raw
        .parse()
        .map_err(|_| AppError::validation("Endereço IP inválido"))?;
    if !is_reachable_target(ip) {
        return Err(AppError::validation(
            "Só endereços de rede privada, CGNAT ou link-local — para outro endereço, cadastre o \
             dispositivo.",
        ));
    }
    Ok(ip)
}

/// Roda a rodada padrão contra o alvo. Um diagnóstico por endereço de cada vez.
pub async fn run(
    client: &PingClient,
    target: IpAddr,
    cancel: &CancellationToken,
) -> AppResult<QuickPingReport> {
    let _guard = try_acquire_quick_ping(target).ok_or_else(|| {
        AppError::conflict("Já existe um diagnóstico rápido em andamento para este endereço")
    })?;
    let socket = client.for_ip(target).ok_or_else(|| {
        AppError::ServiceUnavailable(
            "Ping IPv6 indisponível: o sistema recusou o socket ICMPv6 sem privilégio.".into(),
        )
    })?;
    let samples = collect_samples(&socket, target, &QuickPingPlan::default(), cancel).await;
    Ok(estimate(&target.to_string(), &samples))
}
