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
//!
//! **Liderança (ADR 024).** Os três caminhos respeitam a função `Scheduler`:
//! o servidor e o `scheduler_loop` só rodam o ciclo enquanto este processo for
//! o líder (e o retomam se voltarem a sê-lo); o `scheduler_run` pega a função
//! para a única passada e **recusa** se outro processo for o líder — dois
//! ciclos ao mesmo tempo executariam os mesmos monitores. No SQLite o processo
//! é sempre o líder.

use std::collections::{BTreeMap, HashSet};

use chrono::Utc;
use futures::{stream, StreamExt};
use loco_rs::prelude::*;
use netmonitor_core::{
    cluster::leadership::{while_leader, LeaderRole, Leadership},
    models::monitors,
    monitoring::MonitorGroups,
    routines::{AreaRoutines, Cycle},
    shared::transaction::begin_write,
};
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};

use netmonitor_nucleo::{
    services::monitoring::{
        result_processor::process_result,
        scheduler::{
            dispatch_notifications, execute_one, rollup_monitor_results_if_due,
            run_data_pruner_if_due,
        },
    },
    services::probes::liveness::mark_stale_probes_offline,
    services::shared::errors::AppResult,
    services::syslog,
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
        Leadership::from_context(ctx)
            .run_once(LeaderRole::Scheduler, run_cycle(ctx))
            .await
            .map(|_| ())
            .map_err(Into::into)
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

/// Aguarda `Ctrl+C` (ou `SIGTERM` em sistemas Unix) para encerrar o laço
/// graciosamente entre ciclos sem deixar atualizações pela metade.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            () = async {
                if let Some(ref mut signal) = sigterm {
                    signal.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Repete [`run_cycle`] até `shutdown` completar.
///
/// Quem decide quando parar é o hospedeiro: o processo dedicado passa
/// [`shutdown_signal`]; o servidor passa um futuro que nunca completa, e o laço
/// termina com o processo — depois de o Loco drenar as conexões SSE, e não no
/// primeiro `SIGTERM`, enquanto elas ainda recebem os eventos do ciclo.
///
/// O ciclo roda **dentro** deste processo, e não como subprocesso por tique
/// (ADR 007): assim o socket ICMP, o pool de conexões e as cadências internas
/// (`is_due`) são abertos uma vez e sobrevivem entre ciclos.
///
/// Erro de um ciclo é registrado e o laço continua. Um ciclo que falha não pode
/// derrubar o agendamento inteiro — a checagem seguinte pode muito bem passar.
pub async fn run_forever(
    ctx: &AppContext,
    interval_seconds: u64,
    shutdown: impl std::future::Future<Output = ()>,
) {
    tracing::info!(interval_seconds, "scheduler inicializado");

    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_seconds));
    // Ciclo que estourou o intervalo não pode gerar rajada de tiques atrasados:
    // isso empilharia execuções em cima de um sistema que já está lento —
    // exatamente a hora de não piorar.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            biased;
            () = &mut shutdown => {
                tracing::info!("scheduler encerrando graciosamente");
                return;
            }
            _ = ticker.tick() => {}
        }
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
        let leadership = Leadership::from_context(ctx);
        let mut is_leader = leadership.watch(LeaderRole::Scheduler);
        let stop = tokio_util::sync::CancellationToken::new();
        tokio::spawn({
            let stop = stop.clone();
            async move {
                shutdown_signal().await;
                stop.cancel();
            }
        });
        // Líder: o sinal encerra o laço entre ciclos (`run_forever`).
        // Seguidor: não há ciclo a drenar, e o sinal encerra a espera.
        let cycle = while_leader(
            leadership.watch(LeaderRole::Scheduler),
            "ciclo_do_scheduler",
            || run_forever(ctx, seconds, stop.clone().cancelled_owned()),
        );
        tokio::pin!(cycle);
        loop {
            let leading = *is_leader.borrow_and_update();
            tokio::select! {
                () = &mut cycle => break,
                () = stop.cancelled(), if !leading => break,
                changed = is_leader.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
            }
        }
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
    // Monitores que uma área executa em grupo (os SNMP locais de um mesmo
    // dispositivo: uma coleta só) saem do despacho individual.
    let groups = MonitorGroups::from_context(ctx);
    let grouped: HashSet<(usize, i64)> = due
        .iter()
        .filter_map(|monitor| groups.group_of(monitor))
        .collect();
    // Persistir `next_run_at` em lote (agrupado por intervalo, numa única
    // transação de escrita) evita que dois schedulers executem o mesmo monitor
    // e elimina N `UPDATE`s individuais por ciclo.
    if !due.is_empty() {
        let mut by_interval: BTreeMap<i32, Vec<i64>> = BTreeMap::new();
        for monitor in &due {
            by_interval
                .entry(monitor.interval_seconds.max(1))
                .or_default()
                .push(monitor.id);
        }
        let tx = begin_write(&ctx.db).await?;
        for (interval_seconds, ids) in by_interval {
            let next_run: sea_orm::prelude::DateTimeWithTimeZone =
                (now + chrono::Duration::seconds(i64::from(interval_seconds))).into();
            // Lotes de até 400 ids para respeitar o limite de 999 parâmetros do SQLite.
            for chunk in ids.chunks(400) {
                monitors::Entity::update_many()
                    .col_expr(monitors::Column::NextRunAt, Expr::value(Some(next_run)))
                    .filter(monitors::Column::Id.is_in(chunk.iter().copied()))
                    .exec(&tx)
                    .await?;
            }
        }
        tx.commit().await?;
    }
    stream::iter(
        due.iter()
            .filter(|monitor| groups.group_of(monitor).is_none()),
    )
    .for_each_concurrent(MONITOR_CONCURRENCY, |monitor| async move {
        if let Err(error) = execute_one(ctx, monitor).await {
            tracing::warn!(%error, monitor_id = monitor.id, "falha ao executar monitor");
        }
    })
    .await;
    let groups = &groups;
    stream::iter(grouped)
        .for_each_concurrent(MONITOR_CONCURRENCY, |(index, key)| async move {
            if let Err(error) = run_group(ctx, groups, index, key, now).await {
                tracing::warn!(%error, grupo = key, "falha ao executar coleta consolidada");
            }
        })
        .await;
    // As rotinas das áreas (registro de rotinas, ADR 017): a coleta SNMP sem
    // monitor e a varredura agendada da Rede, a telemetria dos túneis da VPN,
    // o vigia do Wi-Fi e os testes de enlace agendados. Cada uma pausa com a
    // própria área quando é o caso; falha de uma não interrompe as outras.
    let cycle = Cycle {
        now,
        detached: detach_discovery,
    };
    for routine in AreaRoutines::from_context(ctx).routines() {
        if let Err(error) = routine.run(ctx, cycle).await {
            tracing::warn!(%error, rotina = routine.name(), "falha na rotina da área");
        }
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
    // Os eventos que o ciclo publicou chegam aos outros processos pelo
    // `NOTIFY` do próprio commit (ADR 024); o ciclo não entrega nada de
    // outro processo.
    Ok(due.len())
}

/// Executa um grupo de monitores e processa o resultado de cada um como o de
/// qualquer monitor.
async fn run_group(
    ctx: &AppContext,
    groups: &MonitorGroups,
    index: usize,
    key: i64,
    now: chrono::DateTime<Utc>,
) -> AppResult<()> {
    let Some(group) = groups.get(index) else {
        return Ok(());
    };
    for (monitor_id, result) in group.run(ctx, key, now).await? {
        if let Err(error) = process_result(ctx, monitor_id, &result, None).await {
            tracing::warn!(%error, monitor_id, grupo = key, "falha ao processar resultado do grupo");
        }
    }
    Ok(())
}
