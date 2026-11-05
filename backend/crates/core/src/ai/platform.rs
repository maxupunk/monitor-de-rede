//! Uma seção do `get_platform_status` contribuída por uma área (ADR 017): a
//! ferramenta é do núcleo, mas o estado da VPN é da área VPN e o das
//! varreduras é da área Rede.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::prelude::AppContext;
use serde_json::Value;

use std::collections::HashMap;

use sea_orm::EntityTrait;

use crate::{models::_entities::networks, shared::errors::AppResult};

const MIB: f64 = 1024.0 * 1024.0;

/// Teto das listas — também o da topologia.
pub const MAX_LIMIT: i64 = 50;

/// Bytes em MiB com duas casas — o tráfego que a IA lê.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn mib(bytes: i64) -> f64 {
    (bytes as f64 / MIB * 100.0).round() / 100.0
}

/// O instante em RFC 3339, em UTC.
#[must_use]
pub fn rfc3339<Tz: chrono::TimeZone>(at: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    at.with_timezone(&Utc).to_rfc3339()
}

#[async_trait]
pub trait PlatformSection: Send + Sync {
    /// O valor de `area` que pede esta seção (`vpn`, `discovery`).
    fn key(&self) -> &'static str;

    /// O estado da seção. `since` é o começo da janela de `hours`; `limit`, o
    /// tamanho das listas.
    async fn report(
        &self,
        ctx: &AppContext,
        since: DateTime<Utc>,
        limit: usize,
    ) -> AppResult<Value>;
}

/// `id → "nome (cidr)"` das redes — também a seção da descoberta usa.
///
/// # Errors
///
/// Falha do banco.
pub async fn network_names(ctx: &AppContext) -> AppResult<HashMap<i64, String>> {
    Ok(networks::Entity::find()
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|network| (network.id, format!("{} ({})", network.name, network.cidr)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trafego_da_vpn_em_mib() {
        assert!((mib(3 * 1024 * 1024) - 3.0).abs() < f64::EPSILON);
        assert!((mib(0)).abs() < f64::EPSILON);
    }
}
