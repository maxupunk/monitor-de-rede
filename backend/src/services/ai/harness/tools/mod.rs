//! Ferramentas do Agent Harness da IA.
//!
//! Cada ferramenta é um [`AiToolHandler`]: declara o próprio contrato (nome,
//! descrição e JSON Schema dos argumentos) e sabe se executar. Ferramenta nova
//! é um tipo novo listado em [`all_handlers`] — o loop do agente, o controller
//! e as demais ferramentas não mudam.
//!
//! - [`inventory`]: estado atual (dispositivos, interfaces, monitores, alertas).
//! - [`history`]: histórico gravado no banco (uptime, falhas, métricas).
//! - [`logs`]: syslog e logs da aplicação, agrupados por padrão.
//! - [`grep`]: busca em logs, alertas e checagens devolvendo só o necessário.
//! - [`analysis`]: causa raiz, baseline, padrão por hora e linha do tempo.
//! - [`charts`]: séries do banco desenhadas no chat com os gráficos das telas.
//! - [`diagnostics`]: testes ativos de rede (ping, traceroute, portas, DNS).
//! - [`actions`]: ações que mudam o sistema — sempre com confirmação do usuário.

mod actions;
mod analysis;
mod args;
mod charts;
mod diagnostics;
mod grep;
mod history;
mod inventory;
mod log_digest;
mod logs;
mod lookup;
mod series;
mod timeline;

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

pub use args::ToolArgs;

use crate::{
    dtos::ai::AiChart,
    services::{
        ai::{
            drivers::traits::{AiTool, AiToolFunction},
            settings::AiSettings,
        },
        shared::errors::{AppError, AppResult},
    },
};

/// O que a ferramenta faz com o mundo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    /// Consulta o banco ou a documentação. Sempre disponível.
    Passive,
    /// Envia pacotes (ping, traceroute, scan). Depende de `allow_active_tools`.
    Active,
    /// Muda o estado do sistema (silenciar alerta, criar monitor). Depende de
    /// `allow_actions` e **sempre** passa pela confirmação do usuário.
    Action,
}

/// Quais ferramentas a sessão pode usar e quais pedem confirmação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolPolicy {
    pub allow_active: bool,
    pub allow_actions: bool,
    /// Pede confirmação também antes dos testes ativos.
    pub confirm_active: bool,
}

impl ToolPolicy {
    /// Só leitura: o que as rotinas automáticas (resumos) podem usar.
    #[must_use]
    pub const fn passive() -> Self {
        Self {
            allow_active: false,
            allow_actions: false,
            confirm_active: false,
        }
    }

    #[must_use]
    pub const fn from_settings(settings: &AiSettings) -> Self {
        Self {
            allow_active: settings.allow_active_tools,
            allow_actions: settings.allow_actions,
            confirm_active: settings.require_tool_confirmation,
        }
    }

    #[must_use]
    pub const fn allows(self, kind: ToolKind) -> bool {
        match kind {
            ToolKind::Passive => true,
            ToolKind::Active => self.allow_active,
            ToolKind::Action => self.allow_actions,
        }
    }

    #[must_use]
    pub const fn needs_confirmation(self, kind: ToolKind) -> bool {
        match kind {
            ToolKind::Passive => false,
            ToolKind::Active => self.confirm_active,
            ToolKind::Action => true,
        }
    }
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

    /// Frase que o usuário lê antes de confirmar ("Silenciar o alerta #12 por
    /// 60 min"). Só é chamada para ferramentas que pedem confirmação.
    async fn preview(&self, _ctx: &AppContext, _args: &ToolArgs) -> AppResult<String> {
        Ok(format!("Executar {}", self.name()))
    }

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
        Box::new(analysis::RootCause),
        Box::new(analysis::BaselineComparison),
        Box::new(analysis::HourlyPattern),
        Box::new(analysis::IncidentTimeline),
        Box::new(charts::MonitorLatencyChart),
        Box::new(charts::InterfaceTrafficChart),
        Box::new(charts::DeviceMetricChart),
        Box::new(logs::LogsOverview),
        Box::new(grep::Grep),
        Box::new(inventory::SearchDocs),
        Box::new(diagnostics::Ping),
        Box::new(diagnostics::Traceroute),
        Box::new(diagnostics::ScanPorts),
        Box::new(diagnostics::DnsLookup),
        Box::new(diagnostics::Playbook),
        Box::new(actions::AcknowledgeAlert),
        Box::new(actions::SilenceAlert),
        Box::new(actions::CreateMaintenanceWindow),
        Box::new(actions::CreateMonitor),
    ]
}

/// Ferramentas liberadas para uma sessão de chat.
pub struct ToolRegistry {
    handlers: Vec<Box<dyn AiToolHandler>>,
    policy: ToolPolicy,
}

impl ToolRegistry {
    /// Monta o registro só com o que a política libera.
    #[must_use]
    pub fn new(policy: ToolPolicy) -> Self {
        let handlers = all_handlers()
            .into_iter()
            .filter(|handler| policy.allows(handler.kind()))
            .collect();
        Self { handlers, policy }
    }

    #[must_use]
    pub fn definitions(&self) -> Vec<AiTool> {
        self.handlers
            .iter()
            .map(|handler| handler.definition())
            .collect()
    }

    /// Só encontra o que está no registro: um modelo que invente o nome de
    /// uma ferramenta desabilitada recebe erro, não um ping.
    fn handler(&self, name: &str) -> AppResult<&dyn AiToolHandler> {
        self.handlers
            .iter()
            .find(|handler| handler.name() == name)
            .map(AsRef::as_ref)
            .ok_or_else(|| AppError::validation(format!("Ferramenta indisponível: {name}")))
    }

    /// Se a chamada precisa passar pelo usuário antes de rodar.
    #[must_use]
    pub fn needs_confirmation(&self, name: &str) -> bool {
        self.handler(name)
            .is_ok_and(|handler| self.policy.needs_confirmation(handler.kind()))
    }

    /// Texto mostrado ao usuário no pedido de confirmação.
    ///
    /// # Errors
    ///
    /// Ferramenta fora do registro ou erro do banco.
    pub async fn preview(
        &self,
        ctx: &AppContext,
        name: &str,
        arguments_json: &str,
    ) -> AppResult<String> {
        self.handler(name)?
            .preview(ctx, &ToolArgs::parse(arguments_json))
            .await
    }

    /// Executa uma chamada da IA dentro do chat. O que pede confirmação não
    /// roda aqui — só por [`Self::execute_confirmed`].
    ///
    /// # Errors
    ///
    /// Ferramenta fora do registro, que exige confirmação, argumento inválido
    /// ou falha do banco.
    pub async fn execute(
        &self,
        ctx: &AppContext,
        name: &str,
        arguments_json: &str,
    ) -> AppResult<ToolOutput> {
        let handler = self.handler(name)?;
        if self.policy.needs_confirmation(handler.kind()) {
            return Err(AppError::validation(format!(
                "A ferramenta {name} exige confirmação do usuário"
            )));
        }
        handler.execute(ctx, &ToolArgs::parse(arguments_json)).await
    }

    /// Executa o que o usuário confirmou no chat, em nome dele.
    ///
    /// Recusa ferramenta passiva: este caminho existe só para o que muda algo
    /// ou gera tráfego, e não deve virar uma porta de consulta paralela.
    ///
    /// # Errors
    ///
    /// Ferramenta fora do registro ou passiva, argumento inválido ou falha do
    /// banco.
    pub async fn execute_confirmed(
        &self,
        ctx: &AppContext,
        name: &str,
        arguments: ToolArgs,
    ) -> AppResult<ToolOutput> {
        let handler = self.handler(name)?;
        if handler.kind() == ToolKind::Passive {
            return Err(AppError::validation(format!(
                "A ferramenta {name} não precisa de confirmação"
            )));
        }
        handler.execute(ctx, &arguments).await
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
        let passivas = ToolRegistry::new(ToolPolicy::passive());
        let nomes: Vec<String> = passivas
            .definitions()
            .into_iter()
            .map(|tool| tool.function.name)
            .collect();
        assert!(nomes.contains(&"chart_monitor_latency".to_string()));
        assert!(nomes.contains(&"get_device_interfaces".to_string()));
        assert!(!nomes.contains(&"ping_host".to_string()));

        assert!(!nomes.contains(&"silence_alert".to_string()));

        let todas = ToolRegistry::new(ToolPolicy {
            allow_active: true,
            allow_actions: true,
            confirm_active: false,
        });
        assert_eq!(todas.definitions().len(), all_handlers().len());
    }

    #[test]
    fn acoes_sempre_pedem_confirmacao_e_testes_ativos_so_quando_configurado() {
        let sem = ToolPolicy {
            allow_active: true,
            allow_actions: true,
            confirm_active: false,
        };
        assert!(!sem.needs_confirmation(ToolKind::Passive));
        assert!(!sem.needs_confirmation(ToolKind::Active));
        assert!(sem.needs_confirmation(ToolKind::Action));

        let com = ToolPolicy {
            confirm_active: true,
            ..sem
        };
        assert!(com.needs_confirmation(ToolKind::Active));

        let registro = ToolRegistry::new(com);
        assert!(registro.needs_confirmation("silence_alert"));
        assert!(registro.needs_confirmation("ping_host"));
        assert!(!registro.needs_confirmation("get_alerts"));
        assert!(!registro.needs_confirmation("inexistente"));
    }
}
