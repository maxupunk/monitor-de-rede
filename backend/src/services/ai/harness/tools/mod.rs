//! Ferramentas do Agent Harness da IA.
//!
//! Cada ferramenta é um [`AiToolHandler`]: declara o próprio contrato (nome,
//! descrição e JSON Schema dos argumentos) e sabe se executar. Ferramenta nova
//! é um tipo novo listado em [`all_handlers`] — o loop do agente, o controller
//! e as demais ferramentas não mudam.
//!
//! - [`inventory`]: estado atual (dispositivos, interfaces, monitores, alertas).
//! - [`history`]: histórico gravado no banco (uptime, falhas, métricas).
//! - [`charts`]: séries do banco desenhadas no chat com os gráficos das telas.
//! - [`diagnostics`]: testes ativos de rede (ping, traceroute, portas, DNS).

mod args;
mod charts;
mod diagnostics;
mod history;
mod inventory;
mod lookup;
mod series;

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

pub use args::ToolArgs;

use crate::{
    dtos::ai::AiChart,
    services::{
        ai::drivers::traits::{AiTool, AiToolFunction},
        shared::errors::{AppError, AppResult},
    },
};

/// Se a ferramenta só lê o que o NetMonitor já sabe ou se gera tráfego de rede.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    /// Consulta o banco ou a documentação. Sempre disponível.
    Passive,
    /// Envia pacotes (ping, traceroute, scan). Depende de `allow_active_tools`.
    Active,
}

/// Resultado de uma ferramenta.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    /// O que a IA recebe: compacto, só o necessário para o diagnóstico.
    pub data: Value,
    /// O que a tela desenha. Não volta para a IA — os pontos custariam tokens
    /// sem acrescentar ao resumo que já está em `data`.
    pub chart: Option<AiChart>,
}

impl ToolOutput {
    #[must_use]
    pub const fn data(data: Value) -> Self {
        Self { data, chart: None }
    }

    #[must_use]
    pub const fn with_chart(data: Value, chart: AiChart) -> Self {
        Self {
            data,
            chart: Some(chart),
        }
    }

    /// Falha de negócio devolvida à IA como dado (alvo não encontrado etc.):
    /// ela pode corrigir o argumento e tentar de novo.
    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::data(json!({ "error": message.into() }))
    }
}

#[async_trait]
pub trait AiToolHandler: Send + Sync {
    fn name(&self) -> &'static str;

    fn description(&self) -> &'static str;

    /// JSON Schema dos argumentos.
    fn parameters(&self) -> Value;

    fn kind(&self) -> ToolKind {
        ToolKind::Passive
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput>;

    /// Contrato no formato de function calling da OpenAI.
    fn definition(&self) -> AiTool {
        AiTool {
            r#type: "function".to_string(),
            function: AiToolFunction {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: self.parameters(),
            },
        }
    }
}

/// Catálogo completo, na ordem em que a IA as vê.
fn all_handlers() -> Vec<Box<dyn AiToolHandler>> {
    vec![
        Box::new(inventory::SystemSummary),
        Box::new(inventory::ListDevices),
        Box::new(inventory::DeviceDetail),
        Box::new(inventory::DeviceInterfaces),
        Box::new(inventory::ListMonitors),
        Box::new(inventory::Alerts),
        Box::new(history::MonitorHistory),
        Box::new(history::DeviceMetrics),
        Box::new(charts::MonitorLatencyChart),
        Box::new(charts::InterfaceTrafficChart),
        Box::new(charts::DeviceMetricChart),
        Box::new(inventory::SearchDocs),
        Box::new(diagnostics::Ping),
        Box::new(diagnostics::Traceroute),
        Box::new(diagnostics::ScanPorts),
        Box::new(diagnostics::DnsLookup),
        Box::new(diagnostics::Playbook),
    ]
}

/// Ferramentas liberadas para uma sessão de chat.
pub struct ToolRegistry {
    handlers: Vec<Box<dyn AiToolHandler>>,
}

impl ToolRegistry {
    /// Monta o registro; as ferramentas ativas só entram com `allow_active`.
    #[must_use]
    pub fn new(allow_active: bool) -> Self {
        let handlers = all_handlers()
            .into_iter()
            .filter(|handler| allow_active || handler.kind() == ToolKind::Passive)
            .collect();
        Self { handlers }
    }

    #[must_use]
    pub fn definitions(&self) -> Vec<AiTool> {
        self.handlers
            .iter()
            .map(|handler| handler.definition())
            .collect()
    }

    /// Executa a ferramenta pedida pela IA.
    ///
    /// Só roda o que está no registro: um modelo que invente o nome de uma
    /// ferramenta ativa desabilitada recebe erro, não um ping.
    ///
    /// # Errors
    ///
    /// Ferramenta fora do registro, argumento inválido ou falha do banco.
    pub async fn execute(
        &self,
        ctx: &AppContext,
        name: &str,
        arguments_json: &str,
    ) -> AppResult<ToolOutput> {
        let handler = self
            .handlers
            .iter()
            .find(|handler| handler.name() == name)
            .ok_or_else(|| AppError::validation(format!("Ferramenta indisponível: {name}")))?;
        handler.execute(ctx, &ToolArgs::parse(arguments_json)).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn nomes_sao_unicos() {
        let handlers = all_handlers();
        let names: HashSet<&str> = handlers.iter().map(|handler| handler.name()).collect();
        assert_eq!(names.len(), handlers.len());
    }

    #[test]
    fn todo_contrato_e_um_objeto_json_schema() {
        for handler in all_handlers() {
            let schema = handler.parameters();
            assert_eq!(schema["type"], "object", "{}", handler.name());
            assert!(schema["properties"].is_object(), "{}", handler.name());
            assert!(!handler.description().is_empty(), "{}", handler.name());
        }
    }

    #[test]
    fn ferramentas_ativas_so_entram_quando_liberadas() {
        let passivas = ToolRegistry::new(false);
        let nomes: Vec<String> = passivas
            .definitions()
            .into_iter()
            .map(|tool| tool.function.name)
            .collect();
        assert!(nomes.contains(&"chart_monitor_latency".to_string()));
        assert!(nomes.contains(&"get_device_interfaces".to_string()));
        assert!(!nomes.contains(&"ping_host".to_string()));

        let todas = ToolRegistry::new(true);
        assert_eq!(todas.definitions().len(), all_handlers().len());
    }
}
