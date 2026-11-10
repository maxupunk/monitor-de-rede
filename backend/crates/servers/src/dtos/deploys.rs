//! Entradas das rotas de deploy contínuo (`/api/deploys`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::deploys::{images::VulnerabilityPolicy, Rollout};

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
    /// Estágio final do Dockerfile (`--target`), quando há vários.
    #[serde(default)]
    #[ts(optional)]
    pub target: Option<String>,
    /// O `image:` do compose sem a tag (`ghcr.io/acme/loja-api`): o nome que a
    /// imagem ganha no host quando a central a entrega (compilação no
    /// servidor, ADR 023).
    #[serde(default)]
    #[ts(optional)]
    pub reference: Option<String>,
}

/// Quem compila as imagens da aplicação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployBuilder"
)]
pub enum Builder {
    /// O agente compila no servidor e a imagem fica no registry da central
    /// (ADR 023). É o que o assistente propõe.
    Netmonitor,
    /// O workflow do GitHub Actions compila e publica (ADR 021). Padrão da
    /// API: quem a chama sem dizer nada continua como antes.
    #[default]
    GithubActions,
}

impl Builder {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Netmonitor => "netmonitor",
            Self::GithubActions => "github_actions",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Self {
        if raw == "netmonitor" {
            Self::Netmonitor
        } else {
            Self::GithubActions
        }
    }
}

/// Argumento de build (`--build-arg`): vai para o histórico da imagem — não
/// é lugar de segredo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployBuildArg"
)]
pub struct BuildArgInput {
    pub name: String,
    pub value: String,
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
    /// O token: imagens privadas e, sem conexão, o repositório.
    #[ts(type = "number | null")]
    pub credential_id: Option<i64>,
    /// A conexão com o GitHub (o app) que lê o repositório.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub github_connection_id: Option<i64>,
    pub compose_files: Vec<String>,
    pub image_tag_variable: String,
    #[serde(default)]
    pub images: Vec<BuildImage>,
    pub rollout: Rollout,
    #[serde(default)]
    pub builder: Builder,
    /// O agente que compila; nulo = o do primeiro alvo da revisão.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub build_probe_id: Option<i64>,
    /// Estágio do Dockerfile que roda os testes antes das imagens.
    #[serde(default)]
    #[ts(optional)]
    pub test_target: Option<String>,
    #[serde(default)]
    pub build_args: Vec<BuildArgInput>,
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
    #[serde(default = "default_policy")]
    pub vulnerability_policy: VulnerabilityPolicy,
    /// Só imagem de origem conhecida (compilada pela central ou enviada pelo
    /// workflow do repositório).
    #[serde(default)]
    pub require_provenance: bool,
}

const fn default_policy() -> VulnerabilityPolicy {
    VulnerabilityPolicy::Warn
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

// ── GitHub ───────────────────────────────────────────────────────────────

/// "Conectar GitHub": o formulário que a tela envia ao GitHub.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployGithubManifestInput {
    /// O endereço da central como o navegador a vê (`window.location.origin`).
    pub origin: String,
    /// Login da organização; vazio cria o app na conta de quem está logado.
    #[serde(default)]
    #[ts(optional)]
    pub organization: Option<String>,
    /// Pedir as permissões de escrita (abrir o PR do workflow).
    #[serde(default)]
    pub allow_write: bool,
}

/// O retorno do GitHub depois de criar o app.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployGithubConnectInput {
    pub code: String,
    pub state: String,
}

/// O retorno do GitHub depois de instalar o app.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployGithubInstallationInput {
    #[ts(type = "number")]
    pub installation_id: i64,
}

/// De onde ler o GitHub na descoberta (o assistente ainda não salvou a
/// aplicação).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubSourceQuery {
    #[serde(default)]
    pub connection_id: Option<i64>,
    #[serde(default)]
    pub credential_id: Option<i64>,
    #[serde(default)]
    pub repository: Option<String>,
    /// Branch ou tag; vazio é a branch padrão.
    #[serde(default)]
    pub reference: Option<String>,
}

/// O endereço público da central, que o workflow chama.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CentralUrlQuery {
    #[serde(default)]
    pub central_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DeployPullRequestInput {
    pub central_url: String,
}
