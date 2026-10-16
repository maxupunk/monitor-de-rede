//! Uma ação de plugin chamada por outro serviço, sem HTTP — o orquestrador do
//! teste de enlace (ADR 015) pede `tp_serve` num equipamento, `tp_run` no
//! outro e `tp_stop` no fim.
//!
//! É a mesma execução da tela: linha em `plugin_runs` (o transcript fica a um
//! clique), auditoria, SSE e as mesmas portas de efeito e credencial. O que
//! muda é quem espera o resultado e quem recebe as amostras de andamento.
//!
//! Só plugin **ativo** e **instalado** no equipamento roda por aqui: o
//! orquestrador não tem operador para aprovar acesso a acesso, então os
//! acessos são liberados como na tela ([`runs::approval_for`]).

use loco_rs::app::AppContext;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use super::{
    actions,
    runs::{self, Origin, Prepared, RunSpec},
    runtime::ProgressSample,
    service,
};
use crate::{
    models::plugin_runs,
    services::shared::errors::{AppError, AppResult},
};

/// O pedido: qual ação, de qual plugin (pelo `slug`), em qual equipamento.
pub struct ServiceRunRequest {
    pub device_id: i64,
    pub plugin_slug: String,
    pub action: String,
    /// Validados contra o esquema da ação, como na tela.
    pub params: Value,
    pub origin: Origin,
    /// Quem pediu o teste, para a auditoria.
    pub user_id: Option<i64>,
    /// Motivo registrado na auditoria ("teste de enlace #12, etapa serve").
    pub reason: Option<String>,
    /// Recebe cada amostra de andamento (`device.ssh_live` com `progress`,
    /// `device.progress`). Elas também vão ao SSE (`plugin:run_progress`).
    pub progress: Option<UnboundedSender<ProgressSample>>,
    /// Cancelar este token cancela a execução (e a chamada em andamento).
    pub cancel: Option<CancellationToken>,
}

/// Como a execução terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Succeeded,
    Failed,
    Cancelled,
}

impl RunStatus {
    fn from_row(status: &str) -> Self {
        match status {
            "succeeded" => Self::Succeeded,
            "cancelled" => Self::Cancelled,
            _ => Self::Failed,
        }
    }
}

/// O desfecho, já sem segredos (a saída passou pela máscara do runtime).
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub run_id: i64,
    pub status: RunStatus,
    pub output: Option<Value>,
    pub error: Option<String>,
}

impl RunOutcome {
    #[must_use]
    pub fn from_row(run: &plugin_runs::Model) -> Self {
        Self {
            run_id: run.id,
            status: RunStatus::from_row(&run.status),
            output: run.output.clone(),
            error: run.error.clone(),
        }
    }
}

/// Valida e cria a execução, sem rodar — o chamador já tem o `run_id`
/// (para gravar na etapa) antes de esperar o fim com [`finish`].
///
/// # Errors
///
/// Equipamento inexistente; nenhum plugin ativo com esse `slug` instalado
/// nele; ou o que [`runs::prepare`] recusar (ação, parâmetros, credencial,
/// agente fora).
pub async fn prepare(ctx: &AppContext, request: ServiceRunRequest) -> AppResult<Prepared> {
    let device = actions::device(ctx, request.device_id).await?;
    let plugin = service::installed_active(&ctx.db, Some(&[device.id]))
        .await?
        .into_iter()
        .map(|(_, plugin)| plugin)
        .filter(|plugin| plugin.slug == request.plugin_slug)
        // O exclusivo do equipamento vale mais que o de modelo.
        .max_by_key(|plugin| (plugin.device_id.is_some(), plugin.id))
        .ok_or_else(|| {
            AppError::business_rule(format!(
                "O plugin `{}` não está ativo e instalado em {}.",
                request.plugin_slug, device.name
            ))
        })?;
    let approval = runs::approval_for(&plugin, request.origin, false);
    let mut prepared = runs::prepare(
        ctx,
        RunSpec {
            plugin,
            device,
            action: request.action,
            params: request.params,
            origin: request.origin,
            user_id: request.user_id,
            reason: request.reason,
            approval,
            batch_id: None,
        },
    )
    .await?;
    if let Some(sender) = request.progress {
        prepared = prepared.with_progress(sender);
    }
    if let Some(cancel) = &request.cancel {
        prepared = prepared.cancelled_by(cancel);
    }
    Ok(prepared)
}

/// Roda o que [`prepare`] criou e espera o fim.
///
/// # Errors
///
/// Só erro do banco ao gravar o desfecho — falha da ação vem em
/// [`RunOutcome::status`].
pub async fn finish(ctx: &AppContext, prepared: Prepared) -> AppResult<RunOutcome> {
    Ok(RunOutcome::from_row(&runs::run(ctx, prepared).await?))
}

/// [`prepare`] e [`finish`] de uma vez.
///
/// # Errors
///
/// Os de [`prepare`] e [`finish`].
pub async fn execute(ctx: &AppContext, request: ServiceRunRequest) -> AppResult<RunOutcome> {
    finish(ctx, prepare(ctx, request).await?).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estado_da_linha_vira_desfecho() {
        assert_eq!(RunStatus::from_row("succeeded"), RunStatus::Succeeded);
        assert_eq!(RunStatus::from_row("cancelled"), RunStatus::Cancelled);
        assert_eq!(RunStatus::from_row("failed"), RunStatus::Failed);
        assert_eq!(RunStatus::from_row("running"), RunStatus::Failed);
    }
}
