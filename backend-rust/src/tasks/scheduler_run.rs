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

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use chrono::{DateTime, Utc};
use loco_rs::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::{
    models::{monitors, probes},
    services::{
        discovery::queue::{process_pending_runs, schedule_due_networks},
        maintenance::data_pruner,
        monitoring::{
            contracts::{CheckResult, MonitorStatus},
            execution_guard::try_acquire_monitor,
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
        },
        probes::{
            dispatcher::{self, ProbeTask},
            liveness::{is_probe_alive, mark_stale_probes_offline},
        },
        shared::errors::AppResult,
        vpn::traffic_recorder,
    },
};

/// Intervalo padrão entre ciclos, em segundos. Ajustável por
/// `SCHEDULER_INTERVAL_SECONDS` ou pelo argumento `interval_seconds`.
pub const DEFAULT_CYCLE_SECONDS: u64 = 5;

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
        match run_cycle(ctx).await {
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
    if let Err(error) = mark_stale_probes_offline(ctx).await {
        tracing::warn!(%error, "falha ao revisar a vida dos probes");
    }

    let now = Utc::now();
    let due = monitors::Entity::find_due(now.into()).all(&ctx.db).await?;
    for monitor in &due {
        // Persistir primeiro evita que dois processos do scheduler executem a
        // mesma linha quando o ciclo seguinte começar antes de a rede responder.
        let mut active: monitors::ActiveModel = monitor.clone().into();
        active.next_run_at = Set(Some(
            (now + chrono::Duration::seconds(i64::from(monitor.interval_seconds.max(1)))).into(),
        ));
        active.update(&ctx.db).await?;
    }
    for monitor in &due {
        if let Err(error) = execute_one(ctx, monitor).await {
            tracing::warn!(%error, monitor_id = monitor.id, "falha ao executar monitor");
        }
    }
    if let Err(error) = sync_vpn_traffic_if_due(ctx).await {
        tracing::warn!(%error, "falha ao sincronizar tráfego VPN");
    }
    // A fila de discovery é persistente: o scheduler apenas enfileira as redes
    // vencidas e processa uma por ciclo para não saturar a LAN.
    if let Err(error) = schedule_due_networks(&ctx.db).await {
        tracing::warn!(%error, "falha ao agendar discovery");
    }
    if let Err(error) = process_pending_runs(ctx).await {
        tracing::warn!(%error, "falha ao processar discovery");
    }
    if let Err(error) = run_data_pruner_if_due(ctx).await {
        tracing::warn!(%error, "falha ao executar purga de dados antigos");
    }
    // O relay do `event_outbox` **não** roda aqui. Ele é in-process e só
    // entrega a quem tem conexão SSE aberta — o servidor. Ver
    // `initializers::monitoring::spawn_event_relay`.
    Ok(due.len())
}

/// Cadência da leitura de status dos túneis. Fina de propósito: é o que faz a
/// tela de dispositivos VPN acompanhar um túnel que acabou de subir.
const VPN_STATUS_INTERVAL_SECONDS: i64 = 10;

/// Cadência da gravação de histórico de tráfego VPN — quatro linhas em
/// `metrics` por peer, não precisa ser fina.
const VPN_TRAFFIC_INTERVAL_SECONDS: i64 = 30;

const DATA_PRUNE_INTERVAL_SECONDS: i64 = 3_600;

/// Próximo instante de cada tarefa periódica do ciclo.
///
/// É memória de processo, e em produção o processo é um só: o `scheduler_loop`
/// chama `run_cycle` em laço (ADR 007). Por isso as cadências abaixo — VPN a
/// cada 10 s, tráfego a cada 30 s, purga a cada hora — valem de verdade, em vez
/// de rodarem a cada tique.
///
/// Rodando `task scheduler_run` à mão, o processo morre no fim do ciclo e o
/// mapa nasce vazio: um disparo avulso executa tudo. É o comportamento que se
/// espera de um comando manual.
fn next_run_at() -> &'static Mutex<HashMap<&'static str, DateTime<Utc>>> {
    static NEXT: OnceLock<Mutex<HashMap<&'static str, DateTime<Utc>>>> = OnceLock::new();
    NEXT.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `true` quando a tarefa venceu; já reserva o próximo horário.
fn is_due(key: &'static str, interval_seconds: i64, now: DateTime<Utc>) -> bool {
    let Ok(mut next) = next_run_at().lock() else {
        return true;
    };
    match next.get(key) {
        Some(scheduled) if now < *scheduled => false,
        _ => {
            next.insert(key, now + chrono::Duration::seconds(interval_seconds));
            true
        }
    }
}

/// Status a cada 10 s, histórico a cada 30 s.
///
/// O ciclo com histórico já sincroniza o status; roda primeiro para não gravar
/// duas amostras de tráfego separadas por poucos segundos.
async fn sync_vpn_traffic_if_due(ctx: &AppContext) -> AppResult<()> {
    let now = Utc::now();
    if is_due("vpn_traffic", VPN_TRAFFIC_INTERVAL_SECONDS, now) {
        traffic_recorder::record_all(ctx).await?;
        // O histórico já cobriu o status: adia o próximo ciclo curto.
        is_due("vpn_status", VPN_STATUS_INTERVAL_SECONDS, now);
        return Ok(());
    }
    if is_due("vpn_status", VPN_STATUS_INTERVAL_SECONDS, now) {
        traffic_recorder::sync_all(ctx).await?;
    }
    Ok(())
}

async fn run_data_pruner_if_due(ctx: &AppContext) -> AppResult<()> {
    if !is_due("data_pruner", DATA_PRUNE_INTERVAL_SECONDS, Utc::now()) {
        return Ok(());
    }
    let stats = data_pruner::prune_all(&ctx.db).await?;
    if stats.total() > 0 {
        tracing::info!(
            outbox = stats.outbox_deleted,
            resultados = stats.results_deleted,
            metricas = stats.metrics_deleted,
            descoberta = stats.discovery_deleted,
            "purga de dados antigos executada"
        );
    }
    Ok(())
}

/// Despacho de um monitor, na ordem portada integralmente do backend anterior
/// (§9.2): probe vivo → tarefa remota; probe offline → tentativa local; nem
/// isso → observação `unknown`.
async fn execute_one(ctx: &AppContext, monitor: &monitors::Model) -> AppResult<()> {
    let Some(_guard) = try_acquire_monitor(monitor.id) else {
        tracing::debug!(
            monitor_id = monitor.id,
            "monitor já em execução; ciclo ignorado"
        );
        return Ok(());
    };
    let timeout_ms = u64::from(monitor.timeout_seconds.max(1) as u32) * 1_000;

    if let Some(probe_id) = monitor.probe_id {
        let probe = probes::Entity::find_by_id(probe_id).one(&ctx.db).await?;
        if is_probe_alive(probe.as_ref()) {
            let task = ProbeTask {
                id: dispatcher::task_id(monitor.id, Utc::now()),
                monitor_id: monitor.id,
                task_type: monitor.r#type.clone(),
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                timeout_ms: timeout_ms as i32,
                payload: monitor.configuration.clone(),
            };
            dispatcher::dispatch_task(&ctx.db, probe_id, &task).await?;
            return Ok(());
        }

        // Probe offline: tenta a rota local antes de desistir. Se o servidor
        // tiver rota até o alvo, a checagem acontece em vez de congelar em
        // UNKNOWN. **Não remover** — diretriz do AGENTS.md §6.
        let local = run_monitor(
            ctx,
            &monitor.r#type,
            &monitor.configuration,
            RunOptions {
                timeout_ms: Some(timeout_ms),
            },
        )
        .await;
        if let Ok(result) = local {
            if result.success {
                process_result(ctx, monitor.id, &result, monitor.probe_id).await?;
                return Ok(());
            }
        }

        let label = probe
            .map(|probe| probe.name)
            .unwrap_or_else(|| format!("#{probe_id}"));
        return report_probe_unavailable(ctx, monitor, &label).await;
    }

    let result = match run_monitor(
        ctx,
        &monitor.r#type,
        &monitor.configuration,
        RunOptions {
            timeout_ms: Some(timeout_ms),
        },
    )
    .await
    {
        Ok(result) => result,
        Err(error) => unavailable_result(&error.to_string()),
    };
    process_result(ctx, monitor.id, &result, monitor.probe_id).await?;
    Ok(())
}

/// Registra a impossibilidade de medir como resultado `unknown`.
///
/// Não é `down`: o alvo pode estar perfeitamente no ar — quem sumiu foi o
/// agente. Mas a checagem precisa deixar rastro no histórico, senão o operador
/// vê apenas um monitor parado e sem motivo aparente (matriz de paridade #7).
async fn report_probe_unavailable(
    ctx: &AppContext,
    monitor: &monitors::Model,
    probe_label: &str,
) -> AppResult<()> {
    tracing::warn!(
        monitor_id = monitor.id,
        probe = probe_label,
        "monitor não executado: probe sem heartbeat"
    );
    let now = Utc::now();
    let result = CheckResult {
        success: false,
        status: MonitorStatus::Unknown,
        started_at: now,
        finished_at: now,
        duration_ms: 0,
        message: Some(format!(
            "Probe {probe_label} está sem heartbeat — a checagem não pôde ser executada."
        )),
        metrics: Vec::new(),
        data: serde_json::json!({ "probeId": monitor.probe_id, "reason": "probe_offline" }),
    };
    process_result(ctx, monitor.id, &result, monitor.probe_id).await?;
    Ok(())
}

fn unavailable_result(message: &str) -> CheckResult {
    let now = Utc::now();
    CheckResult {
        success: false,
        status: MonitorStatus::Unknown,
        started_at: now,
        finished_at: now,
        duration_ms: 0,
        message: Some(format!(
            "A checagem não pôde ser executada localmente: {message}"
        )),
        metrics: Vec::new(),
        data: serde_json::json!({"reason":"local_fallback_unavailable"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indisponibilidade_local_nao_vira_down() {
        assert_eq!(unavailable_result("x").status, MonitorStatus::Unknown);
    }
}
