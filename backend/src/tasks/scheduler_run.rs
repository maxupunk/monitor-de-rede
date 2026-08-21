//! Um ciclo do scheduler de monitores.
//!
//! Duas tarefas moram aqui, e a diferença é só quem repete:
//!
//! * [`SchedulerRun`] (`task scheduler_run`) — **um** ciclo e sai. É o comando
//!   manual, para depurar ou forçar uma passada.
//! * [`SchedulerLoop`] (`task scheduler_loop`) — chama o mesmo ciclo em laço.
//!
//! O laço em si é [`run_forever`], e quem o hospeda em produção é o **próprio
//! servidor** (`initializers::monitoring`). O container `scheduler` deixou de
//! existir: ele nunca precisou ser outro processo — o que o ADR 007 exige é que
//! o ciclo rode *dentro* de um processo longevo, e não como subprocesso por
//! tique. O servidor é longevo, tem o mesmo pool e o mesmo socket ICMP, e ainda
//! ganha uma vantagem que o container separado nunca teve: os eventos que o
//! ciclo publica nascem no mesmo processo que mantém as conexões SSE.
//!
//! A tarefa `scheduler_loop` continua registrada para quem quiser o ciclo em um
//! processo à parte (`SCHEDULER_ENABLED=false` no servidor + um container só
//! com a tarefa).

use std::collections::HashSet;

use chrono::Utc;
use futures::{stream, StreamExt};
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, Set};

use crate::{
    models::monitors,
    services::{
        discovery::queue::{process_pending_runs, schedule_due_networks, spawn_pending_run},
        monitoring::scheduler::{
            dispatch_notifications, execute_one, execute_snmp_device_group, local_snmp_device_id,
            rollup_monitor_results_if_due, run_data_pruner_if_due, sync_vpn_traffic_if_due,
        },
        probes::liveness::mark_stale_probes_offline,
        shared::errors::AppResult,
        syslog,
    },
};

/// Intervalo padrão entre ciclos, em segundos. Ajustável por
/// `SCHEDULER_INTERVAL_SECONDS` ou pelo argumento `interval_seconds`.
pub const DEFAULT_CYCLE_SECONDS: u64 = 5;
const MONITOR_CONCURRENCY: usize = 16;

/// Executa **um** ciclo e sai. Comando manual.
pub struct SchedulerRun;

#[async_trait]
impl Task for SchedulerRun {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "scheduler_run".into(),
            detail: "Executa um único ciclo dos monitores vencidos e sai".into(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &task::Vars) -> Result<()> {
        run_cycle(ctx).await.map(|_| ()).map_err(Into::into)
    }
}

/// Intervalo entre ciclos: argumento da CLI, `SCHEDULER_INTERVAL_SECONDS` ou o
/// padrão. Zero é descartado — pararia o ticker do tokio com um panic.
#[must_use]
pub fn resolve_interval_seconds(explicit: Option<&str>) -> u64 {
    explicit
        .and_then(|value| value.trim().parse::<u64>().ok())
        .or_else(|| {
            std::env::var("SCHEDULER_INTERVAL_SECONDS")
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
        })
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CYCLE_SECONDS)
}

/// Repete [`run_cycle`] para sempre. Nunca retorna.
///
/// O ciclo roda **dentro** deste processo, e não como subprocesso por tique
/// (ADR 007): assim o socket ICMP, o pool de conexões e as cadências internas
/// (`is_due`) são abertos uma vez e sobrevivem entre ciclos.
///
/// Erro de um ciclo é registrado e o laço continua. Um ciclo que falha não pode
/// derrubar o agendamento inteiro — a checagem seguinte pode muito bem passar.
pub async fn run_forever(ctx: &AppContext, interval_seconds: u64) {
    tracing::info!(interval_seconds, "scheduler inicializado");

    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_seconds));
    // Ciclo que estourou o intervalo não pode gerar rajada de tiques atrasados:
    // isso empilharia execuções em cima de um sistema que já está lento —
    // exatamente a hora de não piorar.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;
        match run_cycle_inner(ctx, true).await {
            Ok(0) => {}
            Ok(count) => tracing::debug!(monitores = count, "ciclo concluído"),
            Err(error) => tracing::warn!(%error, "ciclo do scheduler falhou"),
        }
    }
}

/// O mesmo laço como tarefa da CLI, para rodar o ciclo num processo à parte.
pub struct SchedulerLoop;

#[async_trait]
impl Task for SchedulerLoop {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "scheduler_loop".into(),
            detail: "Executa o ciclo dos monitores continuamente (processo dedicado)".into(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &task::Vars) -> Result<()> {
        let seconds = resolve_interval_seconds(vars.cli_arg("interval_seconds").ok());
        run_forever(ctx, seconds).await;
        Ok(())
    }
}

/// Executa os monitores vencidos e grava `next_run_at` antes de medir.
///
/// Cada bloco tem tratamento próprio: falha de um não interrompe os outros
/// (§9.2). O watchdog vem primeiro — um probe que caiu precisa aparecer como
/// caído *antes* do despacho, ou o operador só vê monitores parados sem
/// explicação.
pub async fn run_cycle(ctx: &AppContext) -> AppResult<usize> {
    run_cycle_inner(ctx, false).await
}

async fn run_cycle_inner(ctx: &AppContext, detach_discovery: bool) -> AppResult<usize> {
    if let Err(error) = mark_stale_probes_offline(ctx).await {
        tracing::warn!(%error, "falha ao revisar a vida dos probes");
    }

    let now = Utc::now();
    let due = monitors::Entity::find_due(now.into()).all(&ctx.db).await?;
    let local_snmp_devices: HashSet<i64> = due.iter().filter_map(local_snmp_device_id).collect();
    for monitor in &due {
        // Persistir primeiro evita que dois processos do scheduler executem a
        // mesma linha quando o ciclo seguinte começar antes de a rede responder.
        let mut active: monitors::ActiveModel = monitor.clone().into();
        active.next_run_at = Set(Some(
            (now + chrono::Duration::seconds(i64::from(monitor.interval_seconds.max(1)))).into(),
        ));
        active.update(&ctx.db).await?;
    }
    stream::iter(due.iter().filter(|monitor| {
        !local_snmp_device_id(monitor).is_some_and(|id| local_snmp_devices.contains(&id))
    }))
    .for_each_concurrent(MONITOR_CONCURRENCY, |monitor| async move {
        if let Err(error) = execute_one(ctx, monitor).await {
            tracing::warn!(%error, monitor_id = monitor.id, "falha ao executar monitor");
        }
    })
    .await;
    stream::iter(local_snmp_devices)
        .for_each_concurrent(MONITOR_CONCURRENCY, |device_id| async move {
            if let Err(error) = execute_snmp_device_group(ctx, device_id, now).await {
                tracing::warn!(%error, device_id, "falha ao executar coleta SNMP consolidada");
            }
        })
        .await;
    if let Err(error) = sync_vpn_traffic_if_due(ctx).await {
        tracing::warn!(%error, "falha ao sincronizar tráfego VPN");
    }
    // A fila de discovery é persistente: o scheduler apenas enfileira as redes
    // vencidas e processa uma por ciclo para não saturar a LAN.
    if let Err(error) = schedule_due_networks(&ctx.db).await {
        tracing::warn!(%error, "falha ao agendar discovery");
    }
    let discovery_result = if detach_discovery {
        spawn_pending_run(ctx).await
    } else {
        process_pending_runs(ctx).await
    };
    if let Err(error) = discovery_result {
        tracing::warn!(%error, "falha ao processar discovery");
    }
    // As notificações da Fase 4 saem daqui, e não do ponto onde o alerta nasce:
    // a decisão de notificar virou linha em `notification_outbox`, e é o
    // despachante que a entrega, agrupa ou engole (ver `notifications::outbox`).
    if let Err(error) = dispatch_notifications(ctx).await {
        tracing::warn!(%error, "falha ao despachar notificações pendentes");
    }
    if let Err(error) = run_data_pruner_if_due(ctx).await {
        tracing::warn!(%error, "falha ao executar purga de dados antigos");
    }
    if let Err(error) = rollup_monitor_results_if_due(ctx).await {
        tracing::warn!(%error, "falha ao executar rollup de resultados");
    }
    // Padrões de log casam na ingestão; a avaliação das janelas acontece aqui,
    // junto do resto do motor de alertas — é o que dá ao alerta de log a mesma
    // histerese, detecção de flapping e higiene de notificação das demais
    // regras (ver `syslog::matcher`).
    if let Some(servico) = syslog::SyslogService::from_context(ctx) {
        if let Err(error) = syslog::matcher::evaluate(ctx, &servico.matcher).await {
            tracing::warn!(%error, "falha ao avaliar padrões de log");
        }
    }
    // O relay do `event_outbox` **não** roda aqui. Ele é in-process e só
    // entrega a quem tem conexão SSE aberta — o servidor. Ver
    // `initializers::monitoring::spawn_event_relay`.
    Ok(due.len())
}
