//! Contrato HTTP das tarefas agendadas (exportado para o frontend pelo
//! `ts-rs`) e os valores gravados no banco.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::services::ai::harness::tools::{AiToolRisk, ToolKind};

/// Desfecho de uma execução (e o "último" da tarefa).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum ScheduledTaskStatus {
    Running,
    Success,
    Failed,
    /// Não rodou: o servidor estava parado no horário ou a execução anterior
    /// ainda não tinha terminado.
    Skipped,
}

impl ScheduledTaskStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    #[must_use]
    pub fn from_db(value: &str) -> Option<Self> {
        Some(match value {
            "running" => Self::Running,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            _ => return None,
        })
    }
}

/// O que disparou a execução.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum ScheduledTaskTrigger {
    /// O horário da agenda.
    Schedule,
    /// "Rodar agora", na tela ou no chat.
    Manual,
}

impl ScheduledTaskTrigger {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Schedule => "schedule",
            Self::Manual => "manual",
        }
    }

    #[must_use]
    pub fn from_db(value: &str) -> Self {
        if value == "manual" {
            Self::Manual
        } else {
            Self::Schedule
        }
    }
}

/// Por onde a tarefa foi criada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum ScheduledTaskOrigin {
    /// Pela tela.
    User,
    /// Pelo Assistente IA, com a confirmação do usuário.
    Ai,
}

impl ScheduledTaskOrigin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Ai => "ai",
        }
    }

    #[must_use]
    pub fn from_db(value: &str) -> Self {
        if value == "ai" {
            Self::Ai
        } else {
            Self::User
        }
    }
}

/// O que a ferramenta faz, na linguagem da tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum SchedulableToolKind {
    /// Só lê o banco.
    Query,
    /// Teste ativo na rede (ping, traceroute…).
    Test,
    /// Muda o sistema (alerta, manutenção, monitor, regra).
    Action,
    /// Inicia, para ou reinicia container.
    Container,
}

impl SchedulableToolKind {
    #[must_use]
    pub const fn of(kind: ToolKind) -> Self {
        match kind {
            ToolKind::Active => Self::Test,
            ToolKind::Action => Self::Action,
            ToolKind::ContainerAction => Self::Container,
            ToolKind::Passive | ToolKind::Interactive | ToolKind::DeviceAccess => Self::Query,
        }
    }
}

/// Uma ferramenta que pode virar tarefa.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SchedulableTool {
    pub name: String,
    pub description: String,
    /// Id do grupo do catálogo (`diagnostics`, `actions`…).
    pub group: String,
    /// O grupo em português.
    pub group_label: String,
    pub kind: SchedulableToolKind,
    /// Só o administrador agenda.
    pub admin_only: bool,
    /// JSON Schema dos argumentos — o mesmo que a IA recebe.
    #[ts(type = "Record<string, unknown>")]
    pub parameters: Value,
}

/// Uma tarefa agendada.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ScheduledTask {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub tool: String,
    #[ts(type = "Record<string, unknown>")]
    pub arguments: Value,
    /// Expressão de 5 campos, hora local do servidor.
    pub cron: String,
    /// A agenda em português ("todo dia às 03:00").
    pub schedule_label: String,
    pub enabled: bool,
    pub origin: ScheduledTaskOrigin,
    pub next_run_at: Option<String>,
    pub last_run_at: Option<String>,
    pub last_status: Option<ScheduledTaskStatus>,
    pub last_message: Option<String>,
    /// Em nome de quem roda: quem salvou por último.
    #[ts(type = "number | null")]
    pub owner_id: Option<i64>,
    pub owner_name: Option<String>,
    /// Há uma execução em andamento agora.
    pub running: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Criação/edição de uma tarefa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ScheduledTaskInput {
    pub name: String,
    #[serde(default)]
    #[ts(optional)]
    pub description: Option<String>,
    pub tool: String,
    #[serde(default = "empty_object")]
    #[ts(type = "Record<string, unknown>")]
    pub arguments: Value,
    pub cron: String,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

/// O que conferir antes de salvar: a ferramenta, os argumentos e a agenda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ScheduledTaskPreviewInput {
    pub tool: String,
    #[serde(default = "empty_object")]
    #[ts(type = "Record<string, unknown>")]
    pub arguments: Value,
    pub cron: String,
}

/// A conferência: o que está errado, a agenda em português, as próximas
/// execuções e o que a ferramenta vai fazer.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ScheduledTaskPreview {
    pub valid: bool,
    pub errors: Vec<String>,
    pub schedule_label: Option<String>,
    pub next_runs: Vec<String>,
    /// A frase da ação ("Silenciar o alerta #12 por 60 min"); `null` em
    /// consultas.
    pub summary: Option<String>,
    pub risk: Option<AiToolRisk>,
    pub risk_reason: Option<String>,
}

/// Uma execução.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ScheduledTaskRun {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub task_id: i64,
    pub trigger: ScheduledTaskTrigger,
    pub status: ScheduledTaskStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<i64>,
    pub message: Option<String>,
    /// O que a ferramenta devolveu (cortado quando enorme).
    #[ts(type = "unknown")]
    pub result: Option<Value>,
}

/// Resposta do "rodar agora": a execução que começou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ScheduledTaskStarted {
    #[ts(type = "number")]
    pub run_id: i64,
}

/// Evento `scheduled_task:updated`: a tela relê a lista (e o histórico, se
/// for o aberto).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ScheduledTaskEvent {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub status: Option<ScheduledTaskStatus>,
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

const fn enabled_by_default() -> bool {
    true
}
