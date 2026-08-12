//! Inicialização **exclusiva do processo servidor**.
//!
//! Um `Initializer` do Loco só roda no `run_app` — ou seja, no `start`. É
//! justamente o que se quer aqui: tudo neste arquivo depende de o processo ter
//! banco e atender HTTP. As dependências que **todo** processo precisa moram em
//! [`super::process_deps`].

use std::time::Duration;

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::services::{
    alerts::catalog::service as alert_catalog, events::relay::relay_pending,
    network_tools::dns::registry::DnsServerRegistry, vpn::probe_registrar as vpn_probe_registrar,
};

/// Com que frequência o servidor drena o `event_outbox` para o barramento SSE.
///
/// Cinco segundos acompanha o ciclo do scheduler: é o intervalo em que novos
/// eventos aparecem na tabela. O relay sai barato quando não há ninguém
/// conectado — a primeira coisa que ele faz é checar `has_subscribers()`.
const EVENT_RELAY_INTERVAL: Duration = Duration::from_secs(5);

pub struct MonitoringInitializer;

#[async_trait]
impl Initializer for MonitoringInitializer {
    fn name(&self) -> String {
        "monitoring".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        spawn_event_relay(ctx.clone());

        // Cadastro é uma conveniência de boot: banco indisponível não impede o
        // processo HTTP de subir, e a operação é idempotente em banco vazio.
        if let Err(error) = DnsServerRegistry::ensure_defaults(&ctx.db).await {
            tracing::warn!(%error, "não foi possível semear resolvedores DNS padrão");
        }
        // Provisiona o conjunto básico de regras em instalação nova. Falha aqui
        // **não** impede o boot: o banco pode estar migrando, e a API precisa
        // subir de qualquer forma. A operação é idempotente e só age quando não
        // existe regra alguma.
        match alert_catalog::ensure_defaults(&ctx.db).await {
            Ok(result) if !result.created.is_empty() => {
                tracing::info!(
                    created = result.created.len(),
                    "regras básicas aplicadas a partir do catálogo"
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "não foi possível provisionar as regras básicas de alerta");
            }
        }
        // Registro idempotente do `vpn-probe`. Também não impede o boot: o
        // container do túnel pode subir depois, e o registro é refeito no
        // próximo start. ⚠️ O fallback para o token compartilhado é o que
        // permite o `vpn-probe` autenticar sem configuração.
        if let Err(error) = vpn_probe_registrar::register(&ctx.db, None).await {
            tracing::warn!(%error, "não foi possível registrar o probe dedicado da VPN");
        }
        Ok(())
    }
}

/// Sobe o laço que replica o `event_outbox` para os clientes SSE.
///
/// **Só faz sentido aqui.** O barramento (`EventBus`) é in-process: um
/// `broadcast::Sender` vivo na memória de quem roda. Quem tem as conexões SSE
/// abertas é o servidor, então é o servidor que precisa ler a tabela e
/// republicar localmente.
///
/// Antes, o relay era chamado de dentro do ciclo do `scheduler`. Lá ele nunca
/// entregava nada: `has_subscribers()` é sempre falso num processo que não
/// atende HTTP, então a função saía no primeiro `if` e o evento gerado pelo
/// scheduler — mudança de estado de dispositivo, alerta aberto — nunca chegava
/// à tela.
fn spawn_event_relay(ctx: AppContext) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(EVENT_RELAY_INTERVAL);
        // Um tique perdido não deve virar rajada de tiques atrasados.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match relay_pending(&ctx).await {
                Ok(0) => {}
                Ok(count) => tracing::debug!(count, "eventos replicados para os clientes SSE"),
                Err(error) => tracing::warn!(%error, "falha ao retransmitir eventos"),
            }
        }
    });
}
