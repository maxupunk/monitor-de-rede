//! O coletor do mapa Wi-Fi aberto (AGENTS §9).
//!
//! Os APs não avisam quando um aparelho troca de rádio: alguém precisa ler.
//! Enquanto alguma aba exibe o mapa — o interesse `wifi` declarado na própria
//! conexão SSE (`Interests`) — **um** laço no servidor lê a frota no intervalo
//! curto das Configurações de Wi-Fi (padrão 15 s) e publica cada AP pelo SSE.
//! Nunca um ciclo por aba, nunca consulta do navegador. Sem ninguém olhando,
//! o laço só confere o interesse; a rotina em segundo plano
//! ([`crate::routines`]) mantém o histórico de roaming e os alertas.

use std::time::Duration;

use loco_rs::app::AppContext;
use netmonitor_core::{
    areas::{self, Area},
    events::{interest, Interests},
    shared::errors::AppResult,
};
use netmonitor_network::plugins::wifi_source::Requester;

use crate::{rounds, settings};

/// Sem ninguém olhando, de quanto em quanto tempo o laço confere o interesse:
/// o mapa aberto começa a ser lido em até 2 s.
const IDLE_CHECK: Duration = Duration::from_secs(2);

/// O que uma volta do laço fez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveStep {
    /// Nenhuma aba exibe o mapa: nada é lido.
    NoSubscriber,
    /// A área Rede está desligada (ADR 017): o mapa não lê.
    AreaOff,
    /// Outra rodada (a da rotina, por exemplo) estava em andamento.
    Busy,
    /// Leu a frota.
    Read,
}

/// Alguma aba exibe o mapa agora?
#[must_use]
pub fn is_watched(ctx: &AppContext) -> bool {
    Interests::from_context(ctx).is_watched(interest::WIFI)
}

/// Uma volta: lê a frota só se alguém olha e a área está ligada.
///
/// # Errors
///
/// Erro do banco.
pub async fn step(ctx: &AppContext) -> AppResult<LiveStep> {
    if !is_watched(ctx) {
        return Ok(LiveStep::NoSubscriber);
    }
    if !areas::is_enabled(&ctx.db, Area::Network).await? {
        return Ok(LiveStep::AreaOff);
    }
    Ok(match rounds::run(ctx, Requester::live()).await? {
        Some(_) => LiveStep::Read,
        None => LiveStep::Busy,
    })
}

/// Quanto esperar até a próxima volta: o intervalo configurado, descontado o
/// tempo que a leitura levou (uma rodada por vez, sem sobrepor); sem leitura,
/// só a conferência do interesse.
#[must_use]
pub fn next_wait(step: LiveStep, interval: Duration, elapsed: Duration) -> Duration {
    match step {
        LiveStep::Read => interval.saturating_sub(elapsed).max(Duration::from_secs(1)),
        LiveStep::NoSubscriber | LiveStep::AreaOff | LiveStep::Busy => IDLE_CHECK,
    }
}

/// Sobe o laço no processo do servidor (é ele que tem as conexões SSE).
pub fn spawn(ctx: AppContext) {
    tokio::spawn(async move {
        loop {
            let started = std::time::Instant::now();
            let step = step(&ctx).await.unwrap_or_else(|error| {
                tracing::warn!(%error, "falha na leitura do mapa Wi-Fi");
                LiveStep::Busy
            });
            let interval = match step {
                LiveStep::Read => settings::load(&ctx.db)
                    .await
                    .map(|settings| settings.live_interval_s)
                    .unwrap_or_default(),
                _ => 0,
            };
            tokio::time::sleep(next_wait(
                step,
                Duration::from_secs(u64::from(interval)),
                started.elapsed(),
            ))
            .await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_le_de_novo_depois_do_intervalo_descontada_a_leitura() {
        let quinze = Duration::from_secs(15);
        assert_eq!(
            next_wait(LiveStep::Read, quinze, Duration::from_secs(4)),
            Duration::from_secs(11)
        );
        assert_eq!(
            next_wait(LiveStep::Read, quinze, Duration::from_secs(40)),
            Duration::from_secs(1),
            "leitura mais longa que o intervalo: a próxima vem logo, sem sobrepor"
        );
        assert_eq!(
            next_wait(LiveStep::NoSubscriber, quinze, Duration::ZERO),
            IDLE_CHECK
        );
    }
}
