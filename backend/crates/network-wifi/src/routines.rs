//! A rotina do Wi-Fi no ciclo do scheduler (registro de rotinas do núcleo).

use std::sync::Arc;

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::{
    areas::{self, Area},
    routines::{cadence::is_due, Cycle, Routine},
    shared::errors::AppResult,
};

/// Cadência do vigia dos clientes Wi-Fi vigiados: uma sessão SSH por AP a
/// cada 5 min — leve para o AP e fino o bastante para a regra de 10 min.
pub const WIFI_WATCH_INTERVAL_SECONDS: i64 = 300;

/// Dispara o vigia dos clientes Wi-Fi vigiados. Roda em segundo plano: a
/// leitura dos APs por SSH leva segundos e não pode atrasar os monitores.
/// Com a área Rede desligada (ADR 017) o vigia fica pausado.
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
        if !is_due("wifi_watch", WIFI_WATCH_INTERVAL_SECONDS, cycle.now) {
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
            if let Err(error) = crate::watch::run(&ctx).await {
                tracing::warn!(%error, "falha no vigia dos clientes Wi-Fi");
            }
        });
        Ok(())
    }
}

/// As rotinas deste submódulo.
#[must_use]
pub fn routines() -> Vec<Arc<dyn Routine>> {
    vec![Arc::new(WifiWatch)]
}
