//! O que a VPN contribui ao ciclo do scheduler (ADR 017, registro de
//! rotinas): a telemetria dos túneis. Alimenta os alertas dos peers, então
//! não pausa com a área desligada — sem servidor ativo ela não faz nada.

use std::sync::Arc;

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::{
    areas::Area,
    routines::{cadence::is_due, Cycle, Routine},
    shared::errors::AppResult,
};

use crate::traffic_recorder;

/// Cadência da leitura de status dos túneis VPN.
pub const VPN_STATUS_INTERVAL_SECONDS: i64 = 10;

/// Cadência da gravação de histórico de tráfego VPN.
pub const VPN_TRAFFIC_INTERVAL_SECONDS: i64 = 30;

/// Sincroniza o status (10s) e o histórico (30s) do tráfego dos túneis.
pub struct TunnelTelemetry;

#[async_trait]
impl Routine for TunnelTelemetry {
    fn name(&self) -> &'static str {
        "telemetria_dos_tuneis"
    }

    fn area(&self) -> Area {
        Area::Vpn
    }

    async fn run(&self, ctx: &AppContext, cycle: Cycle) -> AppResult<()> {
        let now = cycle.now;
        if is_due("vpn_traffic", VPN_TRAFFIC_INTERVAL_SECONDS, now) {
            traffic_recorder::record_all(ctx).await?;
            is_due("vpn_status", VPN_STATUS_INTERVAL_SECONDS, now);
            return Ok(());
        }
        if is_due("vpn_status", VPN_STATUS_INTERVAL_SECONDS, now) {
            traffic_recorder::sync_all(ctx).await?;
        }
        Ok(())
    }
}

/// As rotinas do ciclo desta área.
#[must_use]
pub fn routines() -> Vec<Arc<dyn Routine>> {
    vec![Arc::new(TunnelTelemetry)]
}
