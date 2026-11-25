//! Inicialização **exclusiva do processo servidor**.
//!
//! Um `Initializer` do Loco só roda no `run_app` — ou seja, no `start`. É
//! justamente o que se quer aqui: tudo neste arquivo depende de o processo ter
//! banco e atender HTTP. As dependências que **todo** processo precisa moram em
//! [`super::process_deps`] — inclusive a escuta dos eventos dos outros
//! processos da central (ADR 024).
//!
//! **Rotinas do líder e deveres de cada processo** (ADR 024). Com várias
//! instâncias da central sobre o mesmo PostgreSQL, só o processo líder de cada
//! função roda o que agiria duas vezes sobre o banco ou o mundo:
//!
//! | Função ([`LeaderRole`]) | Rotinas |
//! |---|---|
//! | `Scheduler` | o ciclo: monitores, rotinas e retenção das áreas, despacho de notificações (e do Web Push), purga, rollup |
//! | `Background` | backups (do NetMonitor e dos bancos), despachante de deploys, tarefas agendadas, atualização do registro de fabricantes, IA proativa (`initializers::ai`), limpeza dos testes de enlace (`initializers::link_tests`) |
//!
//! Ficam em **todo** processo, porque servem às conexões dele: o SSE e os
//! snapshots publicados no boot, os canais dos agentes conectados aqui (modo ao
//! vivo, `last_seen_at`, entrega de tarefas), o mapa Wi-Fi ao vivo das abas
//! **deste** processo (o interesse `wifi` é da conexão SSE), a porta de syslog,
//! a carga do registro de fabricantes em memória e os cadastros idempotentes
//! de boot.
//!
//! O despachante de deploys fala com os agentes pelo canal deste processo:
//! numa instalação com várias instâncias, os agentes precisam chegar à que é
//! líder (é a topologia do compose, com uma instância só).

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    environment::Environment,
    Result,
};

use netmonitor_core::{
    cluster::leadership::{spawn_while_leader, LeaderRole},
    vpn_probe::probe_is_external,
};

use netmonitor_nucleo::{
    services::agents::{background as agents_background, local as local_agent},
    services::alerts::catalog::service as alert_catalog,
    services::network_tools::dns::registry::DnsServerRegistry,
    services::probes::vpn_registrar as vpn_probe_registrar,
};

use crate::tasks::scheduler_run;

pub struct MonitoringInitializer;

#[async_trait]
impl Initializer for MonitoringInitializer {
    fn name(&self) -> String {
        "monitoring".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        // Canal dos agentes — o local e os remotos: modo ao vivo e entrega de
        // tarefas (ADR 011). O Docker e o histórico de métricas deste host
        // chegam pelo agente local (ADR 014). Dever de cada processo: são os
        // agentes conectados **aqui**.
        agents_background::spawn(ctx.clone());
        spawn_scheduler(ctx);
        // Registro de fabricantes do IEEE em memória: cada processo carrega o
        // seu; quem o baixa e regrava é o líder (abaixo).
        let db = ctx.db.clone();
        tokio::spawn(async move { netmonitor_network::vendors::service::ensure_loaded(&db).await });
        if ctx.environment != Environment::Test {
            spawn_background_routines(ctx);
        }
        // O mapa Wi-Fi aberto: um coletor só por processo, que lê a frota no
        // intervalo curto enquanto alguma conexão SSE **deste** processo
        // declara o interesse `wifi`. Os snapshots chegam aos outros processos
        // pelo barramento (ADR 024).
        netmonitor_network_wifi::live::spawn(ctx.clone());

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
        ensure_local_agent(ctx).await;
        // A lista dos aparelhos vigiados fica no barramento desde o boot: a
        // tela a recebe ao conectar no SSE, sem consulta.
        netmonitor_network_wifi::watchlist::publish(ctx).await;
        Ok(())
    }
}

/// Cadastro idempotente do agente local e do token dele (ADR 014). Sem
/// `LOCAL_AGENT_TOKEN_FILE` não há agente local — desenvolvimento fora do
/// Docker. Falha aqui não impede o boot: a tela de Docker só fica sem o host
/// desta central até o próximo.
async fn ensure_local_agent(ctx: &AppContext) {
    let Some(path) = local_agent::token_file() else {
        tracing::info!("sem LOCAL_AGENT_TOKEN_FILE: o Docker desta central não será gerenciado");
        return;
    };
    match local_agent::ensure(&ctx.db, &path).await {
        Ok(probe) => tracing::info!(probe_id = probe.id, "agente local pronto"),
        Err(error) => tracing::warn!(%error, "não foi possível preparar o agente local"),
    }
}

/// As rotinas de fundo do líder da função `Background` (ADR 024). Fora do
/// ambiente de teste — lá, quem testa chama cada `run_due` diretamente.
fn spawn_background_routines(ctx: &AppContext) {
    let role = LeaderRole::Background;
    // Registro de fabricantes do IEEE: baixa e regrava quando vence.
    spawn_while_leader(
        ctx,
        role,
        "registro_de_fabricantes",
        netmonitor_network::vendors::service::auto_update,
    );
    // Backup automático do próprio NetMonitor.
    spawn_while_leader(
        ctx,
        role,
        "backup_netmonitor",
        netmonitor_nucleo::services::backup::schedule::run,
    );
    // Backup automático dos bancos de dados cadastrados.
    spawn_while_leader(
        ctx,
        role,
        "backup_bancos",
        netmonitor_servers::databases::schedule::run,
    );
    // Deploy contínuo (ADR 021): o despachante da fila fala com os agentes
    // pelo canal deste processo.
    spawn_while_leader(
        ctx,
        role,
        "despachante_de_deploys",
        netmonitor_servers::deploys::orchestrator::run,
    );
    // Tarefas agendadas (cron): ferramentas do catálogo da IA nos horários
    // marcados, em nome de quem as salvou.
    spawn_while_leader(
        ctx,
        role,
        "tarefas_agendadas",
        netmonitor_nucleo::services::scheduled_tasks::runner::run,
    );
}

/// Sobe o ciclo do scheduler dentro do processo do servidor, como rotina do
/// líder da função `Scheduler` (ADR 024): com várias instâncias, só uma roda o
/// ciclo; se ela cair, outra assume.
///
/// `SCHEDULER_ENABLED=false` desliga — é o que permite tirar o ciclo daqui e
/// pô-lo num processo próprio (`task scheduler_loop`) numa instalação grande.
/// O processo que desliga nem disputa a função, então não tira a vez do
/// processo dedicado.
fn spawn_scheduler(ctx: &AppContext) {
    if !scheduler_enabled(&ctx.environment, std::env::var("SCHEDULER_ENABLED").ok()) {
        tracing::info!("scheduler desligado neste processo (SCHEDULER_ENABLED=false)");
        return;
    }
    let seconds = scheduler_run::resolve_interval_seconds(None);
    // Sem handler de sinal próprio: o laço vive enquanto o servidor viver, e
    // o encerramento é o do Loco (que drena as conexões antes de sair).
    spawn_while_leader(
        ctx,
        LeaderRole::Scheduler,
        "ciclo_do_scheduler",
        move |ctx| async move {
            scheduler_run::run_forever(&ctx, seconds, std::future::pending()).await;
        },
    );
}

/// Ligado por padrão. Só `false`/`0` desligam — qualquer outro valor mantém o
/// ciclo de pé, porque um erro de digitação aqui pararia o monitoramento
/// inteiro em silêncio.
///
/// **No ambiente de teste, desligado.** O harness do Loco (`boot_test`) roda
/// os initializers, e um ciclo real a cada 5 s checava de verdade os
/// monitores criados pelos testes — um `down` real caía entre os resultados
/// que o teste injeta e mudava o estado do alerta só quando a suíte estava
/// lenta. Quem testa o ciclo o chama diretamente.
fn scheduler_enabled(environment: &Environment, setting: Option<String>) -> bool {
    if *environment == Environment::Test {
        return false;
    }
    setting.is_none_or(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "false" | "0" | "no" | "off"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Valor que não é uma negação conhecida mantém o ciclo de pé: um erro de
    /// digitação no compose não pode parar o monitoramento em silêncio.
    #[test]
    fn o_scheduler_vem_ligado_so_um_no_explicito_desliga_e_nunca_no_teste() {
        assert!(scheduler_enabled(&Environment::Production, None));
        assert!(scheduler_enabled(
            &Environment::Development,
            Some("sim".into())
        ));
        assert!(!scheduler_enabled(
            &Environment::Production,
            Some(" False ".into())
        ));
        assert!(!scheduler_enabled(
            &Environment::Production,
            Some("0".into())
        ));
        assert!(!scheduler_enabled(&Environment::Test, None));
        assert!(!scheduler_enabled(&Environment::Test, Some("true".into())));
    }
}
