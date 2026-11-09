//! Deploy contínuo (ADR 021): o GitHub compila e publica as imagens, a
//! central decide quando e onde, o agente aplica.
//!
//! ## Vocabulário
//!
//! * **Aplicação** — um repositório (`dono/repo`): de onde vêm o compose e as
//!   imagens.
//! * **Alvo** — onde ela roda: um agente, um projeto compose, a branch (ou
//!   padrão de tag) que o alimenta e a política (aprovação, janela de
//!   manutenção, backup antes, rollback automático).
//! * **Deploy** (`deploy_runs`) — cada pedido, do webhook à conclusão, com o
//!   log inteiro.
//!
//! ## Como as peças se encaixam
//!
//! | Peça | Responsabilidade |
//! |---|---|
//! | [`credentials`] | o cofre dos tokens (Git e registry), cifrados |
//! | [`applications`] | aplicações e alvos: cadastro e validação |
//! | [`env_versions`] | as versões do `.env` de cada alvo, cifradas |
//! | [`env`] | ler chaves e comparar versões — puro |
//! | [`refs`] | qual alvo um ref do GitHub alimenta — puro |
//! | [`window`] | a janela de manutenção — puro |
//! | [`oidc`] | a prova de que a chamada veio do GitHub Actions do repositório |
//! | [`webhook`] | do evento verificado aos deploys pedidos |
//! | [`runs`] | o registro de cada deploy e a visão que a tela recebe |
//! | [`orchestrator`] | pedir, aprovar, agendar, despachar |
//! | [`executor`] | executar: backup, agente, rollback |
//! | [`events`] | o andamento pelo SSE |
//! | [`notify`] | os avisos (canais e Web Push) |
//! | [`workflow`] | o `.github/workflows/deploy.yml` sugerido — puro |
//! | [`github`] | os commits entre duas versões, para a aprovação |
//! | [`readiness`] | o que falta para o primeiro deploy |
//! | [`ai_tools`] | as ferramentas da IA: consultar e agir (sempre confirmado) |
//! | [`catalog`] | a página inteira numa consulta (hidratação das views) |
//!
//! O processo da API nunca executa `git` nem `docker` (ADR 010, ADR 014):
//! tudo o que toca o host vai ao agente como `DockerCall::StackDeploy`.

pub mod access;
pub mod ai_tools;
pub mod applications;
pub mod catalog;
pub mod connections;
pub mod credentials;
pub mod env;
pub mod env_versions;
pub mod events;
pub mod executor;
pub mod github;
pub mod notify;
pub mod oidc;
pub mod orchestrator;
pub mod readiness;
pub mod refs;
pub mod runs;
pub mod webhook;
pub mod window;
pub mod workflow;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use agent_protocol::docker::stacks::StackStage;
use netmonitor_core::shared::errors::{AppError, AppResult};

/// Estado de um deploy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployRunStatus"
)]
pub enum RunStatus {
    /// Pronto para publicar, esperando um administrador.
    AwaitingApproval,
    /// Aprovado para a próxima janela de manutenção.
    Scheduled,
    /// Na fila: espera o agente, a vez do alvo ou o alvo anterior (canário).
    Queued,
    Running,
    Succeeded,
    Failed,
    /// Falhou e a versão anterior voltou.
    RolledBack,
    /// Um deploy mais novo do mesmo alvo chegou antes deste rodar.
    Superseded,
    Cancelled,
    /// Recusado na aprovação.
    Rejected,
}

impl RunStatus {
    pub const ALL: [Self; 10] = [
        Self::AwaitingApproval,
        Self::Scheduled,
        Self::Queued,
        Self::Running,
        Self::Succeeded,
        Self::Failed,
        Self::RolledBack,
        Self::Superseded,
        Self::Cancelled,
        Self::Rejected,
    ];

    /// Ainda não começou: pode ser substituído ou cancelado.
    pub const PENDING: [Self; 3] = [Self::AwaitingApproval, Self::Scheduled, Self::Queued];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingApproval => "awaiting_approval",
            Self::Scheduled => "scheduled",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::RolledBack => "rolled_back",
            Self::Superseded => "superseded",
            Self::Cancelled => "cancelled",
            Self::Rejected => "rejected",
        }
    }

    /// # Errors
    ///
    /// `Internal` para valor gravado que ninguém reconhece.
    pub fn parse(value: &str) -> AppResult<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
            .ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!("estado de deploy inválido: {value}"))
            })
    }

    #[must_use]
    pub fn is_pending(self) -> bool {
        Self::PENDING.contains(&self)
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded
                | Self::Failed
                | Self::RolledBack
                | Self::Superseded
                | Self::Cancelled
                | Self::Rejected
        )
    }
}

/// O que pediu o deploy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployRunTrigger"
)]
pub enum RunTrigger {
    /// O GitHub Actions, depois de publicar as imagens.
    Webhook,
    /// "Publicar agora" de um administrador ou da IA.
    Manual,
    /// "Restaurar esta versão".
    Rollback,
}

impl RunTrigger {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Webhook => "webhook",
            Self::Manual => "manual",
            Self::Rollback => "rollback",
        }
    }

    /// # Errors
    ///
    /// `Internal` para valor gravado desconhecido.
    pub fn parse(value: &str) -> AppResult<Self> {
        [Self::Webhook, Self::Manual, Self::Rollback]
            .into_iter()
            .find(|trigger| trigger.as_str() == value)
            .ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!("gatilho de deploy inválido: {value}"))
            })
    }
}

/// Como um push alimenta vários alvos da mesma aplicação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployRollout"
)]
pub enum Rollout {
    /// Todos ao mesmo tempo.
    Parallel,
    /// Um de cada vez, na ordem dos alvos; o seguinte só se o anterior subiu
    /// saudável (canário).
    Sequential,
}

impl Rollout {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parallel => "parallel",
            Self::Sequential => "sequential",
        }
    }

    /// # Errors
    ///
    /// `Validation` para valor desconhecido.
    pub fn parse(value: &str) -> AppResult<Self> {
        [Self::Parallel, Self::Sequential]
            .into_iter()
            .find(|rollout| rollout.as_str() == value)
            .ok_or_else(|| AppError::validation(format!("Estratégia inválida: {value}")))
    }
}

/// Etapa em curso de um deploy, para a barra de progresso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum DeployStage {
    /// Backup do banco antes de atualizar (ADR 013).
    Backup,
    Sync,
    Env,
    Pull,
    Start,
    Prune,
    /// Voltando a versão anterior.
    Rollback,
}

impl DeployStage {
    pub const ALL: [Self; 7] = [
        Self::Backup,
        Self::Sync,
        Self::Env,
        Self::Pull,
        Self::Start,
        Self::Prune,
        Self::Rollback,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Backup => "backup",
            Self::Sync => "sync",
            Self::Env => "env",
            Self::Pull => "pull",
            Self::Start => "start",
            Self::Prune => "prune",
            Self::Rollback => "rollback",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stage| stage.as_str() == value)
    }
}

impl From<StackStage> for DeployStage {
    fn from(stage: StackStage) -> Self {
        match stage {
            StackStage::Sync => Self::Sync,
            StackStage::Env => Self::Env,
            StackStage::Pull => Self::Pull,
            StackStage::Start => Self::Start,
            StackStage::Prune => Self::Prune,
        }
    }
}

/// O commit abreviado que a tela mostra.
#[must_use]
pub fn short_sha(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estados_ida_e_volta() {
        for status in RunStatus::ALL {
            assert_eq!(RunStatus::parse(status.as_str()).expect("lido"), status);
            assert_ne!(
                status.is_pending(),
                status.is_terminal() || status == RunStatus::Running
            );
        }
    }

    #[test]
    fn etapas_ida_e_volta() {
        for stage in DeployStage::ALL {
            assert_eq!(DeployStage::parse(stage.as_str()), Some(stage));
        }
        assert_eq!(short_sha("a1b2c3d4e5"), "a1b2c3d");
        assert_eq!(short_sha("a1"), "a1");
    }
}
