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
//! - [`alert_rules`]: origem de um alerta, regras cadastradas, guia e — com
//!   confirmação — criar ou excluir regra.
//! - [`docker`]: containers da central e dos agentes remotos (estado e
//!   consumo, sem variáveis de ambiente).
//! - [`platform`]: o próprio NetMonitor — agentes, VPN, topologia, descoberta,
//!   redes, manutenção, auditoria e notificações.
//! - [`ask`]: a IA pergunta ao usuário antes de prosseguir.

mod actions;
mod alert_rules;
mod analysis;
mod args;
mod ask;
mod charts;
mod diagnostics;
mod docker;
mod grep;
mod history;
mod inventory;
mod log_digest;
mod logs;
mod lookup;
mod platform;
mod series;
mod timeline;

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

pub use args::ToolArgs;
pub use lookup::device_matches;

use crate::{
    dtos::ai::AiChart,
    services::{
        ai::{
            drivers::traits::{AiTool, AiToolFunction},
            settings::{AiContainerActionMode, AiSettings},
        },
        audit::AuditActor,
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
    /// Conversa com o usuário (perguntar antes de prosseguir). Só no chat —
    /// nas rotinas automáticas não há ninguém para responder.
    Interactive,
    /// Inicia, para ou reinicia container. Tem configuração própria
    /// ([`AiContainerActionMode`]): desligado, com confirmação (padrão) ou
    /// automático.
    ContainerAction,
}

/// Grupo do catálogo. Só o [`ToolGroup::Core`] vai em toda chamada ao
/// provedor; os demais entram quando a pergunta pede ou quando a IA os
/// carrega com [`LOAD_TOOLS`] — cada schema enviado custa tokens em toda
/// rodada, use a IA a ferramenta ou não.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolGroup {
    /// Inventário, alertas, grep, documentação e a pergunta ao usuário.
    Core,
    History,
    Analysis,
    Charts,
    Logs,
    Docker,
    Diagnostics,
    Actions,
    AlertRules,
    Platform,
}

impl ToolGroup {
    /// Os grupos que podem ser carregados sob demanda, na ordem do catálogo.
    pub const DEFERRED: [Self; 9] = [
        Self::History,
        Self::Analysis,
        Self::Charts,
        Self::Logs,
        Self::Docker,
        Self::Platform,
        Self::Diagnostics,
        Self::Actions,
        Self::AlertRules,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::History => "history",
            Self::Analysis => "analysis",
            Self::Charts => "charts",
            Self::Logs => "logs",
            Self::Docker => "docker",
            Self::Diagnostics => "diagnostics",
            Self::Actions => "actions",
            Self::AlertRules => "alert_rules",
            Self::Platform => "platform",
        }
    }

    /// Para que serve, na linha do catálogo.
    #[must_use]
    pub const fn purpose(self) -> &'static str {
        match self {
            Self::Core => "consultas básicas",
            Self::History => "uptime, falhas e métricas gravadas no tempo",
            Self::Analysis => {
                "causa raiz, comparação com o normal, padrão por hora, linha do tempo"
            }
            Self::Charts => "gráficos de latência, tráfego e CPU/memória",
            Self::Logs => "panorama dos logs agrupado por padrão",
            Self::Docker => {
                "containers da central e dos agentes remotos: estado, consumo de CPU/memória e servidores"
            }
            Self::Diagnostics => "ping, traceroute, portas, DNS e playbooks",
            Self::Actions => "reconhecer/silenciar alerta, janela de manutenção, criar monitor",
            Self::AlertRules => {
                "origem de um alerta, regras cadastradas, guia de regras, criar/excluir regra"
            }
            Self::Platform => {
                "agentes remotos, VPN, topologia/vizinhos, descoberta, redes/sites/DNS, manutenção, auditoria, notificações"
            }
        }
    }

    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::DEFERRED
            .into_iter()
            .find(|group| group.id().eq_ignore_ascii_case(id.trim()))
    }
}

/// Grupos carregados numa sessão, na ordem em que entraram.
///
/// A ordem importa: os contratos vão ao provedor nessa sequência, e um grupo
/// novo entra no fim da lista — o começo dela continua idêntico e o cache de
/// prefixo do provedor segue valendo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolGroups(Vec<ToolGroup>);

impl ToolGroups {
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// `false` quando o grupo já estava carregado.
    pub fn insert(&mut self, group: ToolGroup) -> bool {
        if self.contains(&group) {
            return false;
        }
        self.0.push(group);
        true
    }

    #[must_use]
    pub fn contains(&self, group: &ToolGroup) -> bool {
        self.0.contains(group)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = ToolGroup> + '_ {
        self.0.iter().copied()
    }

    /// Ids na ordem de carga, para a tela devolver na próxima pergunta.
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        self.0.iter().map(|group| group.id().to_string()).collect()
    }

    /// Grupos pelos ids; o que não existe é ignorado.
    #[must_use]
    pub fn from_ids<S: AsRef<str>>(ids: &[S]) -> Self {
        ids.iter()
            .filter_map(|id| ToolGroup::from_id(id.as_ref()))
            .collect()
    }
}

impl Extend<ToolGroup> for ToolGroups {
    fn extend<I: IntoIterator<Item = ToolGroup>>(&mut self, groups: I) {
        for group in groups {
            self.insert(group);
        }
    }
}

impl FromIterator<ToolGroup> for ToolGroups {
    fn from_iter<I: IntoIterator<Item = ToolGroup>>(groups: I) -> Self {
        let mut set = Self::new();
        set.extend(groups);
        set
    }
}

impl<const N: usize> From<[ToolGroup; N]> for ToolGroups {
    fn from(groups: [ToolGroup; N]) -> Self {
        groups.into_iter().collect()
    }
}

/// Ferramenta do próprio agente que carrega grupos do catálogo. Não é um
/// [`AiToolHandler`]: ela muda o que a sessão enxerga, não consulta nada.
pub const LOAD_TOOLS: &str = "load_tools";

/// Quais ferramentas a sessão pode usar e quais pedem confirmação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolPolicy {
    pub allow_active: bool,
    pub allow_actions: bool,
    /// Pede confirmação também antes dos testes ativos.
    pub confirm_active: bool,
    /// Há alguém do outro lado para responder (chat, não rotina automática).
    pub interactive: bool,
    /// Ações em container: desligadas, com confirmação ou automáticas.
    pub container_actions: AiContainerActionMode,
}

impl ToolPolicy {
    /// Só leitura: o que as rotinas automáticas (resumos) podem usar.
    #[must_use]
    pub const fn passive() -> Self {
        Self {
            allow_active: false,
            allow_actions: false,
            confirm_active: false,
            interactive: false,
            container_actions: AiContainerActionMode::Off,
        }
    }

    #[must_use]
    pub const fn from_settings(settings: &AiSettings) -> Self {
        Self {
            allow_active: settings.allow_active_tools,
            allow_actions: settings.allow_actions,
            confirm_active: settings.require_tool_confirmation,
            interactive: true,
            container_actions: settings.container_actions,
        }
    }

    #[must_use]
    pub const fn allows(self, kind: ToolKind) -> bool {
        match kind {
            ToolKind::Passive => true,
            ToolKind::Active => self.allow_active,
            ToolKind::Action => self.allow_actions,
            ToolKind::Interactive => self.interactive,
            ToolKind::ContainerAction => {
                !matches!(self.container_actions, AiContainerActionMode::Off)
            }
        }
    }

    #[must_use]
    pub const fn needs_confirmation(self, kind: ToolKind) -> bool {
        match kind {
            ToolKind::Passive | ToolKind::Interactive => false,
            ToolKind::Active => self.confirm_active,
            ToolKind::Action => true,
            ToolKind::ContainerAction => {
                !matches!(self.container_actions, AiContainerActionMode::Auto)
            }
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
    /// A resposta agora é do usuário: o agente encerra a rodada sem chamar o
    /// provedor de novo.
    pub ends_turn: bool,
}

impl ToolOutput {
    #[must_use]
    pub const fn data(data: Value) -> Self {
        Self {
            data,
            chart: None,
            ends_turn: false,
        }
    }

    #[must_use]
    pub const fn with_chart(data: Value, chart: AiChart) -> Self {
        Self {
            data,
            chart: Some(chart),
            ends_turn: false,
        }
    }

    /// Algo foi perguntado ao usuário; a conversa espera a resposta dele.
    #[must_use]
    pub const fn awaiting_user(data: Value) -> Self {
        Self {
            data,
            chart: None,
            ends_turn: true,
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

    /// Grupo do catálogo. Testes ativos e ações têm grupo próprio; o resto é
    /// básico até dizer o contrário.
    fn group(&self) -> ToolGroup {
        match self.kind() {
            ToolKind::Active => ToolGroup::Diagnostics,
            ToolKind::Action => ToolGroup::Actions,
            ToolKind::ContainerAction => ToolGroup::Docker,
            ToolKind::Passive | ToolKind::Interactive => ToolGroup::Core,
        }
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
        Box::new(docker::DockerHosts),
        Box::new(docker::DockerContainers),
        Box::new(docker::DockerUsage),
        Box::new(docker::DockerContainerAction),
        Box::new(platform::PlatformStatus),
        Box::new(platform::Topology),
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
        Box::new(alert_rules::AlertRulesGuide),
        Box::new(alert_rules::ListAlertRules),
        Box::new(alert_rules::ExplainAlert),
        Box::new(alert_rules::CreateAlertRule),
        Box::new(alert_rules::DeleteAlertRule),
        Box::new(ask::AskUser),
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

    /// Contratos do núcleo, o `load_tools` e depois os grupos carregados, na
    /// ordem em que entraram.
    ///
    /// A lista só cresce pelo fim: o núcleo e o `load_tools` (com o catálogo
    /// inteiro no `enum`, carregado ou não) não mudam, então o prefixo que o
    /// provedor guardou em cache continua batendo depois de um `load_tools`.
    /// Só quando não sobra nada a carregar o `load_tools` sai.
    #[must_use]
    pub fn definitions_for(&self, loaded: &ToolGroups) -> Vec<AiTool> {
        let mut tools = self.group_definitions(ToolGroup::Core);
        let groups = self.available_groups();
        let available: Vec<&str> = groups.iter().map(|group| group.id()).collect();
        if groups.iter().any(|group| !loaded.contains(group)) {
            tools.push(AiTool {
                r#type: "function".to_string(),
                function: AiToolFunction {
                    name: LOAD_TOOLS.to_string(),
                    description: "Carrega grupos de ferramentas do catálogo (listados no prompt) para a próxima rodada. Peça todos os grupos de que precisar numa chamada só.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "groups": { "type": "array", "items": { "type": "string", "enum": available } }
                        },
                        "required": ["groups"]
                    }),
                },
            });
        }
        for group in loaded.iter().filter(|group| *group != ToolGroup::Core) {
            tools.extend(self.group_definitions(group));
        }
        tools
    }

    fn group_definitions(&self, group: ToolGroup) -> Vec<AiTool> {
        self.handlers
            .iter()
            .filter(|handler| handler.group() == group)
            .map(|handler| handler.definition())
            .collect()
    }

    /// Grupos sob demanda que a política desta sessão libera.
    #[must_use]
    pub fn available_groups(&self) -> Vec<ToolGroup> {
        ToolGroup::DEFERRED
            .into_iter()
            .filter(|group| {
                self.handlers
                    .iter()
                    .any(|handler| handler.group() == *group)
            })
            .collect()
    }

    /// Nomes das ferramentas de um grupo, para o catálogo do prompt.
    #[must_use]
    pub fn tool_names(&self, group: ToolGroup) -> Vec<&'static str> {
        self.handlers
            .iter()
            .filter(|handler| handler.group() == group)
            .map(|handler| handler.name())
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

    /// Executa uma chamada da IA sem usuário identificado.
    ///
    /// # Errors
    ///
    /// Os mesmos de [`Self::execute_as`].
    pub async fn execute(
        &self,
        ctx: &AppContext,
        name: &str,
        arguments_json: &str,
    ) -> AppResult<ToolOutput> {
        self.execute_as(ctx, name, arguments_json, &AuditActor::default())
            .await
    }

    /// Executa uma chamada da IA dentro do chat, em nome de quem conversa
    /// (a ação automática fica na auditoria dele). O que pede confirmação não
    /// roda aqui — só por [`Self::execute_confirmed`].
    ///
    /// # Errors
    ///
    /// Ferramenta fora do registro, que exige confirmação, argumento inválido
    /// ou falha do banco.
    pub async fn execute_as(
        &self,
        ctx: &AppContext,
        name: &str,
        arguments_json: &str,
        actor: &AuditActor,
    ) -> AppResult<ToolOutput> {
        let handler = self.handler(name)?;
        if self.policy.needs_confirmation(handler.kind()) {
            return Err(AppError::validation(format!(
                "A ferramenta {name} exige confirmação do usuário"
            )));
        }
        let args = ToolArgs::parse(arguments_json).with_actor(actor.clone());
        handler.execute(ctx, &args).await
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
        if matches!(handler.kind(), ToolKind::Passive | ToolKind::Interactive) {
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
        assert!(
            !nomes.contains(&"ask_user".to_string()),
            "rotina automática não tem a quem perguntar"
        );

        let todas = ToolRegistry::new(ToolPolicy {
            allow_active: true,
            allow_actions: true,
            confirm_active: false,
            interactive: true,
            container_actions: AiContainerActionMode::Confirm,
        });
        assert_eq!(todas.definitions().len(), all_handlers().len());
    }

    #[test]
    fn catalogo_sob_demanda_manda_o_nucleo_e_so_o_que_foi_carregado() {
        let registro = ToolRegistry::new(ToolPolicy {
            allow_active: true,
            allow_actions: false,
            confirm_active: false,
            interactive: true,
            container_actions: AiContainerActionMode::Confirm,
        });
        let nomes = |carregados: &ToolGroups| -> Vec<String> {
            registro
                .definitions_for(carregados)
                .into_iter()
                .map(|tool| tool.function.name)
                .collect()
        };

        let nucleo = nomes(&ToolGroups::new());
        assert!(nucleo.contains(&"grep".to_string()));
        assert!(nucleo.contains(&"ask_user".to_string()));
        assert!(!nucleo.contains(&"chart_monitor_latency".to_string()));
        assert!(nucleo.contains(&LOAD_TOOLS.to_string()));
        assert!(
            !registro.available_groups().contains(&ToolGroup::Actions),
            "ação desligada não entra nem no catálogo"
        );

        let com_graficos = nomes(&ToolGroups::from([ToolGroup::Charts]));
        assert!(com_graficos.contains(&"chart_monitor_latency".to_string()));
        assert_eq!(
            com_graficos[..nucleo.len()],
            nucleo[..],
            "grupo carregado entra no fim: o prefixo em cache não muda"
        );

        let depois = nomes(&ToolGroups::from([ToolGroup::Charts, ToolGroup::History]));
        assert_eq!(depois[..com_graficos.len()], com_graficos[..]);

        let tudo: ToolGroups = registro.available_groups().into_iter().collect();
        assert!(
            !nomes(&tudo).contains(&LOAD_TOOLS.to_string()),
            "nada a carregar, sem load_tools"
        );
    }

    #[test]
    fn grupos_guardam_a_ordem_de_carga_sem_repetir() {
        let mut grupos = ToolGroups::from_ids(&["docker", "history", "inventado", "DOCKER"]);
        assert!(!grupos.insert(ToolGroup::History));
        grupos.insert(ToolGroup::Charts);
        assert_eq!(grupos.ids(), vec!["docker", "history", "charts"]);
    }

    #[test]
    fn nucleo_cabe_num_orcamento_pequeno() {
        let registro = ToolRegistry::new(ToolPolicy::from_settings(&AiSettings::default()));
        let tamanho: usize = registro
            .definitions_for(&ToolGroups::new())
            .iter()
            .map(|tool| serde_json::to_string(tool).unwrap().len())
            .sum();
        assert!(
            tamanho < 7_500,
            "o núcleo vai em toda rodada: {tamanho} caracteres"
        );
    }

    #[test]
    fn acao_em_container_segue_o_modo_configurado() {
        let modo = |container_actions| {
            ToolRegistry::new(ToolPolicy {
                container_actions,
                ..ToolPolicy::from_settings(&AiSettings::default())
            })
        };
        let padrao = ToolRegistry::new(ToolPolicy::from_settings(&AiSettings::default()));
        assert!(
            padrao.needs_confirmation("docker_container_action"),
            "o padrão é pedir permissão"
        );

        let auto = modo(AiContainerActionMode::Auto);
        assert!(!auto.needs_confirmation("docker_container_action"));
        assert!(auto
            .tool_names(ToolGroup::Docker)
            .contains(&"docker_container_action"));

        let desligado = modo(AiContainerActionMode::Off);
        assert!(!desligado
            .tool_names(ToolGroup::Docker)
            .contains(&"docker_container_action"));

        let rotina = ToolRegistry::new(ToolPolicy::passive());
        assert!(
            !rotina
                .tool_names(ToolGroup::Docker)
                .contains(&"docker_container_action"),
            "rotina automática nunca mexe em container"
        );
    }

    #[test]
    fn acoes_sempre_pedem_confirmacao_e_testes_ativos_so_quando_configurado() {
        let sem = ToolPolicy {
            allow_active: true,
            allow_actions: true,
            confirm_active: false,
            interactive: true,
            container_actions: AiContainerActionMode::Confirm,
        };
        assert!(!sem.needs_confirmation(ToolKind::Passive));
        assert!(!sem.needs_confirmation(ToolKind::Active));
        assert!(!sem.needs_confirmation(ToolKind::Interactive));
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
