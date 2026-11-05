//! Ponto de extensão do monitoramento (ADR 017, `MonitorChecker`).
//!
//! O motor de monitoramento roda os tipos do núcleo (ping, TCP, HTTP, DNS,
//! recursos do host…) e consulta este registro para os demais: um tipo de
//! monitor de uma área é implementado no crate dela e registrado pelo ponto de
//! montagem. Assim o motor não conhece área nenhuma — e todo checker é
//! compilado e registrado sempre: área desligada não para monitor.

use std::sync::Arc;

use agent_protocol::monitoring::CheckResult;
use async_trait::async_trait;
use serde_json::Value;

use crate::shared::errors::{AppError, AppResult};

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
