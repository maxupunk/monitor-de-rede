//! O vocabulário Docker do canal: operações, filtros, erros e os DTOs que o
//! agente devolve ([`views`]).
//!
//! Mora aqui, e não no crate da área Servidores, porque o agente e a central
//! precisam do mesmo tipo dos dois lados do fio — e o contrato não pode
//! depender de área nenhuma (ADR 017).

pub mod images;
pub mod stacks;
pub mod views;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("Docker está desabilitado nesta instalação")]
    Disabled,
    #[error("Docker Engine indisponível")]
    Unavailable,
    #[error("Recurso Docker não encontrado")]
    NotFound,
    #[error("A operação conflita com o estado atual do recurso Docker")]
    Conflict,
    #[error("{0}")]
    Validation(String),
    #[error("Falha ao comunicar com a Docker Engine")]
    Engine,
    /// A política local do agente não permite a operação (ADR 011).
    #[error("{0}")]
    Forbidden(String),
    /// A operação não existe para este host (ex.: compose na própria central).
    #[error("{0}")]
    Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Remove { force: bool },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilters {
    pub tail: String,
    pub since: i64,
    pub until: i64,
    pub timestamps: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum ComposeAction {
    Pull,
    Up,
    Restart,
    Stop,
    Down,
}

impl ComposeAction {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pull => "pull",
            Self::Up => "up",
            Self::Restart => "restart",
            Self::Stop => "stop",
            Self::Down => "down",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeRequest {
    pub project: String,
    pub action: ComposeAction,
    /// Serviço específico; `None` age sobre o projeto inteiro.
    #[serde(default)]
    pub service: Option<String>,
}
