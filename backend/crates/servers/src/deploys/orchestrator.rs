//! Pedir, aprovar, agendar e despachar deploys.
//!
//! ## O caminho de um deploy
//!
//! ```text
//! webhook ─┬─(alvo pede aprovação)─▶ awaiting_approval ─(aprovar)─┐
//!          └──────────────────────────────────────────────────────┤
//! manual / rollback (já aprovado por quem pediu) ─────────────────┤
//!                                                                 ▼
//!                          fora da janela ─▶ scheduled ─(abre)─▶ queued
//!                                                                 │ agente conectado,
//!                                                                 │ alvo livre, canário ok
//!                                                                 ▼
//!                                                              running ─▶ succeeded
//!                                                                      ├▶ rolled_back
//!                                                                      └▶ failed
//! ```
//!
//! Um deploy novo de um alvo **substitui** os pendentes dele (`superseded`):
//! não faz sentido aprovar a versão de ontem quando a de hoje já chegou.
//!
//! O despachante é um laço só, no processo da API — é ele que tem os canais
//! dos agentes. Quem cria um deploy na fila o acorda na hora; o laço também
//! roda sozinho a cada [`TICK`] para as janelas que abrem e os agentes que
//! voltam.

use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use loco_rs::app::AppContext;
use tokio::sync::Notify;

use agent_protocol::{docker::stacks::validate_image_tag, policy::Permission};
use netmonitor_core::{
    agents::hub::AgentHub,
    models::{deploy_applications, deploy_runs, deploy_targets},
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};

use super::{
    applications::{self, rollout_of},
    builds, env_versions, events, executor, next_revision,
    notify::{self, Moment},
    runs::{self, Change, NewRun},
    window::Window,
    Rollout, RunStatus, RunTrigger,
};
use crate::dtos::deploys::{DeployInput, DeployWhen};

/// Intervalo do laço quando ninguém o acorda.
pub const TICK: Duration = Duration::from_secs(15);
/// Quanto um deploy espera o agente voltar antes de falhar.
pub const AGENT_PATIENCE: chrono::Duration = chrono::Duration::minutes(10);

static WAKE: Notify = Notify::const_new();

/// Acorda o despachante agora (um deploy entrou na fila).
pub fn kick() {
    WAKE.notify_one();
}

/// A versão a publicar.
#[derive(Debug, Clone)]
pub struct Revision {
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub reference: Option<String>,
    pub image_tag: String,
    /// A compilação no servidor que produziu a imagem (ADR 023).
    pub build_id: Option<i64>,
}

/// Estado inicial de um deploy — puro.
///
/// `approved` diz se quem pediu já é a aprovação (um administrador, a IA
/// confirmada); `when` é o "agora ou na janela" dele. O webhook respeita a
/// janela.
#[must_use]
pub fn initial_status(
    target: &deploy_targets::Model,
    approved: bool,
    when: DeployWhen,
    now: DateTime<Local>,
) -> (RunStatus, Option<DateTime<Utc>>) {
    if !approved && target.requires_approval {
        return (RunStatus::AwaitingApproval, None);
    }
    scheduled_status(target, when, now)
}

fn scheduled_status(
    target: &deploy_targets::Model,
    when: DeployWhen,
    now: DateTime<Local>,
) -> (RunStatus, Option<DateTime<Utc>>) {
    if when == DeployWhen::Now {
        return (RunStatus::Queued, None);
    }
    match Window::from_columns(target.window_start.as_deref(), target.window_end.as_deref()) {
        Ok(Some(window)) if !window.contains(now.time()) => (
            RunStatus::Scheduled,
            Some(window.next_opening(now).with_timezone(&Utc)),
        ),
        _ => (RunStatus::Queued, None),
    }
}

struct Ask<'a> {
    application: &'a deploy_applications::Model,
    target: &'a deploy_targets::Model,
    revision: &'a Revision,
    trigger: RunTrigger,
    triggered_by: Option<String>,
    /// Quem aprovou ao pedir (manual, rollback, IA confirmada).
    approver: Option<String>,
    when: DeployWhen,
    env_version_id: Option<i64>,
    rollback_of_run_id: Option<i64>,
    after_run_id: Option<i64>,
}

async fn create(ctx: &AppContext, ask: Ask<'_>) -> AppResult<deploy_runs::Model> {
    let txn = begin_write(&ctx.db).await?;
    let mut superseded = Vec::new();
    for run in runs::pending_for_target(&txn, ask.target.id).await? {
        if runs::transition_in(
            &txn,
            run.id,
            &RunStatus::PENDING,
            RunStatus::Superseded,
            Change {
                error: Some(Some("Uma versão mais nova chegou antes desta rodar".into())),
                finished: true,
                ..Change::default()
            },
        )
        .await?
        {
            superseded.push(run.id);
        }
    }
    let (status, scheduled_for) =
        initial_status(ask.target, ask.approver.is_some(), ask.when, Local::now());
    let run = runs::insert(
        &txn,
        NewRun {
            application_id: ask.application.id,
            target_id: ask.target.id,
            trigger: ask.trigger,
            triggered_by: ask.triggered_by,
            reference: ask.revision.reference.clone(),
            commit_sha: ask.revision.commit_sha.clone(),
            commit_message: ask.revision.commit_message.clone(),
            image_tag: ask.revision.image_tag.clone(),
            build_id: ask.revision.build_id,
            env_version_id: ask.env_version_id,
            status,
            rollback_of_run_id: ask.rollback_of_run_id,
            after_run_id: ask.after_run_id,
            approved_by: ask.approver,
            scheduled_for,
        },
    )
    .await?;
    txn.commit().await?;
    for id in superseded {
        events::publish_run(ctx, id).await;
    }
    events::publish_run(ctx, run.id).await;
    match status {
        RunStatus::AwaitingApproval => {
            let view = events::view(&ctx.db, &run).await?;
            notify::send(ctx, Moment::AwaitingApproval, &view).await;
        }
        RunStatus::Queued => kick(),
        _ => {}
    }
    Ok(run)
}

async fn current_env(ctx: &AppContext, target_id: i64) -> AppResult<Option<i64>> {
    Ok(env_versions::current(&ctx.db, target_id)
        .await?
        .map(|version| version.id))
}

/// Os deploys de um push: um por alvo, na ordem dos alvos. Na estratégia
/// sequencial, cada um espera o anterior (canário).
///
/// # Errors
///
/// Banco.
pub async fn request_from_hook(
    ctx: &AppContext,
    application: &deploy_applications::Model,
    targets: &[deploy_targets::Model],
    revision: &Revision,
    actor: Option<String>,
) -> AppResult<Vec<deploy_runs::Model>> {
    let sequential = rollout_of(application) == Rollout::Sequential;
    let mut created = Vec::with_capacity(targets.len());
    let mut previous: Option<i64> = None;
    for target in targets {
        let run = create(
            ctx,
            Ask {
                application,
                target,
                revision,
                trigger: RunTrigger::Webhook,
                triggered_by: actor.clone(),
                approver: None,
                when: DeployWhen::Window,
                env_version_id: current_env(ctx, target.id).await?,
                rollback_of_run_id: None,
                after_run_id: if sequential { previous } else { None },
            },
        )
        .await?;
        previous = Some(run.id);
        created.push(run);
    }
    Ok(created)
}

/// "Publicar": uma versão escolhida, a do ar de novo (com o `.env` atual) ou,
/// num alvo nunca publicado, o commit mais novo da branch
/// ([`next_revision::resolve`]).
///
/// # Errors
///
/// `404`; `Validation` com commit/tag inválidos ou quando o commit vem vazio
/// e não há versão no ar nem como descobrir o mais novo.
pub async fn request_manual(
    ctx: &AppContext,
    target_id: i64,
    input: &DeployInput,
    user: &str,
) -> AppResult<deploy_runs::Model> {
    request_manual_with_approval_policy(ctx, target_id, input, user, true).await
}

/// "Publicar" com política explícita de aprovação: quando `auto_approve` é
/// `false` (ex.: chamada via MCP), alvos com `requires_approval = true` entram
/// em `awaiting_approval` em vez de pular a aprovação humana.
///
/// # Errors
///
/// Como [`request_manual`].
pub async fn request_manual_with_approval_policy(
    ctx: &AppContext,
    target_id: i64,
    input: &DeployInput,
    user: &str,
    auto_approve: bool,
) -> AppResult<deploy_runs::Model> {
    let target = applications::find_target(&ctx.db, target_id).await?;
    let application = applications::find(&ctx.db, target.application_id).await?;
    let db = ctx.db.clone();
    let owner = application.clone();
    let revision = next_revision::resolve(
        &ctx.db,
        &application,
        &target,
        input,
        move |branch| async move { builds::head_of(&db, &owner, &branch).await },
    )
    .await?;
    validate_image_tag(&revision.image_tag)
        .map_err(|error| AppError::validation(error.to_string()))?;
    create(
        ctx,
        Ask {
            application: &application,
            target: &target,
            revision: &revision,
            trigger: RunTrigger::Manual,
            triggered_by: Some(user.to_string()),
            approver: auto_approve.then(|| user.to_string()),
            when: input.when,
            env_version_id: current_env(ctx, target.id).await?,
            rollback_of_run_id: None,
            after_run_id: None,
        },
    )
    .await
}

/// "Restaurar esta versão": a imagem **e** o `.env` de um deploy que deu
/// certo.
///
/// # Errors
///
/// `404`; `Validation` para deploy que não terminou bem.
pub async fn request_restore(
    ctx: &AppContext,
    run_id: i64,
    when: DeployWhen,
    user: &str,
) -> AppResult<deploy_runs::Model> {
    request_restore_with_approval_policy(ctx, run_id, when, user, true).await
}

/// "Restaurar esta versão" com política explícita de aprovação.
///
/// # Errors
///
/// `404`; `Validation` para deploy que não terminou bem.
pub async fn request_restore_with_approval_policy(
    ctx: &AppContext,
    run_id: i64,
    when: DeployWhen,
    user: &str,
    auto_approve: bool,
) -> AppResult<deploy_runs::Model> {
    let source = runs::find(&ctx.db, run_id).await?;
    if runs::status_of(&source) != RunStatus::Succeeded {
        return Err(AppError::validation(
            "Só uma versão que subiu com sucesso pode ser restaurada",
        ));
    }
    let target = applications::find_target(&ctx.db, source.target_id).await?;
    let application = applications::find(&ctx.db, target.application_id).await?;
    let revision = Revision {
        commit_sha: source.commit_sha.clone(),
        commit_message: source.commit_message.clone(),
        reference: source.reference.clone(),
        image_tag: source.image_tag.clone(),
        build_id: source.build_id,
    };
    create(
        ctx,
        Ask {
            application: &application,
            target: &target,
            revision: &revision,
            trigger: RunTrigger::Rollback,
            triggered_by: Some(user.to_string()),
            approver: auto_approve.then(|| user.to_string()),
            when,
            env_version_id: source.env_version_id,
            rollback_of_run_id: Some(source.id),
            after_run_id: None,
        },
    )
    .await
}

async fn settled(
    ctx: &AppContext,
    run_id: i64,
    changed: bool,
    refusal: &str,
) -> AppResult<deploy_runs::Model> {
    if !changed {
        return Err(AppError::conflict(refusal.to_string()));
    }
    events::publish_run(ctx, run_id).await;
    runs::find(&ctx.db, run_id).await
}

/// Aprova um deploy pendente: agora ou na janela do alvo.
///
/// # Errors
///
/// `404`; `409` quando ele já não espera aprovação.
pub async fn approve(
    ctx: &AppContext,
    run_id: i64,
    when: DeployWhen,
    user: &str,
) -> AppResult<deploy_runs::Model> {
    let run = runs::find(&ctx.db, run_id).await?;
    let target = applications::find_target(&ctx.db, run.target_id).await?;
    let (status, scheduled_for) = scheduled_status(&target, when, Local::now());
    let changed = runs::transition(
        &ctx.db,
        run_id,
        &[RunStatus::AwaitingApproval],
        status,
        Change {
            approved_by: Some(user.to_string()),
            scheduled_for: Some(scheduled_for),
            ..Change::default()
        },
    )
    .await?;
    let run = settled(
        ctx,
        run_id,
        changed,
        "Este deploy não está mais aguardando aprovação",
    )
    .await?;
    if status == RunStatus::Queued {
        kick();
    }
    Ok(run)
}

/// # Errors
///
/// `404`; `409` quando ele já não espera aprovação.
pub async fn reject(
    ctx: &AppContext,
    run_id: i64,
    user: &str,
    reason: Option<&str>,
) -> AppResult<deploy_runs::Model> {
    runs::find(&ctx.db, run_id).await?;
    let reason = reason
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map_or_else(
            || format!("Recusado por {user}"),
            |reason| format!("Recusado por {user}: {reason}"),
        );
    let changed = runs::transition(
        &ctx.db,
        run_id,
        &[RunStatus::AwaitingApproval],
        RunStatus::Rejected,
        Change {
            error: Some(Some(reason)),
            finished: true,
            ..Change::default()
        },
    )
    .await?;
    settled(
        ctx,
        run_id,
        changed,
        "Este deploy não está mais aguardando aprovação",
    )
    .await
}

/// Cancela um deploy que ainda não começou. Um deploy rodando não é
/// interrompido: parar no meio deixaria a stack pela metade.
///
/// # Errors
///
/// `404`; `409` quando já começou ou terminou.
pub async fn cancel(ctx: &AppContext, run_id: i64, user: &str) -> AppResult<deploy_runs::Model> {
    runs::find(&ctx.db, run_id).await?;
    let changed = runs::transition(
        &ctx.db,
        run_id,
        &RunStatus::PENDING,
        RunStatus::Cancelled,
        Change {
            error: Some(Some(format!("Cancelado por {user}"))),
            finished: true,
            ..Change::default()
        },
    )
    .await?;
    settled(
        ctx,
        run_id,
        changed,
        "Só um deploy que ainda não começou pode ser cancelado",
    )
    .await
}

/// O despachante, para sempre. Rotina do processo líder (ADR 024), subida
/// pelo ponto de montagem fora do ambiente de teste: a recuperação do começo
/// roda a cada vez que este processo assume.
pub async fn run(ctx: AppContext) {
    if let Err(error) = recover_interrupted(&ctx).await {
        tracing::warn!(%error, "não foi possível encerrar os deploys interrompidos");
    }
    if let Err(error) = super::builds::recover(&ctx).await {
        tracing::warn!(%error, "não foi possível retomar as compilações interrompidas");
    }
    loop {
        if let Err(error) = dispatch(&ctx, Utc::now()).await {
            tracing::warn!(%error, "ciclo do despachante de deploys falhou");
        }
        tokio::select! {
            () = WAKE.notified() => {}
            () = tokio::time::sleep(TICK) => {}
        }
    }
}

/// Deploys que estavam rodando quando o processo caiu não têm mais quem os
/// acompanhe: viram falha, com o motivo.
async fn recover_interrupted(ctx: &AppContext) -> AppResult<()> {
    for run in runs::with_status(&ctx.db, &[RunStatus::Running]).await? {
        if runs::transition(
            &ctx.db,
            run.id,
            &[RunStatus::Running],
            RunStatus::Failed,
            Change {
                error: Some(Some(
                    "A central reiniciou durante o deploy: confira o estado da stack e publique de \
                     novo"
                        .into(),
                )),
                finished: true,
                ..Change::default()
            },
        )
        .await?
        {
            events::publish_run(ctx, run.id).await;
        }
    }
    Ok(())
}

/// Desde quando o deploy está pronto na fila: a aprovação, a abertura da
/// janela, o fim do alvo anterior na sequência ou a criação — o que for mais
/// recente.
fn queued_since(
    run: &deploy_runs::Model,
    upstream_finished: Option<sea_orm::prelude::DateTimeWithTimeZone>,
) -> DateTime<Utc> {
    [
        Some(run.created_at),
        run.approved_at,
        run.scheduled_for,
        upstream_finished,
    ]
    .into_iter()
    .flatten()
    .map(|at| at.with_timezone(&Utc))
    .max()
    .unwrap_or_else(Utc::now)
}

async fn fail(ctx: &AppContext, run_id: i64, to: RunStatus, message: String) -> AppResult<()> {
    if runs::transition(
        &ctx.db,
        run_id,
        &[RunStatus::Queued],
        to,
        Change {
            error: Some(Some(message)),
            finished: true,
            ..Change::default()
        },
    )
    .await?
    {
        events::publish_run(ctx, run_id).await;
        if to == RunStatus::Failed {
            if let Ok(run) = runs::find(&ctx.db, run_id).await {
                if let Ok(view) = events::view(&ctx.db, &run).await {
                    notify::send(ctx, Moment::Failed, &view).await;
                }
            }
        }
    }
    Ok(())
}

/// Uma passada: janelas que abriram viram fila; da fila, começa quem pode.
///
/// # Errors
///
/// Banco.
pub async fn dispatch(ctx: &AppContext, now: DateTime<Utc>) -> AppResult<usize> {
    // Cancela pendentes (`awaiting_approval`/`scheduled`) cujo predecessor na
    // sequência já terminou sem sucesso, evitando aprovações ou agendamentos órfãos.
    let pending = runs::with_status(
        &ctx.db,
        &[RunStatus::AwaitingApproval, RunStatus::Scheduled],
    )
    .await?;
    // Os predecessores numa consulta só, sem o log (até 256 KiB cada).
    let upstream_ids: Vec<i64> = pending.iter().filter_map(|run| run.after_run_id).collect();
    let upstreams = runs::find_many_without_log(&ctx.db, &upstream_ids).await?;
    for run in pending {
        let Some(after) = run.after_run_id else {
            continue;
        };
        // O predecessor sumiu: nada a encadear.
        if let Some(upstream) = upstreams.get(&after) {
            let status = runs::status_of(upstream);
            if status.is_terminal()
                && status != RunStatus::Succeeded
                && runs::transition(
                    &ctx.db,
                    run.id,
                    &[RunStatus::AwaitingApproval, RunStatus::Scheduled],
                    RunStatus::Cancelled,
                    Change {
                        error: Some(Some(
                            "O alvo anterior da sequência não subiu: este ficou de fora para \
                                 não espalhar a falha"
                                .into(),
                        )),
                        finished: true,
                        ..Change::default()
                    },
                )
                .await?
            {
                events::publish_run(ctx, run.id).await;
            }
        }
    }

    let local_now = now.with_timezone(&Local);
    for run in runs::with_status(&ctx.db, &[RunStatus::Scheduled]).await? {
        let due = run
            .scheduled_for
            .is_none_or(|at| at.with_timezone(&Utc) <= now);
        if !due {
            continue;
        }
        // Se a central ficou fora e passou do fim da janela de manutenção,
        // reagenda para a próxima abertura em vez de disparar fora da janela.
        if let Ok(target) = applications::find_target(&ctx.db, run.target_id).await {
            if let Ok(Some(window)) =
                Window::from_columns(target.window_start.as_deref(), target.window_end.as_deref())
            {
                if !window.contains(local_now.time()) {
                    let next = window.next_opening(local_now).with_timezone(&Utc);
                    if runs::transition(
                        &ctx.db,
                        run.id,
                        &[RunStatus::Scheduled],
                        RunStatus::Scheduled,
                        Change {
                            scheduled_for: Some(Some(next)),
                            ..Change::default()
                        },
                    )
                    .await?
                    {
                        events::publish_run(ctx, run.id).await;
                    }
                    continue;
                }
            }
        }
        if runs::transition(
            &ctx.db,
            run.id,
            &[RunStatus::Scheduled],
            RunStatus::Queued,
            Change::default(),
        )
        .await?
        {
            events::publish_run(ctx, run.id).await;
        }
    }

    let hub = AgentHub::from_context(ctx).ok();
    let mut started = 0;
    for run in runs::with_status(&ctx.db, &[RunStatus::Queued]).await? {
        let target = match applications::find_target(&ctx.db, run.target_id).await {
            Ok(target) => target,
            Err(AppError::NotFound(_)) => continue,
            Err(error) => return Err(error),
        };
        if !target.enabled {
            fail(
                ctx,
                run.id,
                RunStatus::Cancelled,
                "O alvo foi desativado".into(),
            )
            .await?;
            continue;
        }
        if runs::running_on(&ctx.db, target.id).await? {
            continue;
        }
        let mut upstream_finished = None;
        if let Some(after) = run.after_run_id {
            match runs::find_without_log(&ctx.db, after).await {
                Ok(upstream) => {
                    let status = runs::status_of(&upstream);
                    if status == RunStatus::Succeeded {
                        upstream_finished = upstream.finished_at;
                    } else if !status.is_terminal() {
                        continue;
                    } else {
                        fail(
                            ctx,
                            run.id,
                            RunStatus::Cancelled,
                            "O alvo anterior da sequência não subiu: este ficou de fora para não \
                             espalhar a falha"
                                .into(),
                        )
                        .await?;
                        continue;
                    }
                }
                Err(AppError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        let Some(probe_id) = target.probe_id else {
            fail(
                ctx,
                run.id,
                RunStatus::Failed,
                "O agente deste alvo foi removido: edite o alvo e escolha outro".into(),
            )
            .await?;
            continue;
        };
        let Some(session) = hub.as_ref().and_then(|hub| hub.get(probe_id)) else {
            if now - queued_since(&run, upstream_finished) > AGENT_PATIENCE {
                fail(
                    ctx,
                    run.id,
                    RunStatus::Failed,
                    format!(
                        "O agente ficou desconectado por mais de {} minutos: publique de novo \
                         quando ele voltar",
                        AGENT_PATIENCE.num_minutes()
                    ),
                )
                .await?;
            }
            continue;
        };
        if !session.allows(Permission::Deploy) {
            fail(
                ctx,
                run.id,
                RunStatus::Failed,
                "A política do agente não libera deploy: acrescente `deploy` ao AGENT_ALLOW \
                 do servidor"
                    .into(),
            )
            .await?;
            continue;
        }
        if runs::transition(
            &ctx.db,
            run.id,
            &[RunStatus::Queued],
            RunStatus::Running,
            Change {
                started: true,
                ..Change::default()
            },
        )
        .await?
        {
            events::publish_run(ctx, run.id).await;
            started += 1;
            let ctx = ctx.clone();
            tokio::spawn(async move { executor::execute(ctx, run.id).await });
        }
    }
    Ok(started)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn target(requires_approval: bool, window: Option<(&str, &str)>) -> deploy_targets::Model {
        let now = Utc::now().into();
        deploy_targets::Model {
            id: 1,
            application_id: 1,
            name: "Produção".into(),
            probe_id: Some(1),
            project: "sgc".into(),
            reference: "main".into(),
            requires_approval,
            window_start: window.map(|(start, _)| start.to_string()),
            window_end: window.map(|(_, end)| end.to_string()),
            backup_connection_id: None,
            wait_timeout_secs: 180,
            auto_rollback: true,
            keep_images: 3,
            enabled: true,
            position: 0,
            vulnerability_policy: "warn".into(),
            require_provenance: false,
            created_at: now,
            updated_at: now,
        }
    }

    fn at(hour: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 11, 8, hour, 0, 0)
            .single()
            .expect("instante")
    }

    #[test]
    fn producao_espera_aprovacao_e_homologacao_entra_na_fila() {
        let (status, _) = initial_status(&target(true, None), false, DeployWhen::Window, at(10));
        assert_eq!(status, RunStatus::AwaitingApproval);
        let (status, _) = initial_status(&target(false, None), false, DeployWhen::Window, at(10));
        assert_eq!(status, RunStatus::Queued);
    }

    #[test]
    fn quem_aprova_ao_pedir_pula_a_aprovacao() {
        let (status, _) = initial_status(&target(true, None), true, DeployWhen::Now, at(10));
        assert_eq!(status, RunStatus::Queued);
    }

    #[test]
    fn fora_da_janela_fica_agendado_e_agora_ignora_a_janela() {
        let night = target(false, Some(("23:00", "05:00")));
        let (status, scheduled) = initial_status(&night, false, DeployWhen::Window, at(14));
        assert_eq!(status, RunStatus::Scheduled);
        assert_eq!(scheduled.map(|at| at.with_timezone(&Local)), Some(at(23)));
        let (status, _) = initial_status(&night, true, DeployWhen::Now, at(14));
        assert_eq!(status, RunStatus::Queued);
        let (status, _) = initial_status(&night, false, DeployWhen::Window, at(23));
        assert_eq!(status, RunStatus::Queued);
    }
}
