//! As rotinas do Wi-Fi no ciclo do scheduler e na purga (registro de rotinas
//! do núcleo).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use netmonitor_core::{
    areas::{self, Area},
    routines::{cadence::is_due, Cycle, RetentionJob, Routine},
    shared::errors::AppResult,
};
use netmonitor_network::plugins::wifi_source::Requester;
use sea_orm::DatabaseConnection;

use crate::{live, roaming, rounds, settings};

/// A leitura leve da frota sem ninguém olhando o mapa: mantém o histórico de
/// roaming, o ping-pong e o vigia dos aparelhos vigiados. A cadência é a das
/// Configurações de Wi-Fi (padrão 2 min — fino o bastante para a regra de
/// 10 min do vigiado e leve para o AP: uma sessão SSH por rodada).
///
/// Com o mapa aberto neste processo, quem lê é o coletor do mapa
/// ([`live`]), no intervalo curto, e a rotina não lê de novo. Com a área
/// Rede desligada (ADR 017) fica pausada. Roda em segundo plano: a leitura
/// dos APs por SSH leva segundos e não pode atrasar os monitores.
pub struct WifiWatch;

#[async_trait]
impl Routine for WifiWatch {
    fn name(&self) -> &'static str {
        "vigia_wifi"
    }

    fn area(&self) -> Area {
        Area::Network
    }

    async fn run(&self, ctx: &AppContext, cycle: Cycle) -> AppResult<()> {
        let interval = settings::load(&ctx.db).await?.background_interval_s;
        if !is_due("wifi_watch", i64::from(interval), cycle.now) {
            return Ok(());
        }
        if live::is_watched(ctx) {
            return Ok(());
        }
        let ctx = ctx.clone();
        tokio::spawn(async move {
            if !areas::is_enabled(&ctx.db, Area::Network)
                .await
                .unwrap_or(true)
            {
                return;
            }
            if let Err(error) = rounds::run(&ctx, Requester::watch()).await {
                tracing::warn!(%error, "falha na leitura do Wi-Fi em segundo plano");
            }
        });
        Ok(())
    }
}

/// O histórico de roaming, pelo prazo das Configurações de Wi-Fi (padrão 30
/// dias).
pub struct RoamingRetention;

#[async_trait]
impl RetentionJob for RoamingRetention {
    fn name(&self) -> &'static str {
        "roaming_wifi"
    }

    fn area(&self) -> Area {
        Area::Network
    }

    async fn prune(&self, db: &DatabaseConnection, now: DateTime<Utc>) -> AppResult<u64> {
        roaming::prune(db, now).await
    }
}

/// As rotinas deste submódulo.
#[must_use]
pub fn routines() -> Vec<Arc<dyn Routine>> {
    vec![Arc::new(WifiWatch)]
}

/// A retenção deste submódulo.
#[must_use]
pub fn retention() -> Vec<Arc<dyn RetentionJob>> {
    vec![Arc::new(RoamingRetention)]
}
