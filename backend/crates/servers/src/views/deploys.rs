//! Serialização do deploy contínuo para a tela.
//!
//! Segredo nenhum sai daqui: credencial só diz que tem token, versão do
//! `.env` só lista as chaves, e o deploy não carrega o log (que vem sob
//! demanda).

use serde::Serialize;
use ts_rs::TS;

use agent_protocol::docker::stacks::StackReadiness;
use netmonitor_core::models::{
    deploy_applications, deploy_credentials, deploy_env_versions, deploy_github_connections,
    deploy_runs, deploy_targets,
};

use crate::{
    deploys::{
        applications::{compose_files_of, images_of, repository_url, rollout_of},
        env_versions::keys_of,
        images::VulnerabilityPolicy,
        runs::status_of,
        short_sha, DeployStage, Rollout, RunStatus, RunTrigger,
    },
    dtos::deploys::{BuildArgInput, BuildImage, Builder},
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
    /// A compilação no servidor que produziu a imagem (ADR 023).
    #[ts(type = "number | null")]
    pub build_id: Option<i64>,
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
            build_id: run.build_id,
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
    /// A política local libera compilar (`build`, ADR 023).
    pub allows_build: bool,
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
    pub vulnerability_policy: VulnerabilityPolicy,
    pub require_provenance: bool,
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
            vulnerability_policy: VulnerabilityPolicy::parse(&row.vulnerability_policy),
            require_provenance: row.require_provenance,
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
    #[ts(type = "number | null")]
    pub github_connection_id: Option<i64>,
    /// A conta do GitHub da conexão (`maxupunk`).
    pub github_account: Option<String>,
    pub compose_files: Vec<String>,
    pub image_tag_variable: String,
    pub images: Vec<BuildImage>,
    pub rollout: Rollout,
    pub builder: Builder,
    #[ts(type = "number | null")]
    pub build_probe_id: Option<i64>,
    pub test_target: Option<String>,
    pub build_args: Vec<BuildArgInput>,
    pub targets: Vec<DeployTargetView>,
    pub created_at: String,
}

impl DeployApplicationView {
    #[must_use]
    pub fn new(
        row: &deploy_applications::Model,
        credential_name: Option<String>,
        github_account: Option<String>,
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
            github_connection_id: row.github_connection_id,
            github_account,
            compose_files: compose_files_of(row),
            image_tag_variable: row.image_tag_variable.clone(),
            images: images_of(row),
            rollout: rollout_of(row),
            builder: Builder::parse(&row.builder),
            build_probe_id: row.build_probe_id,
            test_target: row.test_target.clone(),
            build_args: serde_json::from_value(row.build_args.clone()).unwrap_or_default(),
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
    pub github_connections: Vec<DeployGithubConnectionView>,
    /// O endereço que o workflow chama (`…/api/deploy-hooks/github`).
    pub hook_path: String,
    /// O `audience` que o token OIDC precisa ter.
    pub oidc_audience: String,
    /// O caminho até a primeira versão no ar, etapa por etapa.
    pub guide: DeployGuideView,
}

/// Uma etapa do caminho até o primeiro deploy (`deploys::guide`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DeployGuideStepKey {
    Github,
    Agent,
    Application,
    Target,
    Env,
    FirstDeploy,
}

/// Onde a etapa está.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DeployGuideStatus {
    Done,
    /// A próxima a fazer (só uma por vez).
    Current,
    Todo,
    /// Recomendada, mas não segura as outras (o `.env`).
    Optional,
}

/// O que o botão da etapa faz na tela; a IA lê o mesmo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DeployGuideAction {
    ConnectGithub,
    InstallGithub {
        url: String,
    },
    OpenAgents,
    Setup,
    AddTarget {
        #[serde(rename = "applicationId")]
        #[ts(rename = "applicationId", type = "number")]
        application_id: i64,
    },
    EditEnv {
        #[serde(rename = "targetId")]
        #[ts(rename = "targetId", type = "number")]
        target_id: i64,
    },
    BuildNow {
        #[serde(rename = "applicationId")]
        #[ts(rename = "applicationId", type = "number")]
        application_id: i64,
    },
    OpenRun {
        #[serde(rename = "runId")]
        #[ts(rename = "runId", type = "number")]
        run_id: i64,
    },
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployGuideStep {
    pub key: DeployGuideStepKey,
    pub status: DeployGuideStatus,
    pub title: String,
    /// Como fazer a etapa (vale sempre; é o que a IA explica).
    pub how: String,
    /// O que o sistema vê agora nesta etapa (o que falta ou o que já está pronto).
    pub detail: String,
    pub action: Option<DeployGuideAction>,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployGuideView {
    pub steps: Vec<DeployGuideStep>,
    /// Todas as etapas obrigatórias feitas: cada push já publica sozinho.
    pub done: bool,
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

// ── GitHub ───────────────────────────────────────────────────────────────

/// Uma conexão com o GitHub (o GitHub App que esta central criou).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployGithubConnectionView {
    #[ts(type = "number")]
    pub id: i64,
    /// Conta ou organização dona do app.
    pub account: String,
    pub organization: bool,
    pub app_slug: String,
    /// Instalado: já enxerga repositórios.
    pub installed: bool,
    /// Pode abrir o PR do workflow.
    pub can_write: bool,
    /// Onde instalar (ou escolher outros repositórios).
    pub install_url: String,
    /// As configurações do app no GitHub (permissões, excluir).
    pub settings_url: String,
    pub created_at: String,
}

impl From<&deploy_github_connections::Model> for DeployGithubConnectionView {
    fn from(row: &deploy_github_connections::Model) -> Self {
        let organization = row.account_type == "Organization";
        let settings_url = if organization {
            format!(
                "https://github.com/organizations/{}/settings/apps/{}",
                row.account_login, row.app_slug
            )
        } else {
            format!("https://github.com/settings/apps/{}", row.app_slug)
        };
        Self {
            id: row.id,
            account: row.account_login.clone(),
            organization,
            app_slug: row.app_slug.clone(),
            installed: row.installation_id.is_some(),
            can_write: row.can_write,
            install_url: format!("https://github.com/apps/{}/installations/new", row.app_slug),
            settings_url,
            created_at: timestamp(&row.created_at),
        }
    }
}

/// O formulário que a tela envia ao GitHub para criar o app.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployGithubManifestView {
    /// O `action` do formulário (já com o `state`).
    pub action_url: String,
    /// O campo `manifest`: JSON.
    pub manifest: String,
}

/// Um repositório que a credencial enxerga.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployRepositoryView {
    /// `dono/repo`.
    pub full_name: String,
    pub private: bool,
    pub default_branch: String,
    pub description: Option<String>,
    pub pushed_at: Option<String>,
}

/// Branches e tags, para escolher o que cada alvo acompanha.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployReferencesView {
    pub default_branch: String,
    pub branches: Vec<String>,
    pub tags: Vec<String>,
}

/// O que o compose do repositório preenche sozinho.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployComposeDiscovery {
    /// Os arquivos compose na raiz do repositório.
    pub files: Vec<String>,
    /// O que o `docker compose` usaria sozinho.
    pub suggested_files: Vec<String>,
    pub services: Vec<String>,
    pub images: Vec<BuildImage>,
    pub image_tag_variable: Option<String>,
    /// O que ajustar no compose antes do primeiro deploy.
    pub notes: Vec<String>,
}

/// O `deploy.yml` do repositório comparado ao gerado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DeployWorkflowState {
    /// Não há `deploy.yml` na branch padrão.
    Missing,
    /// Igual ao gerado.
    Current,
    /// Diferente: editado à mão ou gerado antes de mudar a aplicação.
    Different,
    /// Não deu para ler.
    Unknown,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployPullRequestView {
    pub number: u64,
    pub url: String,
}

/// A última execução do workflow no Actions.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployBuildView {
    /// `queued`, `in_progress`, `completed`…
    pub status: String,
    /// `success`, `failure`, `cancelled`… (só quando terminou).
    pub conclusion: Option<String>,
    pub url: String,
    pub branch: Option<String>,
    pub short_sha: String,
    pub created_at: String,
}

/// Como está a ligação com o GitHub de uma aplicação.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployGithubStatusView {
    pub workflow: DeployWorkflowState,
    /// O arquivo no GitHub, quando existe.
    pub workflow_url: Option<String>,
    /// O PR do NetMonitor ainda aberto.
    pub pull_request: Option<DeployPullRequestView>,
    pub last_build: Option<DeployBuildView>,
    /// A conexão pode abrir o PR.
    pub can_write: bool,
    /// Por que não deu para conferir.
    pub unavailable_reason: Option<String>,
}
