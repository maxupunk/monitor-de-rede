//! Ponto de extensão do monitoramento (ADR 017, `MonitorChecker`).
//!
//! O motor de monitoramento roda os tipos do núcleo (ping, TCP, HTTP, DNS,
//! recursos do host…) e consulta este registro para os demais: um tipo de
//! monitor de uma área é implementado no crate dela e registrado pelo ponto de
//! montagem. Assim o motor não conhece área nenhuma — e todo checker é
//! compilado e registrado sempre: área desligada não para monitor.

pub mod check_deps;
pub mod device_status;
pub mod execution_guard;
pub mod health;
pub mod link_speed;
pub mod metrics_repository;
pub mod reachability;
pub mod series;

pub use check_deps::CheckDeps;

use std::sync::Arc;

use agent_protocol::monitoring::CheckResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use serde_json::Value;

use crate::{
    models::monitors,
    shared::errors::{AppError, AppResult},
};

/// Abstração para checkers puros e testáveis fora do HTTP.
#[async_trait]
pub trait Checker {
    type Config: Send;

    /// Executa uma medição e sempre produz uma observação útil ao operador.
    async fn execute(&self, config: Self::Config) -> CheckResult;
}

/// Um tipo de monitor atendido por uma área.
#[async_trait]
pub trait MonitorChecker: Send + Sync {
    /// O tipo (`monitors.type`), em minúsculas.
    fn kind(&self) -> &'static str;

    /// Executa uma medição com a configuração do monitor (já com o
    /// `timeoutMs` do monitor aplicado).
    ///
    /// # Errors
    ///
    /// Configuração inválida ou dependência de processo ausente. Falha de
    /// medição é dado (`CheckResult`), não erro.
    async fn check(&self, configuration: &Value) -> AppResult<CheckResult>;
}

/// Os checkers registrados. Barato de clonar: vive no `shared_store` do
/// processo e é copiado para as dependências de cada execução.
#[derive(Clone, Default)]
pub struct MonitorCheckers(Arc<Vec<Arc<dyn MonitorChecker>>>);

impl MonitorCheckers {
    #[must_use]
    pub fn new(checkers: Vec<Arc<dyn MonitorChecker>>) -> Self {
        Self(Arc::new(checkers))
    }

    /// O checker do tipo, se alguma área o registrou.
    #[must_use]
    pub fn find(&self, kind: &str) -> Option<&dyn MonitorChecker> {
        self.0
            .iter()
            .find(|checker| checker.kind() == kind)
            .map(AsRef::as_ref)
    }
}

/// Monitores que uma área executa **em grupo**: os SNMP locais de um mesmo
/// dispositivo compartilham uma coleta só (da Rede). O ciclo do scheduler
/// tira do despacho individual os monitores vencidos que algum grupo
/// reivindica, chama o grupo uma vez por chave e processa cada resultado como
/// o de qualquer monitor. Grupo nunca pausa: é monitor.
#[async_trait]
pub trait MonitorGroup: Send + Sync {
    /// A chave do grupo do monitor (o dispositivo), se este grupo o executa.
    fn group_of(&self, monitor: &monitors::Model) -> Option<i64>;

    /// Executa o grupo da chave e devolve o resultado de cada monitor.
    ///
    /// # Errors
    ///
    /// Erro do banco; falha de medição é dado (`CheckResult`).
    async fn run(
        &self,
        ctx: &AppContext,
        key: i64,
        scheduled_at: DateTime<Utc>,
    ) -> AppResult<Vec<(i64, CheckResult)>>;

    /// "Testar agora" de um monitor da tela: se ele é deste grupo, executa o
    /// grupo inteiro fora da cadência e devolve o resultado de cada monitor.
    /// `None` quando o monitor não é deste grupo.
    async fn run_now(
        &self,
        ctx: &AppContext,
        monitor: &monitors::Model,
    ) -> Option<AppResult<Vec<(i64, CheckResult)>>>;
}

/// Os grupos registrados, no `shared_store` do processo.
#[derive(Clone, Default)]
pub struct MonitorGroups(Arc<Vec<Arc<dyn MonitorGroup>>>);

impl MonitorGroups {
    #[must_use]
    pub fn new(groups: Vec<Arc<dyn MonitorGroup>>) -> Self {
        Self(Arc::new(groups))
    }

    /// Os do processo; vazio quando o ponto de montagem não registrou nenhum.
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Self {
        ctx.shared_store.get::<Self>().unwrap_or_default()
    }

    /// `(grupo, chave)` do monitor, se algum grupo o reivindica.
    #[must_use]
    pub fn group_of(&self, monitor: &monitors::Model) -> Option<(usize, i64)> {
        self.0
            .iter()
            .enumerate()
            .find_map(|(index, group)| group.group_of(monitor).map(|key| (index, key)))
    }

    /// "Testar agora" pelo grupo que reivindica o monitor, se algum.
    pub async fn run_now(
        &self,
        ctx: &AppContext,
        monitor: &monitors::Model,
    ) -> Option<AppResult<Vec<(i64, CheckResult)>>> {
        for group in self.0.iter() {
            if let Some(result) = group.run_now(ctx, monitor).await {
                return Some(result);
            }
        }
        None
    }

    /// O grupo de índice `index` (de [`Self::group_of`]).
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&dyn MonitorGroup> {
        self.0.get(index).map(AsRef::as_ref)
    }
}

/// Lê a configuração de um monitor no formato do checker.
///
/// # Errors
///
/// `validation` com o nome do tipo quando o JSON não serve.
pub fn parse_config<T: serde::de::DeserializeOwned>(
    configuration: &Value,
    name: &str,
) -> AppResult<T> {
    serde_json::from_value(configuration.clone()).map_err(|error| {
        AppError::validation(format!(
            "Configuração inválida para monitor {name}: {error}"
        ))
    })
}
