//! Serialização do deploy contínuo para a tela.
//!
//! Segredo nenhum sai daqui: credencial só diz que tem token, versão do
//! `.env` só lista as chaves, e o deploy não carrega o log (que vem sob
//! demanda).

use serde::Serialize;
use ts_rs::TS;

use agent_protocol::docker::stacks::StackReadiness;
use netmonitor_core::models::{
    deploy_applications, deploy_credentials, deploy_env_versions, deploy_runs, deploy_targets,
};

use crate::{
    deploys::{
        applications::{compose_files_of, images_of, repository_url, rollout_of},
        env_versions::keys_of,
        runs::status_of,
        short_sha, DeployStage, Rollout, RunStatus, RunTrigger,
    },
    dtos::deploys::BuildImage,
};

fn timestamp(value: &sea_orm::prelude::DateTimeWithTimeZone) -> String {
    value.to_rfc3339()
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployCredentialView {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub registry: String,
    pub username: String,
    pub has_secret: bool,
    pub created_at: String,
}

impl From<&deploy_credentials::Model> for DeployCredentialView {
    fn from(row: &deploy_credentials::Model) -> Self {
        Self {
            id: row.id,
            name: row.name.clone(),
            registry: row.registry.clone(),
            username: row.username.clone(),
            has_secret: !row.secret_encrypted.is_empty(),
            created_at: timestamp(&row.created_at),
        }
    }
}

/// Um deploy, sem o log.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployRunView {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub application_id: i64,
    pub application_name: String,
    /// `dono/repo`, para os links do GitHub.
    pub repository: String,
    #[ts(type = "number")]
    pub target_id: i64,
    pub target_name: String,
    pub trigger: RunTrigger,
    pub triggered_by: Option<String>,
    pub reference: Option<String>,
    pub commit_sha: String,
    pub short_sha: String,
    pub commit_message: Option<String>,
    pub image_tag: String,
    #[ts(type = "number | null")]
    pub env_version_id: Option<i64>,
    pub status: RunStatus,
    pub stage: Option<DeployStage>,
    #[ts(type = "number | null")]
    pub rollback_of_run_id: Option<i64>,
    #[ts(type = "number | null")]
    pub after_run_id: Option<i64>,
    #[ts(type = "number | null")]
    pub backup_id: Option<i64>,
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
    pub scheduled_for: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<i64>,
}

impl DeployRunView {
    #[must_use]
    pub fn new(
        run: &deploy_runs::Model,
        application: Option<&deploy_applications::Model>,
        target: Option<&deploy_targets::Model>,
    ) -> Self {
        Self {
            id: run.id,
            application_id: run.application_id,
            application_name: application.map_or_else(String::new, |app| app.name.clone()),
            repository: application.map_or_else(String::new, |app| app.repository.clone()),
            target_id: run.target_id,
            target_name: target.map_or_else(String::new, |target| target.name.clone()),
            trigger: RunTrigger::parse(&run.trigger).unwrap_or(RunTrigger::Manual),
            triggered_by: run.triggered_by.clone(),
            reference: run.reference.clone(),
            commit_sha: run.commit_sha.clone(),
            short_sha: short_sha(&run.commit_sha).to_string(),
            commit_message: run.commit_message.clone(),
            image_tag: run.image_tag.clone(),
            env_version_id: run.env_version_id,
            status: status_of(run),
            stage: run.stage.as_deref().and_then(DeployStage::parse),
            rollback_of_run_id: run.rollback_of_run_id,
            after_run_id: run.after_run_id,
            backup_id: run.backup_id,
            approved_by: run.approved_by.clone(),
            approved_at: run.approved_at.as_ref().map(timestamp),
            scheduled_for: run.scheduled_for.as_ref().map(timestamp),
            error: run.error.clone(),
            created_at: timestamp(&run.created_at),
            started_at: run.started_at.as_ref().map(timestamp),
            finished_at: run.finished_at.as_ref().map(timestamp),
            duration_ms: run.duration_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployRunLogView {
    #[ts(type = "number")]
    pub id: i64,
    pub log: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployEnvVersionView {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub target_id: i64,
    pub keys: Vec<String>,
    pub created_by: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
    /// A versão em vigor (a mais nova).
    pub current: bool,
}

impl DeployEnvVersionView {
    #[must_use]
    pub fn new(row: &deploy_env_versions::Model, current: bool) -> Self {
        Self {
            id: row.id,
            target_id: row.target_id,
            keys: keys_of(row),
            created_by: row.created_by.clone(),
            note: row.note.clone(),
            created_at: timestamp(&row.created_at),
            current,
        }
    }
}

/// O conteúdo de uma versão — só para administrador, a pedido.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployEnvContentView {
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    pub content: String,
}

/// O agente de um alvo, como a tela precisa.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployAgentView {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub connected: bool,
    /// A política local libera `deploy` (desconectado conta como sim até
    /// o próximo `Hello`).
    pub allows_deploy: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployTargetView {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub application_id: i64,
    pub name: String,
    /// `None` quando o agente foi removido.
    pub agent: Option<DeployAgentView>,
    pub project: String,
    pub reference: String,
    pub requires_approval: bool,
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    #[ts(type = "number | null")]
    pub backup_connection_id: Option<i64>,
    pub backup_connection_name: Option<String>,
    pub wait_timeout_secs: i32,
    pub auto_rollback: bool,
    pub keep_images: i32,
    pub enabled: bool,
    pub position: i32,
    /// A versão no ar.
    pub live: Option<DeployRunView>,
    /// O deploy pendente ou em andamento mais recente.
    pub pending: Option<DeployRunView>,
    pub env: Option<DeployEnvVersionView>,
}

/// Peças que o alvo junta de outras tabelas.
pub struct TargetParts {
    pub agent: Option<DeployAgentView>,
    pub backup_connection_name: Option<String>,
    pub live: Option<DeployRunView>,
    pub pending: Option<DeployRunView>,
    pub env: Option<DeployEnvVersionView>,
}

impl DeployTargetView {
    #[must_use]
    pub fn new(row: &deploy_targets::Model, parts: TargetParts) -> Self {
        Self {
            id: row.id,
            application_id: row.application_id,
            name: row.name.clone(),
            agent: parts.agent,
            project: row.project.clone(),
            reference: row.reference.clone(),
            requires_approval: row.requires_approval,
            window_start: row.window_start.clone(),
            window_end: row.window_end.clone(),
            backup_connection_id: row.backup_connection_id,
            backup_connection_name: parts.backup_connection_name,
            wait_timeout_secs: row.wait_timeout_secs,
            auto_rollback: row.auto_rollback,
            keep_images: row.keep_images,
            enabled: row.enabled,
            position: row.position,
            live: parts.live,
            pending: parts.pending,
            env: parts.env,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployApplicationView {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub repository: String,
    pub repository_url: String,
    #[ts(type = "number | null")]
    pub credential_id: Option<i64>,
    pub credential_name: Option<String>,
    pub compose_files: Vec<String>,
    pub image_tag_variable: String,
    pub images: Vec<BuildImage>,
    pub rollout: Rollout,
    pub targets: Vec<DeployTargetView>,
    pub created_at: String,
}

impl DeployApplicationView {
    #[must_use]
    pub fn new(
        row: &deploy_applications::Model,
        credential_name: Option<String>,
        targets: Vec<DeployTargetView>,
    ) -> Self {
        Self {
            id: row.id,
            name: row.name.clone(),
            slug: row.slug.clone(),
            repository: row.repository.clone(),
            repository_url: repository_url(&row.repository),
            credential_id: row.credential_id,
            credential_name,
            compose_files: compose_files_of(row),
            image_tag_variable: row.image_tag_variable.clone(),
            images: images_of(row),
            rollout: rollout_of(row),
            targets,
            created_at: timestamp(&row.created_at),
        }
    }
}

/// Os números do topo da página.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployCounts {
    pub applications: u32,
    pub targets: u32,
    pub succeeded_24h: u32,
    pub failed_24h: u32,
    pub awaiting_approval: u32,
    pub running: u32,
}

/// A página inteira numa resposta: aplicações com alvos, deploys recentes e
/// os números.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployOverview {
    pub applications: Vec<DeployApplicationView>,
    pub recent_runs: Vec<DeployRunView>,
    pub counts: DeployCounts,
    pub credentials: Vec<DeployCredentialView>,
    /// O endereço que o workflow chama (`…/api/deploy-hooks/github`).
    pub hook_path: String,
    /// O `audience` que o token OIDC precisa ter.
    pub oidc_audience: String,
}

/// Um item da checagem de prontidão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployReadinessLevel"
)]
pub enum ReadinessLevel {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployReadinessCheck"
)]
pub struct ReadinessCheck {
    pub id: String,
    pub label: String,
    pub level: ReadinessLevel,
    pub detail: String,
    /// O que fazer, quando há o que fazer.
    pub fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployReadinessView {
    /// Nenhum item em erro: o primeiro deploy pode rodar.
    pub ready: bool,
    pub checks: Vec<ReadinessCheck>,
    pub stack: Option<StackReadiness>,
}

/// Um commit entre a versão no ar e a que vai entrar.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployCommitView {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author: Option<String>,
    pub date: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployChangesView {
    /// A versão no ar (base da comparação), se há.
    pub base_sha: Option<String>,
    pub head_sha: String,
    /// Link do "Compare" do GitHub.
    pub compare_url: Option<String>,
    pub commits: Vec<DeployCommitView>,
    /// Commits além dos listados (o GitHub corta em 250).
    pub total: u32,
    /// Por que a lista não veio (sem credencial, repositório privado…).
    pub unavailable_reason: Option<String>,
}

/// O workflow sugerido e o que configurar no GitHub.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployWorkflowView {
    pub path: String,
    pub yaml: String,
    /// A variável do repositório que guarda o endereço da central.
    pub url_variable: String,
    /// A rota que o workflow chama; a tela junta ao endereço público.
    pub hook_path: String,
    pub audience: String,
}

/// Resposta de salvar o `.env`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployEnvSaved {
    pub version: DeployEnvVersionView,
    pub diff: crate::deploys::env::EnvDiff,
    /// O deploy pedido para aplicar o `.env` novo, quando pedido.
    pub run: Option<DeployRunView>,
}

/// Uma cópia do banco feita antes de um deploy, para restaurar.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployBackupView {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub connection_id: i64,
    pub database_name: String,
    pub status: String,
    pub started_at: String,
}
