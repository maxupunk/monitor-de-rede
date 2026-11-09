//! Entradas das rotas de deploy contínuo (`/api/deploys`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::deploys::Rollout;

/// Credencial do cofre. Token vazio na edição é "manter o salvo": a tela
/// nunca recebe o gravado para reenviar.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployCredentialInput"
)]
pub struct CredentialInput {
    pub name: String,
    /// `ghcr.io` quando vazio.
    #[ts(optional)]
    pub registry: Option<String>,
    pub username: String,
    #[ts(optional)]
    pub secret: Option<String>,
}

/// Uma imagem que o CI compila e publica.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployBuildImage"
)]
pub struct BuildImage {
    /// Nome no registry, sem dono nem tag: `sgc-api`.
    pub name: String,
    /// `apps/api/Dockerfile`.
    pub dockerfile: String,
    /// Contexto do build, relativo à raiz: `.`.
    pub context: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployApplicationInput"
)]
pub struct ApplicationInput {
    pub name: String,
    /// Também o nome padrão do projeto compose.
    pub slug: String,
    /// `dono/repo` no GitHub.
    pub repository: String,
    #[ts(type = "number | null")]
    pub credential_id: Option<i64>,
    pub compose_files: Vec<String>,
    pub image_tag_variable: String,
    #[serde(default)]
    pub images: Vec<BuildImage>,
    pub rollout: Rollout,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployTargetInput"
)]
pub struct TargetInput {
    pub name: String,
    #[ts(type = "number")]
    pub probe_id: i64,
    pub project: String,
    /// Branch (`main`) ou padrão de tag (`v*`).
    pub reference: String,
    pub requires_approval: bool,
    #[ts(optional)]
    pub window_start: Option<String>,
    #[ts(optional)]
    pub window_end: Option<String>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub backup_connection_id: Option<i64>,
    pub wait_timeout_secs: i32,
    pub auto_rollback: bool,
    pub keep_images: i32,
    pub enabled: bool,
    #[serde(default)]
    pub position: i32,
}

/// O assistente: aplicação (nova ou existente), alvo e `.env` de uma vez —
/// tudo ou nada.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeploySetupInput"
)]
pub struct SetupInput {
    /// Aplicação existente; sem ela, `application` cria uma.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub application_id: Option<i64>,
    #[serde(default)]
    #[ts(optional)]
    pub application: Option<ApplicationInput>,
    pub target: TargetInput,
    #[serde(default)]
    #[ts(optional)]
    pub env: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployEnvSaveInput"
)]
pub struct EnvSaveInput {
    pub content: String,
    #[serde(default)]
    #[ts(optional)]
    pub note: Option<String>,
    /// Pedir um deploy com o `.env` novo logo depois de salvar.
    #[serde(default)]
    pub redeploy: bool,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployEnvPreviewInput"
)]
pub struct EnvPreviewInput {
    pub content: String,
}

/// Quando um deploy aprovado roda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DeployWhen {
    /// Agora, mesmo fora da janela de manutenção.
    Now,
    /// Na janela de manutenção do alvo (agora, se ela está aberta).
    Window,
}

/// "Publicar" de um administrador: uma versão já publicada pelo CI.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployInput {
    /// Commit a publicar; vazio republica a versão no ar (com o `.env`
    /// atual).
    #[serde(default)]
    #[ts(optional)]
    pub commit_sha: Option<String>,
    /// Tag da imagem; vazia é o próprio commit.
    #[serde(default)]
    #[ts(optional)]
    pub image_tag: Option<String>,
    pub when: DeployWhen,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployApproveInput"
)]
pub struct ApproveInput {
    pub when: DeployWhen,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployRejectInput"
)]
pub struct RejectInput {
    #[serde(default)]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// O que o passo final do workflow manda, além do token OIDC (que prova
/// repositório, ref e commit).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookInput {
    #[serde(default)]
    pub commit_message: Option<String>,
    /// Tag das imagens publicadas; vazia é o SHA do commit.
    #[serde(default)]
    pub image_tag: Option<String>,
}
