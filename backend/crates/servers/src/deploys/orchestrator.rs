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

use agent_protocol::{
    docker::stacks::validate_commit, docker::stacks::validate_image_tag, policy::Permission,
};
use netmonitor_core::{
    agents::hub::AgentHub,
    models::{deploy_applications, deploy_runs, deploy_targets},
    shared::errors::{AppError, AppResult},
};

use super::{
    applications::{self, rollout_of},
    env_versions, events, executor,
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

/// Marca como substituídos os pendentes do alvo.
async fn supersede(ctx: &AppContext, target_id: i64) -> AppResult<()> {
    for run in runs::with_status(&ctx.db, &RunStatus::PENDING).await? {
        if run.target_id != target_id {
            continue;
        }
        if runs::transition(
            &ctx.db,
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
            events::publish_run(ctx, run.id).await;
        }
    }
    Ok(())
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
    supersede(ctx, ask.target.id).await?;
    let (status, scheduled_for) =
        initial_status(ask.target, ask.approver.is_some(), ask.when, Local::now());
    let run = runs::insert(
        &ctx.db,
        NewRun {
            application_id: ask.application.id,
            target_id: ask.target.id,
            trigger: ask.trigger,
            triggered_by: ask.triggered_by,
            reference: ask.revision.reference.clone(),
            commit_sha: ask.revision.commit_sha.clone(),
            commit_message: ask.revision.commit_message.clone(),
            image_tag: ask.revision.image_tag.clone(),
            env_version_id: ask.env_version_id,
            status,
            rollback_of_run_id: ask.rollback_of_run_id,
            after_run_id: ask.after_run_id,
            approved_by: ask.approver,
            scheduled_for,
        },
    )
    .await?;
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

/// "Publicar": uma versão escolhida (ou a do ar de novo, com o `.env` atual).
///
/// # Errors
///
/// `404`; `Validation` sem versão no ar e sem commit, ou commit/tag inválidos.
pub async fn request_manual(
    ctx: &AppContext,
    target_id: i64,
    input: &DeployInput,
    user: &str,
) -> AppResult<deploy_runs::Model> {
    let target = applications::find_target(&ctx.db, target_id).await?;
    let application = applications::find(&ctx.db, target.application_id).await?;
    let commit = input
        .commit_sha
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let revision = match commit {
        Some(commit) => {
            let commit = commit.to_lowercase();
            if commit.len() != 40 {
                return Err(AppError::validation(
                    "Informe o SHA completo do commit (40 caracteres)",
                ));
            }
            validate_commit(&commit).map_err(|error| AppError::validation(error.to_string()))?;
            let known = runs::recent(&ctx.db, Some(target_id), 200)
                .await?
                .into_iter()
                .find(|run| run.commit_sha == commit);
            Revision {
                image_tag: input
                    .image_tag
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
                    .or_else(|| known.as_ref().map(|run| run.image_tag.clone()))
                    .unwrap_or_else(|| commit.clone()),
                commit_message: known.as_ref().and_then(|run| run.commit_message.clone()),
                reference: known.and_then(|run| run.reference),
                commit_sha: commit,
            }
        }
        None => {
            let live = runs::live(&ctx.db, target_id).await?.ok_or_else(|| {
                AppError::validation(
                    "Este alvo ainda não tem versão publicada: informe o commit a publicar",
                )
            })?;
            Revision {
                commit_sha: live.commit_sha,
                commit_message: live.commit_message,
                reference: live.reference,
                image_tag: live.image_tag,
            }
        }
    };
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
            approver: Some(user.to_string()),
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
    };
    create(
        ctx,
        Ask {
            application: &application,
            target: &target,
            revision: &revision,
            trigger: RunTrigger::Rollback,
            triggered_by: Some(user.to_string()),
            approver: Some(user.to_string()),
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

/// Sobe o despachante. Fora do ambiente de teste.
pub fn spawn(ctx: &AppContext) {
    if ctx.environment == loco_rs::environment::Environment::Test {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(error) = recover_interrupted(&ctx).await {
            tracing::warn!(%error, "não foi possível encerrar os deploys interrompidos");
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
    });
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

/// Desde quando o deploy está na fila: a aprovação, a abertura da janela ou
/// a criação, o que for mais recente.
fn queued_since(run: &deploy_runs::Model) -> DateTime<Utc> {
    [Some(run.created_at), run.approved_at, run.scheduled_for]
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
    for run in runs::with_status(&ctx.db, &[RunStatus::Scheduled]).await? {
        let due = run
            .scheduled_for
            .is_none_or(|at| at.with_timezone(&Utc) <= now);
        if due
            && runs::transition(
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
        let Ok(target) = applications::find_target(&ctx.db, run.target_id).await else {
            continue;
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
        if let Some(after) = run.after_run_id {
            match runs::find(&ctx.db, after)
                .await
                .map(|run| runs::status_of(&run))
            {
                Ok(RunStatus::Succeeded) | Err(_) => {}
                Ok(status) if !status.is_terminal() => continue,
                Ok(_) => {
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
            if now - queued_since(&run) > AGENT_PATIENCE {
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
