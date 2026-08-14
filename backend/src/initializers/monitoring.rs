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

use crate::{
    services::{
        alerts::catalog::service as alert_catalog, events::relay::relay_pending,
        network_tools::dns::registry::DnsServerRegistry, vpn::probe_is_external,
        vpn::probe_registrar as vpn_probe_registrar,
    },
    tasks::scheduler_run,
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
        spawn_scheduler(ctx.clone());

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
        // Registro idempotente do `vpn-probe`, e **só** quando o túnel está em
        // outro container. Na topologia padrão o WireGuard sobe ao lado da API,
        // no mesmo namespace de rede: registrar o agente ali criaria um probe
        // que nunca bate heartbeat — vermelho permanente na tela e todo monitor
        // da VPN caindo no fallback local a cada ciclo.
        //
        // Sem o registro, `monitor_provisioner::resolve_probe_id` devolve
        // `None` e os monitores rodam locais, que é exatamente o certo: a `wg0`
        // é do próprio processo.
        //
        // ⚠️ O fallback para o token compartilhado é o que permite o
        // `vpn-probe` autenticar sem configuração — ver §6 do AGENTS.md.
        if probe_is_external() {
            if let Err(error) = vpn_probe_registrar::register(&ctx.db, None).await {
                tracing::warn!(%error, "não foi possível registrar o probe dedicado da VPN");
            }
        }
        Ok(())
    }
}

/// Sobe o ciclo do scheduler dentro do processo do servidor.
///
/// `SCHEDULER_ENABLED=false` desliga — é o que permite tirar o ciclo daqui e
/// pô-lo num processo próprio (`task scheduler_loop`) numa instalação grande,
/// sem que os dois disputem os mesmos monitores.
fn spawn_scheduler(ctx: AppContext) {
    if !scheduler_enabled() {
        tracing::info!("scheduler desligado neste processo (SCHEDULER_ENABLED=false)");
        return;
    }
    let seconds = scheduler_run::resolve_interval_seconds(None);
    tokio::spawn(async move { scheduler_run::run_forever(&ctx, seconds).await });
}

/// Ligado por padrão. Só `false`/`0` desligam — qualquer outro valor mantém o
/// ciclo de pé, porque um erro de digitação aqui pararia o monitoramento
/// inteiro em silêncio.
fn scheduler_enabled() -> bool {
    std::env::var("SCHEDULER_ENABLED").map_or(true, |value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "false" | "0" | "no" | "off"
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn o_scheduler_vem_ligado_e_so_um_no_explicito_desliga() {
        std::env::remove_var("SCHEDULER_ENABLED");
        assert!(scheduler_enabled(), "ausente = ligado");
        std::env::set_var("SCHEDULER_ENABLED", "false");
        assert!(!scheduler_enabled());
        std::env::set_var("SCHEDULER_ENABLED", "0");
        assert!(!scheduler_enabled());
        // Valor que não é uma negação conhecida mantém o ciclo de pé: um erro
        // de digitação no compose não pode parar o monitoramento em silêncio.
        std::env::set_var("SCHEDULER_ENABLED", "sim");
        assert!(scheduler_enabled());
        std::env::remove_var("SCHEDULER_ENABLED");
    }
}
