//! O contrato das ferramentas da IA (ADR 017, ponto de extensão
//! `AiToolHandler`) e o registro que as libera por sessão.
//!
//! Cada ferramenta é um [`AiToolHandler`]: declara o próprio contrato (nome,
//! descrição e JSON Schema dos argumentos), a área do produto a que pertence e
//! sabe se executar. O contrato mora no núcleo para que cada área contribua as
//! suas; o catálogo completo — a ordem em que a IA as vê — é montado no ponto
//! de montagem (`src/registries.rs`) e instalado com [`install_catalog`] pelo
//! `after_context`, em todo processo (servidor, `scheduler`, `task`).

use std::sync::OnceLock;

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

use super::{
    args::ToolArgs,
    automation::AiAutomationMode,
    chart::AiChart,
    function::{AiTool, AiToolFunction},
    risk::{AiToolRisk, Approval, RiskAssessment},
};
use crate::{
    areas::{Area, AreaSet},
    audit::AuditActor,
    roles::AreaAccess,
    shared::errors::{AppError, AppResult},
};

/// O que as configurações da IA dizem sobre as ferramentas do chat — o
/// assistente implementa para as configurações dele.
pub trait ToolSettings {
    fn allow_active_tools(&self) -> bool;
    fn action_mode(&self) -> AiAutomationMode;
    fn require_tool_confirmation(&self) -> bool;
    fn container_actions(&self) -> AiAutomationMode;
}

/// Monta o catálogo completo, na ordem em que a IA vê as ferramentas.
pub type Catalog = fn() -> Vec<Box<dyn AiToolHandler>>;

static CATALOG: OnceLock<Catalog> = OnceLock::new();

/// Instala o catálogo do processo. Só o primeiro vale: o ponto de montagem
/// chama uma vez por processo, e chamar de novo não muda nada.
pub fn install_catalog(catalog: Catalog) {
    let _ = CATALOG.set(catalog);
}

/// Catálogo completo, na ordem em que a IA as vê. Sem catálogo instalado,
/// nenhuma ferramenta.
fn all_handlers() -> Vec<Box<dyn AiToolHandler>> {
    CATALOG.get().map_or_else(Vec::new, |catalog| catalog())
}

/// O que a ferramenta faz com o mundo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    /// Consulta o banco ou a documentação. Sempre disponível.
    Passive,
    /// Envia pacotes (ping, traceroute, scan). Depende de `allow_active_tools`.
    Active,
    /// Muda o estado do sistema (silenciar alerta, criar monitor). Segue o
    /// modo das ações: desligado, com confirmação ou automático por risco.
    Action,
    /// Conversa com o usuário (perguntar antes de prosseguir). Só no chat —
    /// nas rotinas automáticas não há ninguém para responder.
    Interactive,
    /// Inicia, para ou reinicia container. Tem configuração própria
    /// ([`AiAutomationMode`]): desligado, com confirmação (padrão) ou
    /// automático por risco.
    ContainerAction,
    /// Acessa um equipamento (SSH/HTTP/Telnet) ou grava um plugin. **Sempre**
    /// pede confirmação, a cada chamada — exceto com o modo "Aceitar
    /// automaticamente" ligado para aquela conversa e aquele equipamento
    /// (`plugins::auto_accept`), decidido no laço do agente.
    DeviceAccess,
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
    Devices,
    Audit,
    Schedules,
    Dashboard,
}

impl ToolGroup {
    /// Os grupos que podem ser carregados sob demanda, na ordem do catálogo.
    pub const DEFERRED: [Self; 13] = [
        Self::History,
        Self::Analysis,
        Self::Charts,
        Self::Logs,
        Self::Docker,
        Self::Platform,
        Self::Diagnostics,
        Self::Actions,
        Self::AlertRules,
        Self::Devices,
        Self::Audit,
        Self::Schedules,
        Self::Dashboard,
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
            Self::Devices => "devices",
            Self::Audit => "audit",
            Self::Schedules => "schedules",
            Self::Dashboard => "dashboard",
        }
    }

    /// Para que serve, na linha do catálogo.
    #[must_use]
    pub const fn purpose(self) -> &'static str {
        match self {
            Self::Core => "consultas básicas",
            Self::History => "histórico",
            Self::Analysis => "causa raiz e incidente",
            Self::Charts => "gráficos",
            Self::Logs => "logs",
            Self::Docker => "containers",
            Self::Diagnostics => "testes ativos",
            Self::Actions => "ações",
            Self::AlertRules => "regras de alerta",
            Self::Platform => "agentes, VPN, topologia, descoberta e redes",
            Self::Devices => "equipamento por SSH/HTTP e plugins",
            Self::Audit => "auditoria: quem mudou o quê",
            Self::Schedules => "tarefas agendadas",
            Self::Dashboard => "cards do dashboard",
        }
    }

    /// Afirmação sim/não que o Decisor avalia sobre a pergunta do usuário. Em
    /// inglês: é a língua em que os modelos de decisão foram treinados, e o
    /// roteador do Decisor lê a língua da pergunta, não a da afirmação.
    #[must_use]
    pub const fn intent(self) -> &'static str {
        match self {
            Self::Core => "The message asks about devices, monitors, current alerts or documentation.",
            Self::History => "Answering requires uptime, availability or metrics over a past period of time.",
            Self::Analysis => "The message asks why something happened, its root cause, a baseline or an incident timeline.",
            Self::Charts => "The user wants a chart or graph of latency, traffic or resource usage.",
            Self::Logs => "Answering requires reading or searching system, device or syslog logs.",
            Self::Docker => "The message is about Docker containers, Docker hosts or their resource usage or logs.",
            Self::Diagnostics => "The user wants a live network test such as ping, traceroute, port scan or DNS lookup.",
            Self::Actions => "The user wants to change something: acknowledge or silence an alert, schedule maintenance or create a monitor.",
            Self::AlertRules => "The message is about alert rules: where an alert comes from, thresholds, or creating, deleting or toggling a rule.",
            Self::Platform => "The message is about monitoring agents, the VPN, network topology, discovery or networks.",
            Self::Devices => "The user wants to access a device over SSH or its web interface, run a command or change a setting on it, install a package, or list, create, test or use a device plugin, app, driver or script.",
            Self::Audit => "The message is about the audit log: who changed what and when, or which configuration change came before a problem.",
            Self::Schedules => "The user wants something to run automatically at a given time or periodically, or asks about scheduled tasks: list, change, pause, run or delete them.",
            Self::Dashboard => "The user wants to see or change the dashboard layout: add, remove, move, resize or configure dashboard cards or widgets.",
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
    /// Ações (alertas, manutenção, monitores, regras).
    pub actions: AiAutomationMode,
    /// Pede confirmação também antes dos testes ativos.
    pub confirm_active: bool,
    /// Há alguém do outro lado para responder (chat, não rotina automática).
    pub interactive: bool,
    /// Ações em container: desligadas, com confirmação ou automáticas.
    pub container_actions: AiAutomationMode,
    /// Áreas do produto ligadas (ADR 017): ferramenta de área desligada não
    /// entra no catálogo — nem no chat, nem nas rotinas, nem nas tarefas.
    pub areas: AreaSet,
    /// Áreas em que a sessão pode mudar algo (ADR 019): fora delas a IA só
    /// consulta — as ferramentas de ação daquela área não entram.
    pub writable: AreaSet,
}

impl ToolPolicy {
    /// Só leitura: o que as rotinas automáticas (resumos) podem usar.
    #[must_use]
    pub const fn passive() -> Self {
        Self {
            allow_active: false,
            actions: AiAutomationMode::Off,
            confirm_active: false,
            interactive: false,
            container_actions: AiAutomationMode::Off,
            areas: AreaSet::ALL,
            writable: AreaSet::ALL,
        }
    }

    /// Tarefa agendada: roda o que foi decidido ao agendar, sem perguntar e
    /// sem ninguém para responder ([`UnattendedTools`]).
    #[must_use]
    pub const fn unattended() -> Self {
        Self {
            allow_active: true,
            actions: AiAutomationMode::Auto,
            confirm_active: false,
            interactive: false,
            container_actions: AiAutomationMode::Auto,
            areas: AreaSet::ALL,
            writable: AreaSet::ALL,
        }
    }

    /// O chat, com o que as configurações da IA liberam.
    #[must_use]
    pub fn from_settings(settings: &impl ToolSettings) -> Self {
        Self {
            allow_active: settings.allow_active_tools(),
            actions: settings.action_mode(),
            confirm_active: settings.require_tool_confirmation(),
            interactive: true,
            container_actions: settings.container_actions(),
            areas: AreaSet::ALL,
            writable: AreaSet::ALL,
        }
    }

    /// A mesma política restrita às áreas ligadas. Sem Servidores, as ações em
    /// container também saem — e com elas a regra de containers do prompt.
    #[must_use]
    pub const fn with_areas(mut self, areas: AreaSet) -> Self {
        self.areas = areas;
        if !areas.contains(Area::Servers) {
            self.container_actions = AiAutomationMode::Off;
        }
        self
    }

    /// A mesma política restrita ao que o usuário da sessão pode (ADR 019):
    /// só as áreas que ele enxerga, e ações só onde ele pode mudar algo.
    #[must_use]
    pub fn with_access(self, access: &AreaAccess) -> Self {
        let mut policy = self.with_areas(self.areas.intersect(access.visible()));
        policy.writable = policy.writable.intersect(access.writable());
        if !policy.writable.contains(Area::Servers) {
            policy.container_actions = AiAutomationMode::Off;
        }
        policy
    }

    /// A ferramenta entra no catálogo desta sessão? A área precisa estar
    /// visível; se a ferramenta muda algo, a área precisa aceitar escrita.
    #[must_use]
    pub const fn admits(self, kind: ToolKind, area: Area) -> bool {
        let writes = matches!(
            kind,
            ToolKind::Action | ToolKind::ContainerAction | ToolKind::DeviceAccess
        );
        self.areas.contains(area) && (!writes || self.writable.contains(area))
    }

    /// O modo que vale para o tipo de ferramenta.
    #[must_use]
    pub const fn mode(self, kind: ToolKind) -> AiAutomationMode {
        match kind {
            ToolKind::Passive | ToolKind::Interactive => AiAutomationMode::Auto,
            ToolKind::Active => {
                if !self.allow_active {
                    AiAutomationMode::Off
                } else if self.confirm_active {
                    AiAutomationMode::Confirm
                } else {
                    AiAutomationMode::Auto
                }
            }
            ToolKind::Action => self.actions,
            ToolKind::ContainerAction => self.container_actions,
            // ADR 012: todo acesso a equipamento é confirmado; o único atalho
            // é o "Aceitar automaticamente" da conversa, decidido no agente.
            ToolKind::DeviceAccess => AiAutomationMode::Confirm,
        }
    }

    #[must_use]
    pub const fn allows(self, kind: ToolKind) -> bool {
        match kind {
            ToolKind::Interactive => self.interactive,
            // Só no chat: rotina automática não tem quem aprove cada acesso.
            ToolKind::DeviceAccess => self.interactive && self.allow_active,
            _ => !matches!(self.mode(kind), AiAutomationMode::Off),
        }
    }

    /// Pede confirmação sempre, qualquer que seja o risco da chamada.
    #[must_use]
    pub const fn needs_confirmation(self, kind: ToolKind) -> bool {
        matches!(self.mode(kind), AiAutomationMode::Confirm)
    }

    /// Roda ou pergunta: no modo automático, só o risco alto pergunta.
    #[must_use]
    pub const fn approval(self, kind: ToolKind, risk: AiToolRisk) -> Approval {
        match (self.mode(kind), risk) {
            (AiAutomationMode::Auto, AiToolRisk::Low | AiToolRisk::Medium) => Approval::Run,
            _ => Approval::Ask,
        }
    }
}

/// Licença para executar sem confirmação. Só [`ToolRegistry::assess`] a
/// cria, e ela vale para a ferramenta avaliada: não há como chegar à
/// execução sem passar pela política e pelo risco da chamada.
#[derive(Debug)]
pub struct Clearance {
    tool: &'static str,
}

/// O que a política decidiu sobre uma chamada.
#[derive(Debug)]
pub enum Verdict {
    Run(Clearance),
    Ask,
}

/// O risco de uma chamada e o veredito da política.
#[derive(Debug)]
pub struct Assessment {
    pub kind: ToolKind,
    pub risk: RiskAssessment,
    pub verdict: Verdict,
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

/// Erro de validação volta à IA como dado, para ela corrigir.
///
/// # Errors
///
/// O erro que não é de validação (banco, rede).
pub fn as_output<T>(result: AppResult<T>) -> AppResult<Result<T, ToolOutput>> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(
            error @ (AppError::Validation(_)
            | AppError::NotFound(_)
            | AppError::Forbidden(_)
            | AppError::Conflict(_)
            | AppError::BusinessRule(_)),
        ) => Ok(Err(ToolOutput::not_found(error.to_string()))),
        Err(error) => Err(error),
    }
}

/// Começo da frase que a ferramenta devolve quando o argumento não serve.
pub const INVALID_ACTION_PREFIX: &str = "Ação inválida: ";

/// Extrai o motivo quando o preview de uma ação indica argumentos inválidos.
#[must_use]
pub fn invalid_action_reason(preview: &str) -> Option<&str> {
    preview.strip_prefix(INVALID_ACTION_PREFIX)
}

/// Texto do preview de uma ação ou o motivo da recusa de validação.
pub fn preview_or_error(result: Result<String, ToolOutput>) -> String {
    match result {
        Ok(text) => text,
        Err(output) => format!(
            "{INVALID_ACTION_PREFIX}{}",
            output.data["error"]
                .as_str()
                .unwrap_or("argumentos incompletos")
        ),
    }
}

#[async_trait]
pub trait AiToolHandler: Send + Sync {
    fn name(&self) -> &'static str;

    /// A área do produto a que a ferramenta pertence (ADR 017). Sem padrão de
    /// propósito: ferramenta nova que não declara a área não compila. Com a
    /// área desligada, a ferramenta sai do chat, das rotinas e das tarefas.
    fn area(&self) -> Area;

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
            ToolKind::DeviceAccess => ToolGroup::Devices,
            ToolKind::Passive | ToolKind::Interactive => ToolGroup::Core,
        }
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput>;

    /// Existe nesta instalação? Desligada (por exemplo, `AI_WEB_SEARCH=off`)
    /// fica fora de todo catálogo — chat, rotinas e tarefas agendadas — e não
    /// custa tokens. A política decide o resto.
    fn enabled(&self) -> bool {
        true
    }

    /// O risco desta chamada, com o motivo. Nunca falha: sem como avaliar
    /// melhor (alvo não encontrado, por exemplo), vale o padrão do tipo — e a
    /// execução recusa o argumento inválido do mesmo jeito.
    async fn risk(&self, _ctx: &AppContext, _args: &ToolArgs) -> RiskAssessment {
        RiskAssessment::for_kind(self.kind())
    }

    /// Host ou IP que o teste ativo vai alcançar; `None` quando a ferramenta
    /// não gera tráfego para um alvo. O agente confere a origem dele antes de
    /// executar ou pedir confirmação (`harness::grounding`).
    fn network_target(&self, _args: &ToolArgs) -> Option<String> {
        None
    }

    /// Frase que o usuário lê antes de confirmar ("Silenciar o alerta #12 por
    /// 60 min"). Só é chamada para ferramentas que pedem confirmação.
    async fn preview(&self, _ctx: &AppContext, _args: &ToolArgs) -> AppResult<String> {
        Ok(format!("Executar {}", self.name()))
    }

    /// Pode virar tarefa agendada ([`UnattendedTools`])? Quem pergunta ao
    /// usuário ou acessa equipamento já fica de fora pela política; aqui saem
    /// o que só faz sentido dentro de uma conversa (gráficos, guias) e as
    /// próprias tarefas — agendar um agendamento não leva a nada.
    fn schedulable(&self) -> bool {
        !matches!(self.group(), ToolGroup::Charts | ToolGroup::Schedules)
    }

    /// Só o administrador pode rodar (e, por isso, agendar).
    fn admin_only(&self) -> bool {
        self.kind() == ToolKind::ContainerAction
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

/// Os nomes das ferramentas de uma área — para as consequências de desligá-la
/// (tarefas agendadas que passariam a ser puladas).
#[must_use]
pub fn tool_names_in_area(area: Area) -> Vec<&'static str> {
    all_handlers()
        .into_iter()
        .filter(|handler| handler.area() == area)
        .map(|handler| handler.name())
        .collect()
}

/// A área de uma ferramenta pelo nome (`None` se ela não existe).
#[must_use]
pub fn area_of_tool(name: &str) -> Option<Area> {
    all_handlers()
        .into_iter()
        .find(|handler| handler.name() == name)
        .map(|handler| handler.area())
}

/// Contrato de uma ferramenta que pode virar tarefa agendada.
#[derive(Debug, Clone)]
pub struct ToolContract {
    pub name: &'static str,
    pub description: &'static str,
    pub group: ToolGroup,
    pub kind: ToolKind,
    pub admin_only: bool,
    /// A área do produto da ferramenta (ADR 017).
    pub area: Area,
    /// JSON Schema dos argumentos — o mesmo que a IA recebe.
    pub parameters: Value,
}

/// O catálogo das tarefas agendadas: as ferramentas que rodam sem ninguém
/// do outro lado.
///
/// A confirmação aconteceu quando a tarefa foi salva — na tela, ou no chat
/// com o pedido de confirmação de `create_scheduled_task` —, então a
/// execução não pergunta de novo. Por isso o catálogo nunca tem o que pede
/// confirmação a cada chamada (acesso a equipamento) nem o que conversa com
/// o usuário, e independe das configurações da IA: quem decidiu rodar foi a
/// pessoa que agendou, com o papel dela.
pub struct UnattendedTools {
    handlers: Vec<Box<dyn AiToolHandler>>,
}

impl Default for UnattendedTools {
    fn default() -> Self {
        Self::new()
    }
}

impl UnattendedTools {
    #[must_use]
    pub fn new() -> Self {
        Self::for_areas(AreaSet::ALL)
    }

    /// O catálogo só com as ferramentas das áreas ligadas (ADR 017).
    #[must_use]
    pub fn for_areas(areas: AreaSet) -> Self {
        let policy = ToolPolicy::unattended().with_areas(areas);
        let handlers = all_handlers()
            .into_iter()
            .filter(|handler| {
                handler.enabled()
                    && policy.allows(handler.kind())
                    && handler.schedulable()
                    && policy.admits(handler.kind(), handler.area())
            })
            .collect();
        Self { handlers }
    }

    fn handler(&self, name: &str) -> AppResult<&dyn AiToolHandler> {
        self.handlers
            .iter()
            .find(|handler| handler.name() == name)
            .map(AsRef::as_ref)
            .ok_or_else(|| {
                AppError::validation(format!("A ferramenta \"{name}\" não pode ser agendada"))
            })
    }

    fn contract_of(handler: &dyn AiToolHandler) -> ToolContract {
        ToolContract {
            name: handler.name(),
            description: handler.description(),
            group: handler.group(),
            kind: handler.kind(),
            admin_only: handler.admin_only(),
            area: handler.area(),
            parameters: handler.parameters(),
        }
    }

    /// Todo o catálogo, na ordem em que a IA o vê.
    #[must_use]
    pub fn contracts(&self) -> Vec<ToolContract> {
        self.handlers
            .iter()
            .map(|handler| Self::contract_of(handler.as_ref()))
            .collect()
    }

    /// O contrato de uma ferramenta agendável.
    ///
    /// # Errors
    ///
    /// `Validation` quando ela não existe ou não pode ser agendada.
    pub fn contract(&self, name: &str) -> AppResult<ToolContract> {
        self.handler(name).map(Self::contract_of)
    }

    /// Nomes agendáveis, para a mensagem de erro de quem errou o nome.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        self.handlers.iter().map(|handler| handler.name()).collect()
    }

    /// O risco de rodar a chamada.
    ///
    /// # Errors
    ///
    /// Ferramenta fora do catálogo.
    pub async fn risk(
        &self,
        ctx: &AppContext,
        name: &str,
        args: &ToolArgs,
    ) -> AppResult<RiskAssessment> {
        Ok(self.handler(name)?.risk(ctx, args).await)
    }

    /// O que a chamada vai fazer, na frase da confirmação. `None` para
    /// consultas, que não têm frase própria.
    ///
    /// # Errors
    ///
    /// Ferramenta fora do catálogo ou erro do banco.
    pub async fn preview(
        &self,
        ctx: &AppContext,
        name: &str,
        args: &ToolArgs,
    ) -> AppResult<Option<String>> {
        let handler = self.handler(name)?;
        if handler.kind() == ToolKind::Passive {
            return Ok(None);
        }
        handler.preview(ctx, args).await.map(Some)
    }

    /// Roda a ferramenta em nome do autor que vem em `args`.
    ///
    /// # Errors
    ///
    /// Ferramenta fora do catálogo, argumento inválido ou falha da execução.
    pub async fn execute(
        &self,
        ctx: &AppContext,
        name: &str,
        args: &ToolArgs,
    ) -> AppResult<ToolOutput> {
        self.handler(name)?.execute(ctx, args).await
    }
}

/// Ferramentas liberadas para uma sessão de chat.
pub struct ToolRegistry {
    handlers: Vec<Box<dyn AiToolHandler>>,
    policy: ToolPolicy,
}

impl ToolRegistry {
    /// Monta o registro só com o que existe na instalação e a política libera.
    #[must_use]
    pub fn new(policy: ToolPolicy) -> Self {
        let handlers = all_handlers()
            .into_iter()
            .filter(|handler| {
                handler.enabled()
                    && policy.allows(handler.kind())
                    && policy.admits(handler.kind(), handler.area())
            })
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

    /// Os contratos de um grupo, na ordem do catálogo.
    #[must_use]
    pub fn group_definitions(&self, group: ToolGroup) -> Vec<AiTool> {
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

    /// O grupo da ferramenta, se ela está no registro.
    #[must_use]
    pub fn group_of(&self, name: &str) -> Option<ToolGroup> {
        self.handler(name).ok().map(|handler| handler.group())
    }

    /// O tipo da ferramenta, se ela está no registro.
    #[must_use]
    pub fn kind_of(&self, name: &str) -> Option<ToolKind> {
        self.handler(name).ok().map(|handler| handler.kind())
    }

    /// Executa uma ferramenta de acesso a equipamento **já liberada** pelo
    /// modo "Aceitar automaticamente". Só vale para [`ToolKind::DeviceAccess`]:
    /// nenhuma outra ferramenta que pede confirmação passa por aqui.
    ///
    /// # Errors
    ///
    /// Ferramenta fora do registro ou de outro tipo, ou falha da execução.
    pub async fn execute_auto_approved(
        &self,
        ctx: &AppContext,
        name: &str,
        arguments: ToolArgs,
    ) -> AppResult<ToolOutput> {
        let handler = self.handler(name)?;
        if handler.kind() != ToolKind::DeviceAccess {
            return Err(AppError::validation(format!(
                "A ferramenta {name} não pode ser liberada automaticamente"
            )));
        }
        handler.execute(ctx, &arguments).await
    }

    /// O alvo de rede da chamada, se a ferramenta tem um.
    #[must_use]
    pub fn network_target(&self, name: &str, arguments_json: &str) -> Option<String> {
        self.handler(name)
            .ok()?
            .network_target(&ToolArgs::parse(arguments_json))
    }

    /// Se a ferramenta pede confirmação sempre, qualquer que seja o risco.
    #[must_use]
    pub fn needs_confirmation(&self, name: &str) -> bool {
        self.handler(name)
            .is_ok_and(|handler| self.policy.needs_confirmation(handler.kind()))
    }

    /// Avalia o risco da chamada e decide se ela roda sem confirmação.
    ///
    /// # Errors
    ///
    /// Ferramenta fora do registro.
    pub async fn assess(
        &self,
        ctx: &AppContext,
        name: &str,
        args: &ToolArgs,
    ) -> AppResult<Assessment> {
        let handler = self.handler(name)?;
        let kind = handler.kind();
        let risk = handler.risk(ctx, args).await;
        let verdict = match self.policy.approval(kind, risk.level) {
            Approval::Run => Verdict::Run(Clearance {
                tool: handler.name(),
            }),
            Approval::Ask => Verdict::Ask,
        };
        Ok(Assessment {
            kind,
            risk,
            verdict,
        })
    }

    /// Executa uma chamada liberada por [`Self::assess`].
    ///
    /// # Errors
    ///
    /// Argumento inválido ou falha da execução.
    pub async fn execute_cleared(
        &self,
        ctx: &AppContext,
        clearance: Clearance,
        args: ToolArgs,
    ) -> AppResult<ToolOutput> {
        self.handler(clearance.tool)?.execute(ctx, &args).await
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
        self.execute_in(
            ctx,
            name,
            ToolArgs::parse(arguments_json).with_actor(actor.clone()),
        )
        .await
    }

    /// Como [`Self::execute_as`], com os argumentos já montados (autor e
    /// conversa).
    ///
    /// # Errors
    ///
    /// Os mesmos de [`Self::execute_as`].
    pub async fn execute_in(
        &self,
        ctx: &AppContext,
        name: &str,
        args: ToolArgs,
    ) -> AppResult<ToolOutput> {
        match self.assess(ctx, name, &args).await?.verdict {
            Verdict::Run(clearance) => self.execute_cleared(ctx, clearance, args).await,
            Verdict::Ask => Err(AppError::validation(format!(
                "A ferramenta {name} exige confirmação do usuário"
            ))),
        }
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

/// Marca no registro de auditoria que a ação veio do chat.
pub const VIA_AI: &str = "via Assistente IA";

/// Registra na auditoria, em nome de quem conversa, uma ação feita por uma
/// ferramenta. Falha da auditoria não desfaz a ação.
pub async fn audit(ctx: &AppContext, args: &ToolArgs, entry: crate::audit::AuditEntryInput) {
    let _ = crate::audit::AuditService::new(&ctx.db)
        .log(args.actor().clone(), entry)
        .await;
}

#[cfg(test)]
mod access_tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::roles::Role;

    #[test]
    fn leitor_na_area_ve_as_consultas_dela_mas_nao_as_acoes() {
        let acesso = AreaAccess::new(
            Role::Operator,
            BTreeMap::from([
                (Area::Servers, Role::Viewer),
                (Area::Network, Role::Operator),
            ]),
        );
        let politica = ToolPolicy::passive().with_access(&acesso);
        assert!(politica.admits(ToolKind::Passive, Area::Servers));
        assert!(!politica.admits(ToolKind::ContainerAction, Area::Servers));
        assert!(!politica.admits(ToolKind::Action, Area::Servers));
        assert!(politica.admits(ToolKind::Action, Area::Network));
        assert!(
            !politica.admits(ToolKind::Passive, Area::Vpn),
            "sem acesso à VPN"
        );
        assert_eq!(politica.container_actions, AiAutomationMode::Off);
    }
}
