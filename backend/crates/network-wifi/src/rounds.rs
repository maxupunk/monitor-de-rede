//! Uma rodada da frota: lê todos os APs com plugin Wi-Fi (o SSE leva cada
//! um à tela e o roaming é registrado no caminho, em `collector`) e procura
//! os aparelhos vigiados nessa leitura.
//!
//! Quem pede uma rodada: o coletor do mapa aberto ([`crate::live`]), a rotina
//! em segundo plano ([`crate::routines`]) e o vigia (`watch::run`). Uma rodada
//! por vez no processo — quem chega com outra em andamento desiste, e cada AP
//! continua lido por um pedido de cada vez (`collector::ReadingGuard`).

use loco_rs::app::AppContext;
use netmonitor_core::shared::{errors::AppResult, run_guard::RunGuard};
use netmonitor_network::plugins::wifi_source::Requester;

use crate::{
    collector::{self, FleetReading},
    watch,
};

/// O que uma rodada leu e quantos vigiados tiveram fato.
pub struct RoundOutcome {
    pub reading: FleetReading,
    pub watched_observed: usize,
}

/// Uma rodada; `None` quando outra já está em andamento.
///
/// # Errors
///
/// Erro do banco.
pub async fn run(ctx: &AppContext, requester: Requester) -> AppResult<Option<RoundOutcome>> {
    let Ok(_turn) = RunGuard::acquire("wifi_fleet_round", 0, "rodada Wi-Fi em andamento") else {
        return Ok(None);
    };
    let reading = collector::read_fleet(ctx, requester).await?;
    let watched_observed = watch::observe_fleet(ctx, &reading).await?;
    Ok(Some(RoundOutcome {
        reading,
        watched_observed,
    }))
}
