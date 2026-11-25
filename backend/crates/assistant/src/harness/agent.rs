//! Orquestrador do Agent Harness (ReAct Loop) da IA.
//!
//! Coordena a troca de mensagens com o provedor de IA e a execução iterativa de ferramentas.

use netmonitor_core::areas::AreaSet;
use netmonitor_core::roles::AreaAccess;
use std::{
    collections::HashSet,
    pin::Pin,
    time::{Duration, Instant},
};

use futures::{Future, Stream, StreamExt};
use loco_rs::prelude::AppContext;
use serde::Serialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    approval::{self, ActionReviewer, GateDecision, RunPlan},
    compaction::{
        call_key, estimate_history, estimate_messages, estimate_tokens, fallback_summary,
        fold_count, prune_tool_results, small_window_notice, summarize, supersede_repeated_results,
        worth_folding, ContextBudget,
    },
    grounding::{
        invented_target, sanitize_ask_user_arguments, AnswerCheck, SeenResults, ANSWER_CHECKED,
        ANSWER_CHECK_NOTICE,
    },
    prompt::{build_small_talk_prompt, build_system_prompt, build_turn_context, ChatContext},
    tools::{
        invalid_action_reason, target_device_id, AiToolRisk, RiskAssessment, ToolArgs, ToolGroup,
        ToolGroups, ToolKind, ToolPolicy, ToolRegistry, LOAD_TOOLS,
    },
    turn::{compact_for_model, context_groups, is_small_talk, question_groups, requested_groups},
};
use crate::{
    approvals::{self, ApprovalRequest},
    central::Central,
    context_window::{self, ContextWindow},
    decider::{runtime::DeciderRuntime, tool_routing},
    drivers::traits::{
        AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool, AiToolCall, AiUsage,
    },
    dtos::{AiChart, ChatMessageInput, ChatStreamRequest},
    mentions, screens,
    settings::AiSettings,
};
use netmonitor_core::{
    audit::AuditActor,
    shared::errors::{AppError, AppResult},
};

/// O erro de uma ferramenta, para o modelo e para a tela. Falta de acesso
/// leva o equipamento e o transporte: o cartão oferece cadastrá-lo ali mesmo.
fn tool_error(central: Option<&Central>, err: &AppError) -> serde_json::Value {
    let mut value = json!({ "error": err.to_string() });
    if let Some((device_id, kind)) = central.and_then(|central| central.missing_device_access(err))
    {
        value["missingAccess"] = json!({ "deviceId": device_id, "transport": kind });
    }
    value
}

/// Rodadas de raciocínio (chamada ao provedor + ferramentas) por pergunta.
pub const MAX_ITERATIONS: usize = 6;

/// Se ainda cabe uma rodada. A conferência da resposta final ganha uma rodada
/// além do teto só quando foi pedida na última rodada, e uma vez só.
fn has_round_left(round: usize, retry_on_last: bool, retry_used: bool) -> bool {
    round < MAX_ITERATIONS || (retry_on_last && !retry_used)
}

/// Mensagens anteriores enviadas ao provedor. O histórico inteiro seria
/// reenviado a cada pergunta; as últimas trocas bastam para manter o assunto.
pub const MAX_HISTORY_MESSAGES: usize = 16;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HarnessEvent {
    TextDelta {
        content: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
        /// Acesso a equipamento liberado pelo modo "Aceitar automaticamente":
        /// a tela marca o card como executado sem confirmação.
        #[serde(rename = "autoApproved", skip_serializing_if = "std::ops::Not::not")]
        auto_approved: bool,
        /// Ação ou teste que o modo automático rodou sem perguntar: o risco
        /// que a regra viu, para a tela dizer por que não perguntou.
        #[serde(skip_serializing_if = "Option::is_none")]
        risk: Option<AiToolRisk>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    ToolResult {
        id: String,
        name: String,
        result: serde_json::Value,
        /// Gráfico para a tela; a IA recebe só `result`.
        #[serde(skip_serializing_if = "Option::is_none")]
        chart: Option<AiChart>,
    },
    /// A ferramenta pede confirmação: a tela mostra `summary` com os botões
    /// e, confirmando, chama `POST /api/ai/tools/execute`.
    ConfirmationRequired {
        id: String,
        name: String,
        arguments: serde_json::Value,
        summary: String,
        risk: AiToolRisk,
        /// Por que pergunta ("Exclui a regra…", "O Decisor avaliou…").
        reason: String,
    },
    /// Métricas da resposta: tokens somados de todas as rodadas, o modelo
    /// que respondeu, o tempo gasto e quanto da janela a conversa ocupa.
    #[serde(rename_all = "camelCase")]
    Usage {
        prompt_tokens: u64,
        completion_tokens: u64,
        /// Parte de `prompt_tokens` servida do cache do provedor (cobrada
        /// com desconto). Zero quando o provedor não informa.
        cached_tokens: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        /// Grupos de ferramentas carregados, na ordem: a tela devolve na
        /// próxima pergunta para a lista de ferramentas não mudar (cache).
        tool_groups: Vec<String>,
        /// Tokens da última rodada (entrada + saída) menos o tráfego desta
        /// resposta que não volta no histórico (chamadas e resultados de
        /// ferramenta, conferência): o tamanho da conversa que a próxima
        /// pergunta vai carregar.
        context_tokens: u64,
        /// Janela do modelo; `None` sem modelo conhecido.
        #[serde(skip_serializing_if = "Option::is_none")]
        context_window: Option<u64>,
        /// A janela veio do provedor (`true`) ou é estimada pelo nome.
        context_window_reported: bool,
        /// Tempo do provedor escrevendo (do primeiro pedaço ao último, somado
        /// das rodadas) — a base do tokens/s, sem a espera nem as ferramentas.
        generation_ms: u64,
        /// Tempo total da resposta, ferramentas incluídas.
        duration_ms: u64,
    },
    /// Algo que o usuário precisa saber sobre a resposta (janela pequena
    /// demais para as ferramentas, por exemplo). Não interrompe nada.
    Notice {
        message: String,
        /// Resolve-se nas configurações da IA (janela pequena, modelo
        /// trocado por limite de taxa): a tela oferece o atalho para lá.
        settings: bool,
        /// Aviso que muda de estado ("conferindo" → "conferida"): o novo
        /// com a mesma chave substitui o anterior na tela, em vez de empilhar.
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<&'static str>,
    },
    /// O texto da última rodada (o que veio depois da última ferramenta)
    /// citou recurso que nada mostrou e foi descartado: a tela apaga só esse
    /// trecho; a versão conferida vem em seguida.
    Retract,
    /// As mensagens antigas viraram resumo para caber na janela. A tela
    /// guarda o resumo e passa a mandar só o que veio depois.
    #[serde(rename_all = "camelCase")]
    ContextCompacted {
        summary: String,
        /// Quantas mensagens do início do pedido o resumo cobre.
        folded_messages: usize,
        tokens_before: u64,
        tokens_after: u64,
        /// `false` quando o provedor não resumiu e as mensagens só saíram.
        summarized: bool,
    },
    Done,
    Error {
        message: String,
    },
}

pub type HarnessEventStream = Pin<Box<dyn Stream<Item = HarnessEvent> + Send>>;

/// As últimas `limit` mensagens, começando sempre por uma do usuário — uma
/// resposta da IA sem a pergunta que a originou só confunde o modelo.
#[must_use]
pub fn recent_history(messages: Vec<ChatMessageInput>, limit: usize) -> Vec<ChatMessageInput> {
    let skip = messages.len().saturating_sub(limit);
    messages
        .into_iter()
        .skip(skip)
        .skip_while(|message| message.role != "user")
        .collect()
}

/// Mensagens enviadas numa troca de cortesia: não há o que lembrar.
const SMALL_TALK_HISTORY: usize = 4;

/// Como a sessão recebe os contratos das ferramentas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolLoading {
    /// Todos de saída — rotinas automáticas, onde ninguém espera uma rodada a mais.
    Everything,
    /// O núcleo e o que a pergunta pede; o resto a IA carrega com `load_tools`.
    /// Cortesia não recebe ferramenta nenhuma.
    OnDemand,
}

/// Uma sessão do agente: a pergunta, as ferramentas liberadas e o contexto.
pub struct AgentRequest {
    pub messages: Vec<ChatMessageInput>,
    pub context: ChatContext,
    pub policy: ToolPolicy,
    pub tool_loading: ToolLoading,
    pub memory: ConversationMemory,
    /// Quem conversa: a ação que roda sem confirmação fica na auditoria dele.
    pub actor: AuditActor,
}

/// O que a tela sabe da conversa além das mensagens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConversationMemory {
    /// Resumo das mensagens compactadas antes (não vêm mais em `messages`).
    pub summary: Option<String>,
    /// Tamanho da conversa medido pelo provedor na resposta anterior.
    pub last_context_tokens: Option<u64>,
    /// Janela e modelo da resposta anterior — conta quando o configurado é um
    /// roteador, que pode ter respondido com um modelo de janela menor.
    pub last_context_window: Option<u64>,
    pub last_model: Option<String>,
    /// Compactar agora, mesmo cabendo (pedido explícito da tela).
    pub force_compaction: bool,
    /// Grupos de ferramentas que a conversa já carregou, na ordem.
    pub tool_groups: ToolGroups,
    /// Identificador da conversa na tela (modo automático dos plugins).
    pub conversation_key: Option<String>,
}

/// Modelos que escolhem outro modelo por pergunta.
fn is_router(model: &str) -> bool {
    model.starts_with("openrouter/")
}

/// Seção do prompt com o resumo das mensagens compactadas.
fn with_summary(system_prompt: String, summary: Option<&str>) -> String {
    match summary {
        Some(summary) => format!(
            "{system_prompt}\n\nRESUMO DA CONVERSA ATÉ AQUI (as mensagens anteriores foram compactadas para caber na janela de contexto):\n{summary}\n"
        ),
        None => system_prompt,
    }
}

impl AgentRequest {
    /// Pedido vindo do chat: ferramentas conforme as configurações.
    #[must_use]
    pub fn from_chat(request: ChatStreamRequest, settings: &AiSettings, actor: AuditActor) -> Self {
        Self {
            messages: request.messages,
            context: ChatContext {
                device_id: request.device_id,
                monitor_id: request.monitor_id,
                alert_id: request.alert_id,
                rule_id: request.rule_id,
                mentions: mentions::sanitize(request.mentions),
                screen: screens::sanitize(request.screen),
            },
            policy: ToolPolicy::from_settings(settings),
            tool_loading: ToolLoading::OnDemand,
            memory: ConversationMemory {
                summary: request
                    .summary
                    .map(|summary| summary.trim().to_string())
                    .filter(|summary| !summary.is_empty()),
                last_context_tokens: request.context_hint.as_ref().map(|hint| hint.tokens),
                last_context_window: request.context_hint.as_ref().and_then(|hint| hint.window),
                tool_groups: request
                    .context_hint
                    .as_ref()
                    .map(|hint| ToolGroups::from_ids(&hint.tool_groups))
                    .unwrap_or_default(),
                last_model: request.context_hint.and_then(|hint| hint.model),
                force_compaction: request.compact,
                conversation_key: request
                    .conversation_key
                    .map(|key| key.trim().chars().take(64).collect::<String>())
                    .filter(|key| !key.is_empty()),
            },
            actor,
        }
    }
}

impl HarnessEvent {
    fn notice(message: impl Into<String>, settings: bool) -> Self {
        Self::Notice {
            message: message.into(),
            settings,
            key: None,
        }
    }

    /// Um estado da conferência da resposta: cada um substitui o anterior.
    fn answer_check(message: impl Into<String>) -> Self {
        Self::Notice {
            message: message.into(),
            settings: false,
            key: Some(ANSWER_CHECK_NOTICE),
        }
    }
}

/// Canal de eventos que para de trabalhar quando a tela fecha a conexão.
struct EventSink(mpsc::Sender<HarnessEvent>);

impl EventSink {
    /// `false` quando ninguém escuta mais.
    async fn send(&self, event: HarnessEvent) -> bool {
        self.0.send(event).await.is_ok()
    }

    /// Espera `work` ou a tela fechar, o que vier antes: `None` quando ela
    /// fechou e `work` foi largado. Só para o que pode parar no meio sem efeito
    /// no mundo — provedor, Decisor, resumo, consulta. Uma ação que começou
    /// termina: cortá-la no meio deixaria o equipamento pela metade.
    async fn unless_closed<T>(&self, work: impl Future<Output = T>) -> Option<T> {
        tokio::select! {
            biased;
            () = self.0.closed() => None,
            value = work => Some(value),
        }
    }

    /// Fecha a resposta: resolve a janela do modelo que de fato respondeu e
    /// manda as métricas.
    async fn finish(&self, metrics: &mut TurnMetrics, driver: &dyn AiDriver) {
        metrics.settle_window(driver).await;
        if let Some(event) = metrics.event() {
            let _ = self.send(event).await;
        }
        let _ = self.send(HarnessEvent::Done).await;
    }

    async fn fail(&self, message: String, metrics: &mut TurnMetrics, driver: &dyn AiDriver) {
        let _ = self.send(HarnessEvent::Error { message }).await;
        self.finish(metrics, driver).await;
    }
}

/// O que se mede ao longo das rodadas de uma resposta.
struct TurnMetrics {
    started: Instant,
    usage: AiUsage,
    /// Uso só da rodada mais recente.
    last_round: AiUsage,
    model: Option<String>,
    generation: Duration,
    /// Janela conhecida e o modelo a que ela se refere.
    window: Option<(String, ContextWindow)>,
    /// Tamanho estimado da conversa, para quando o provedor não mede.
    estimated_context: u64,
    /// Estimativa do que a última rodada leu desta resposta (ferramentas,
    /// resultados, conferência): sai da conta porque não volta no histórico.
    transient: u64,
    /// Grupos carregados ao fim da resposta.
    tool_groups: Vec<String>,
}

impl TurnMetrics {
    fn new(configured_model: Option<&str>) -> Self {
        Self {
            started: Instant::now(),
            usage: AiUsage::default(),
            last_round: AiUsage::default(),
            model: configured_model
                .filter(|model| !model.trim().is_empty())
                .map(ToString::to_string),
            generation: Duration::ZERO,
            window: None,
            estimated_context: 0,
            transient: 0,
            tool_groups: Vec::new(),
        }
    }

    /// Garante que a janela é a do modelo que respondeu (um roteador troca).
    async fn settle_window(&mut self, driver: &dyn AiDriver) {
        let Some(model) = self.model.clone() else {
            return;
        };
        if self
            .window
            .as_ref()
            .is_some_and(|(known, _)| *known == model)
        {
            return;
        }
        let window = context_window::resolve(driver, &model).await;
        self.window = Some((model, window));
    }

    /// A janela do modelo atual: a resolvida quando é dele, senão a estimada.
    fn current_window(&self) -> Option<ContextWindow> {
        let model = self.model.as_deref()?;
        Some(match &self.window {
            Some((known, window)) if known == model => *window,
            _ => ContextWindow {
                tokens: context_window::estimate(model),
                reported: false,
            },
        })
    }

    fn start_round(&mut self) {
        self.last_round = AiUsage::default();
    }

    fn add_usage(&mut self, usage: AiUsage) {
        self.usage.add(usage);
        self.last_round.add(usage);
    }

    /// O que a próxima pergunta carrega: a última rodada medida (ou, sem
    /// medida, estimada) menos o tráfego desta resposta. A estimativa do que
    /// fica é o piso — o desconto nunca leva abaixo dela.
    fn carried_context(&self) -> u64 {
        let measured = self.last_round.prompt_tokens + self.last_round.completion_tokens;
        let estimated = self.estimated_context.saturating_sub(self.transient);
        if measured == 0 {
            return estimated;
        }
        measured.saturating_sub(self.transient).max(estimated)
    }

    /// O evento para a tela; `None` quando nada foi medido.
    fn event(&self) -> Option<HarnessEvent> {
        if self.usage == AiUsage::default() && self.model.is_none() {
            return None;
        }
        let window = self.current_window();
        Some(HarnessEvent::Usage {
            prompt_tokens: self.usage.prompt_tokens,
            completion_tokens: self.usage.completion_tokens,
            cached_tokens: self.usage.cached_tokens,
            tool_groups: self.tool_groups.clone(),
            context_tokens: self.carried_context(),
            context_window: window.map(|window| window.tokens),
            context_window_reported: window.is_some_and(|window| window.reported),
            model: self.model.clone(),
            generation_ms: duration_ms(self.generation),
            duration_ms: duration_ms(self.started.elapsed()),
        })
    }
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Aviso mostrado na primeira vez que a IA propõe tocar um equipamento.
pub const DEVICE_ACCESS_WARNING: &str = "Atenção: a IA vai acessar um equipamento real. Ela pode \
alucinar — interpretar errado uma saída ou propor um comando incorreto — e uma alteração errada \
pode derrubar a rede ou cortar o acesso ao equipamento. Leia o comando e o motivo de cada pedido \
antes de aprovar. O NetMonitor não se responsabiliza por danos causados por ações da IA.";

/// Quem conversa e o que decide as chamadas de uma resposta: o aviso de
/// equipamento já dado, a conversa (modo "Aceitar automaticamente") e o Decisor
/// que revisa as ações do modo automático.
struct ToolSession {
    /// Quem conversa: a ação que roda sem confirmação fica na auditoria dele.
    actor: AuditActor,
    /// O resto da central (plugins de dispositivo), resolvido no começo.
    central: Option<Central>,
    conversation_key: Option<String>,
    warned: bool,
    reviewer: Option<ActionReviewer>,
    /// As chamadas (ferramenta + argumentos) que já esperam confirmação nesta
    /// resposta: o modelo pequeno repete a proposta na rodada seguinte, e
    /// cada repetição seria mais um cartão igual na tela.
    awaiting: HashSet<String>,
}

impl ToolSession {
    /// O modo automático está ligado para este usuário, conversa e equipamento
    /// da chamada. Sem usuário, conversa ou equipamento identificável, não está.
    async fn auto_approves(&self, ctx: &AppContext, arguments: &serde_json::Value) -> bool {
        let Some(user_id) = self.actor.user_id else {
            return false;
        };
        let Some(key) = self.conversation_key.as_deref() else {
            return false;
        };
        let Some(device_id) = target_device_id(ctx, arguments).await else {
            return false;
        };
        let Some(central) = self.central.as_ref() else {
            return false;
        };
        central
            .device_auto_accepted(&ctx.db, user_id, key, device_id)
            .await
    }

    /// A chamada voltou ao usuário: fica registrada para aparecer em
    /// qualquer tela e avisar no celular quem não está olhando.
    fn await_user(
        &self,
        ctx: &AppContext,
        tool: &AiToolCall,
        arguments: serde_json::Value,
        summary: &str,
        risk: &RiskAssessment,
    ) {
        let Some(user_id) = self.actor.user_id else {
            return;
        };
        approvals::open(
            ctx,
            ApprovalRequest {
                call_id: &tool.id,
                user_id,
                tool: &tool.name,
                arguments,
                summary,
                risk,
                conversation_key: self.conversation_key.clone(),
            },
        );
    }
}

/// As propostas desta conversa cujo cartão ainda espera a decisão: pedir de
/// novo na pergunta seguinte ("faça", "e aí?") seria um segundo cartão igual.
fn awaiting_in_conversation(
    ctx: &AppContext,
    user_id: Option<i64>,
    conversation_key: Option<&str>,
) -> HashSet<String> {
    let (Some(user_id), Some(key)) = (user_id, conversation_key) else {
        return HashSet::new();
    };
    approvals::ApprovalDesk::from_context(ctx)
        .awaiting_in(user_id, key)
        .into_iter()
        .map(|(tool, arguments)| call_key(&tool, &arguments.to_string()))
        .collect()
}

/// O que vem depois de uma chamada de ferramenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolFlow {
    Continue,
    /// A chamada falhou na validação ou pediu confirmação: um `ask_user` na
    /// mesma rodada é suprimido para a IA corrigir o erro ou não empilhar dois
    /// cartões interativos.
    SuppressInteractive,
    /// A ferramenta perguntou ao usuário: a rodada termina sem nova chamada.
    EndTurn,
    /// A tela fechou a conexão.
    Disconnected,
}

/// Quando o modelo inventa o nome de uma ferramenta ainda não carregada
/// (ex.: `scheduler_add`), deduz o grupo do catálogo pelo nome para carregá-lo
/// já na próxima rodada.
fn infer_group_from_unknown_tool(name: &str) -> Option<ToolGroup> {
    let lower = name.trim().to_lowercase();
    if lower.contains("schedul") || lower.contains("cron") || lower.contains("task") {
        Some(ToolGroup::Schedules)
    } else if lower.contains("docker") || lower.contains("container") {
        Some(ToolGroup::Docker)
    } else if lower.contains("topolog") {
        Some(ToolGroup::Platform)
    } else if lower.contains("chart") || lower.contains("graph") {
        Some(ToolGroup::Charts)
    } else if lower.contains("audit") {
        Some(ToolGroup::Audit)
    } else if lower.contains("dashboard") || lower.contains("widget") {
        Some(ToolGroup::Dashboard)
    } else if lower.contains("rule") {
        Some(ToolGroup::AlertRules)
    } else if lower.contains("plugin") || lower.contains("ssh") {
        Some(ToolGroup::Devices)
    } else if lower.contains("log") {
        // Depois de docker e audit: `docker_logs` e `audit_log` têm grupo próprio.
        Some(ToolGroup::Logs)
    } else if ["histor", "metric", "uptime", "sla"]
        .iter()
        .any(|k| lower.contains(k))
    {
        Some(ToolGroup::History)
    } else if ["ping", "trace", "scan", "dns"]
        .iter()
        .any(|k| lower.contains(k))
    {
        Some(ToolGroup::Diagnostics)
    } else {
        None
    }
}

/// A consulta que a tela aberta pede antes da primeira rodada — só de leitura,
/// e só quando a pergunta não marcou outro alvo com `@`.
fn screen_prefetch(context: &ChatContext, registry: &ToolRegistry) -> Option<AiToolCall> {
    let screen = context.screen.as_ref()?;
    if !context.mentions.is_empty() {
        return None;
    }
    let (name, arguments) = screen.screen.prefetch?;
    (registry.kind_of(name) == Some(ToolKind::Passive)).then(|| AiToolCall {
        id: format!("tela-{}", screen.screen.id),
        name: name.to_string(),
        arguments: arguments.to_string(),
    })
}

/// Resolve uma chamada de ferramenta: executa, ou devolve o pedido de
/// confirmação.
async fn handle_tool_call(
    ctx: &AppContext,
    registry: &ToolRegistry,
    sink: &EventSink,
    conversation: &mut Vec<AiMessage>,
    loaded: &mut ToolGroups,
    session: &mut ToolSession,
    mut tool: AiToolCall,
) -> ToolFlow {
    let mut arguments: serde_json::Value =
        serde_json::from_str(&tool.arguments).unwrap_or_else(|_| json!({}));

    // Carregar grupos é coisa do agente, não consulta: a tela não vê.
    if tool.name == LOAD_TOOLS {
        let available = registry.available_groups();
        let (fresh, already): (Vec<_>, Vec<_>) = requested_groups(&arguments)
            .into_iter()
            .filter(|group| available.contains(group))
            .partition(|group| loaded.insert(*group));
        let tools: Vec<&str> = fresh
            .iter()
            .flat_map(|group| registry.tool_names(*group))
            .collect();
        let mut reply = json!({ "loaded": tools });
        if !already.is_empty() {
            reply["already_loaded"] =
                json!(already.iter().map(|group| group.id()).collect::<Vec<_>>());
        }
        conversation.push(AiMessage {
            role: "tool".to_string(),
            content: Some(reply.to_string()),
            tool_calls: None,
            tool_call_id: Some(tool.id),
        });
        return ToolFlow::Continue;
    }

    // Ferramenta inexistente ou desligada volta para a IA como erro sem poluir
    // a tela com card quebrado; se o nome indicar um grupo sob demanda, ele já
    // entra para a próxima rodada.
    let Some(kind) = registry.kind_of(&tool.name) else {
        let mut message = format!("Ferramenta indisponível: {}", tool.name);
        if let Some(group) = infer_group_from_unknown_tool(&tool.name) {
            if registry.available_groups().contains(&group) {
                loaded.insert(group);
                let names = registry.tool_names(group).join(", ");
                message = format!(
                    "{message}. O grupo '{}' foi carregado; use uma destas ferramentas: {names}. (Nota: tarefas agendadas só executam ferramentas do catálogo, não lembretes de texto livre.)",
                    group.id()
                );
            }
        }
        tracing::info!(tool = %tool.name, "ferramenta inexistente recusada sem card na tela");
        conversation.push(AiMessage {
            role: "tool".to_string(),
            content: Some(json!({ "error": message }).to_string()),
            tool_calls: None,
            tool_call_id: Some(tool.id),
        });
        return ToolFlow::SuppressInteractive;
    };

    // Chamada direta pelo nome do catálogo, sem `load_tools`: roda do mesmo
    // jeito, e o grupo entra para as próximas rodadas terem o contrato.
    if let Some(group) = registry.group_of(&tool.name) {
        if group != ToolGroup::Core {
            loaded.insert(group);
        }
    }

    if kind == ToolKind::Interactive {
        arguments = sanitize_ask_user_arguments(ctx, conversation, &arguments).await;
        tool.arguments = arguments.to_string();
    }

    // Alvo interno inventado volta para a IA como erro, sem card nem pedido
    // de confirmação: o usuário não tem o que decidir sobre um IP sem origem.
    if let Some(target) = registry.network_target(&tool.name, &tool.arguments) {
        if let Some(rejection) = invented_target(&ctx.db, conversation, &target).await {
            tracing::info!(tool = %tool.name, %target, "teste ativo recusado: alvo sem origem");
            conversation.push(AiMessage {
                role: "tool".to_string(),
                content: Some(json!({ "error": rejection }).to_string()),
                tool_calls: None,
                tool_call_id: Some(tool.id),
            });
            return ToolFlow::SuppressInteractive;
        }
    }

    let proposal = call_key(&tool.name, &tool.arguments);
    if session.awaiting.contains(&proposal) {
        tracing::info!(tool = %tool.name, "proposta repetida: o cartão já está na tela");
        conversation.push(AiMessage {
            role: "tool".to_string(),
            content: Some(
                json!({
                    "status": "awaiting_user_confirmation",
                    "note": "Esta mesma proposta já espera confirmação na tela; não repita a chamada. Diga em uma frase o que propôs.",
                })
                .to_string(),
            ),
            tool_calls: None,
            tool_call_id: Some(tool.id),
        });
        return ToolFlow::SuppressInteractive;
    }

    let mut flow = ToolFlow::Continue;
    let device_access = kind == ToolKind::DeviceAccess;
    if device_access && !session.warned {
        session.warned = true;
        if !sink
            .send(HarnessEvent::notice(DEVICE_ACCESS_WARNING, false))
            .await
        {
            return ToolFlow::Disconnected;
        }
    }
    let auto_approved = device_access && session.auto_approves(ctx, &arguments).await;
    let args = ToolArgs::parse(&tool.arguments)
        .with_actor(session.actor.clone())
        .with_conversation(session.conversation_key.clone());
    let decision = if auto_approved {
        GateDecision::Run(RunPlan::auto_accepted())
    } else {
        let review = approval::decide(
            ctx,
            registry,
            session.reviewer.as_ref(),
            &tool.name,
            &args,
            &tool.arguments,
        );
        let Some(decision) = sink.unless_closed(review).await else {
            return ToolFlow::Disconnected;
        };
        decision
    };
    let result = match decision {
        GateDecision::Ask(risk) => {
            let preview = registry.preview(ctx, &tool.name, &tool.arguments);
            let Some(preview) = sink.unless_closed(preview).await else {
                return ToolFlow::Disconnected;
            };
            let summary = match preview {
                Ok(text) => text,
                Err(error) => {
                    conversation.push(AiMessage {
                        role: "tool".to_string(),
                        content: Some(compact_for_model(&tool_error(
                            session.central.as_ref(),
                            &error,
                        ))),
                        tool_calls: None,
                        tool_call_id: Some(tool.id),
                    });
                    return ToolFlow::SuppressInteractive;
                }
            };
            // Ação com argumento inválido ou alvo inexistente volta para a IA
            // corrigir na próxima rodada — sem botão "Confirmar" quebrado.
            if let Some(problem) = invalid_action_reason(&summary) {
                tracing::info!(tool = %tool.name, %problem, "ação inválida devolvida à IA sem pedir confirmação");
                conversation.push(AiMessage {
                    role: "tool".to_string(),
                    content: Some(json!({ "error": problem }).to_string()),
                    tool_calls: None,
                    tool_call_id: Some(tool.id),
                });
                return ToolFlow::SuppressInteractive;
            }
            if !sink
                .send(HarnessEvent::ConfirmationRequired {
                    id: tool.id.clone(),
                    name: tool.name.clone(),
                    arguments: arguments.clone(),
                    summary: summary.clone(),
                    risk: risk.level,
                    reason: risk.reason.clone(),
                })
                .await
            {
                return ToolFlow::Disconnected;
            }
            session.await_user(ctx, &tool, arguments, &summary, &risk);
            session.awaiting.insert(proposal);
            flow = ToolFlow::SuppressInteractive;
            json!({
                "status": "awaiting_user_confirmation",
                "summary": summary,
                "why": risk.reason,
                "note": "Os botões de confirmar/cancelar já estão na tela: diga em uma frase o que propôs e não repita a chamada.",
            })
        }
        GateDecision::Run(plan) => {
            let shown = plan.risk;
            if !sink
                .send(HarnessEvent::ToolCall {
                    id: tool.id.clone(),
                    name: tool.name.clone(),
                    arguments,
                    auto_approved,
                    risk: shown.as_ref().map(|risk| risk.level),
                    reason: shown.map(|risk| risk.reason),
                })
                .await
            {
                return ToolFlow::Disconnected;
            }
            let run = plan.execution.run(ctx, registry, &tool.name, args);
            // Consulta e teste ativo param com a tela; ação vai até o fim.
            let execution = if matches!(kind, ToolKind::Passive | ToolKind::Active) {
                match sink.unless_closed(run).await {
                    Some(execution) => execution,
                    None => return ToolFlow::Disconnected,
                }
            } else {
                run.await
            };
            let (result, chart) = match execution {
                Ok(output) => {
                    if output.ends_turn {
                        flow = ToolFlow::EndTurn;
                    }
                    (output.data, output.chart)
                }
                Err(err) => (tool_error(session.central.as_ref(), &err), None),
            };
            if !sink
                .send(HarnessEvent::ToolResult {
                    id: tool.id.clone(),
                    name: tool.name.clone(),
                    result: result.clone(),
                    chart,
                })
                .await
            {
                return ToolFlow::Disconnected;
            }
            result
        }
    };

    conversation.push(AiMessage {
        role: "tool".to_string(),
        content: Some(compact_for_model(&result)),
        tool_calls: None,
        tool_call_id: Some(tool.id),
    });
    flow
}

/// Executa o loop do Agent Harness transmitindo eventos em tempo real.
pub async fn run_agent_loop(
    ctx: AppContext,
    settings: AiSettings,
    driver: Box<dyn AiDriver>,
    request: AgentRequest,
) -> HarnessEventStream {
    let (sender, receiver) = mpsc::channel(64);

    tokio::spawn(async move {
        let sink = EventSink(sender);
        // As áreas desligadas (ADR 017) tiram ferramentas, grupos, regras do
        // prompt e intenções do Decisor de uma vez: tudo sai do registro.
        let areas = netmonitor_core::areas::load(&ctx.db)
            .await
            .map(|enabled| AreaSet::from_enabled(&enabled))
            .unwrap_or_default();
        let mut policy = request.policy.with_areas(areas);
        // Papel por área (ADR 019): a IA do chat só alcança o que o usuário
        // alcança. A borda HTTP (`controllers/ai.rs`) exige usuário autenticado
        // antes de chegar aqui; rotinas internas (proativo) definem
        // `ToolPolicy::passive()` explicitamente.
        if let Some(user_id) = request.actor.user_id {
            policy = policy.with_access(&AreaAccess::load(&ctx.db, Some(user_id)).await);
        }
        let registry = ToolRegistry::new(policy);
        let options = AiChatOptions {
            max_tokens: settings.response_style.max_output_tokens(),
        };
        let requested = request.messages.clone();
        let mut history = recent_history(request.messages, MAX_HISTORY_MESSAGES);
        let small_talk = request.tool_loading == ToolLoading::OnDemand && is_small_talk(&history);
        let available = registry.available_groups();
        let question = history
            .last()
            .map_or("", |message| message.content.as_str());
        let configured_model = driver.model().unwrap_or_default().to_string();
        // Modelo treinado com as ferramentas do NetMonitor conhece o catálogo
        // e o `load_tools`: dispensa o roteamento do Decisor (a revisão das
        // ações, no `reviewer`, continua).
        let trained =
            request.tool_loading == ToolLoading::OnDemand && settings.is_netmonitor_trained();
        // O Decisor decide enquanto a janela de contexto é consultada: as duas
        // são chamadas de rede e nenhuma depende da outra.
        let consult_decider =
            request.tool_loading == ToolLoading::OnDemand && !small_talk && !trained;
        let start = async {
            tokio::join!(
                async {
                    if consult_decider {
                        tool_routing::route(
                            &DeciderRuntime::from_context(&ctx),
                            &settings.decider,
                            question,
                            &available,
                        )
                        .await
                    } else {
                        None
                    }
                },
                context_window::resolve(driver.as_ref(), &configured_model)
            )
        };
        let Some((routed, mut window)) = sink.unless_closed(start).await else {
            return;
        };
        let everything = || available.iter().copied().collect::<ToolGroups>();
        // O que a pessoa marcou (`@container`, o alerta, a tela aberta) vai sempre.
        let marked = || context_groups(&request.context);
        let preselected: ToolGroups = match (request.tool_loading, routed) {
            (ToolLoading::Everything, _) => everything(),
            (ToolLoading::OnDemand, _) if small_talk => ToolGroups::new(),
            // Prompt enxuto: núcleo, o marcado e até 3 grupos claros da
            // pergunta; o resto entra sob demanda pelo `load_tools`.
            (ToolLoading::OnDemand, _) if trained => {
                let mut groups = marked();
                groups.extend(question_groups(question).iter());
                groups
            }
            // Sem Decisor não há quem escolha: vai o catálogo inteiro.
            (ToolLoading::OnDemand, None) => everything(),
            // O Decisor escolhe; o marcado entra junto.
            (ToolLoading::OnDemand, Some(decided)) => {
                let mut groups = marked();
                groups.extend(decided.iter());
                groups
            }
        }
        .iter()
        .filter(|group| available.contains(group))
        .collect();
        // O que a conversa já carregou vem primeiro, na mesma ordem: a lista
        // de ferramentas repete a da pergunta anterior e o cache vale.
        let mut loaded: ToolGroups = request
            .memory
            .tool_groups
            .iter()
            .filter(|group| available.contains(group))
            .chain(preselected.iter())
            .collect();
        let mut device_session = ToolSession {
            actor: request.actor.clone(),
            central: Central::from_context(&ctx).ok(),
            conversation_key: request.memory.conversation_key.clone(),
            warned: false,
            awaiting: awaiting_in_conversation(
                &ctx,
                request.actor.user_id,
                request.memory.conversation_key.as_deref(),
            ),
            // Só o chat tem pedido do usuário para o Decisor ler.
            reviewer: (request.tool_loading == ToolLoading::OnDemand).then(|| ActionReviewer {
                runtime: DeciderRuntime::from_context(&ctx),
                settings: settings.decider.clone(),
                request: question.to_string(),
            }),
        };
        let system_prompt = if small_talk {
            history = recent_history(history, SMALL_TALK_HISTORY);
            build_small_talk_prompt(&settings)
        } else {
            // O catálogo do `load_tools` só faz sentido se sobrou grupo de fora.
            build_system_prompt(
                &settings,
                policy,
                available.iter().any(|group| !preselected.contains(group)),
            )
        };

        let mut metrics = TurnMetrics::new(driver.model());
        // Um roteador pode ter respondido com um modelo de janela menor: vale a
        // menor das duas.
        if let (Some(hint), Some(last_model)) = (
            request.memory.last_context_window,
            request.memory.last_model.as_deref(),
        ) {
            if is_router(&configured_model) && last_model != configured_model && hint > 0 {
                window.tokens = window.tokens.min(hint);
            }
        }
        metrics.window = Some((configured_model, window));
        let budget = ContextBudget::new(window.tokens, options.max_tokens);

        let mut summary = request.memory.summary.clone();
        // Compactar a pedido vale até numa cortesia: a tela já zerou o pedido.
        if !small_talk || request.memory.force_compaction {
            let fixed_for = |loaded: &ToolGroups| {
                estimate_tokens(&system_prompt)
                    + estimate_tokens(
                        &serde_json::to_string(&registry.definitions_for(loaded))
                            .unwrap_or_default(),
                    )
            };
            let mut fixed = fixed_for(&loaded);
            // A parte fixa sozinha não cabe: resumir mensagens não resolve.
            // Primeiro saem os grupos herdados de perguntas anteriores (o
            // cache de prefixo vale menos que caber); se nem assim, avisa.
            if fixed > budget.target() && loaded != preselected {
                loaded = preselected.clone();
                fixed = fixed_for(&loaded);
            }
            if let Some(message) = small_window_notice(&budget, fixed, driver.id()) {
                if !sink.send(HarnessEvent::notice(message, true)).await {
                    return;
                }
            }
            let summary_tokens = summary.as_deref().map_or(0, estimate_tokens);
            let measured = fixed + summary_tokens + estimate_history(&history);
            let reported = request.memory.last_context_tokens.map_or(0, |tokens| {
                tokens
                    + history
                        .last()
                        .map_or(0, |last| estimate_tokens(&last.content))
            });
            let used = measured.max(reported);
            let force = request.memory.force_compaction;
            if force || budget.needs_compaction(used) {
                let fold = fold_count(&history, fixed, &budget, force);
                // Resumir pouco não libera nada: a conversa sai do mesmo
                // tamanho (ou maior) e ainda custa uma chamada ao provedor.
                if fold > 0 && worth_folding(&history[..fold], force) {
                    // O que `recent_history` já tinha deixado de fora entra no
                    // resumo também: nada some sem ser resumido.
                    let folded = requested.len() - history.len() + fold;
                    let summarizing = summarize(
                        driver.as_ref(),
                        &budget,
                        summary.as_deref(),
                        &requested[..folded],
                    );
                    let Some(summarized) = sink.unless_closed(summarizing).await else {
                        return;
                    };
                    let text = match summarized {
                        Ok(text) => Some(text),
                        Err(error) => {
                            tracing::warn!(%error, "falha ao resumir a conversa; mensagens antigas saem sem resumo");
                            None
                        }
                    };
                    let summarized = text.is_some();
                    let text = text.unwrap_or_else(|| fallback_summary(summary.as_deref(), folded));
                    history.drain(..fold);
                    let after = fixed + estimate_tokens(&text) + estimate_history(&history);
                    if !sink
                        .send(HarnessEvent::ContextCompacted {
                            summary: text.clone(),
                            folded_messages: folded,
                            tokens_before: used,
                            tokens_after: after,
                            summarized,
                        })
                        .await
                    {
                        return;
                    }
                    summary = Some(text);
                    // O prefixo mudou de qualquer jeito: a conversa recomeça
                    // só com o que esta pergunta pede.
                    loaded = preselected.clone();
                }
            }
        }
        let system_prompt = if small_talk {
            system_prompt
        } else {
            with_summary(system_prompt, summary.as_deref())
        };

        if !small_talk {
            let turn_context = build_turn_context(&ctx.db, &request.context).await;
            if let Some(last) = history
                .iter_mut()
                .rev()
                .find(|message| message.role == "user")
            {
                last.content = format!("{turn_context}{}", last.content);
            }
        }

        let mut conversation = vec![AiMessage {
            role: "system".to_string(),
            content: Some(system_prompt),
            tool_calls: None,
            tool_call_id: None,
        }];
        conversation.extend(history.into_iter().map(|message| AiMessage {
            role: message.role,
            content: Some(message.content),
            tool_calls: None,
            tool_call_id: None,
        }));
        // Daqui em diante é a pergunta atual: o que vem antes já foi conferido.
        let question_at = conversation.len().saturating_sub(1);

        // Início da rodada mais recente: os resultados dela a IA ainda não leu.
        let mut round_start = conversation.len();
        // O estado da tela aberta entra antes da primeira rodada.
        if !small_talk {
            if let Some(call) = screen_prefetch(&request.context, &registry) {
                conversation.push(AiMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![call.clone()]),
                    tool_call_id: None,
                });
                let flow = handle_tool_call(
                    &ctx,
                    &registry,
                    &sink,
                    &mut conversation,
                    &mut loaded,
                    &mut device_session,
                    call,
                )
                .await;
                if matches!(flow, ToolFlow::Disconnected) {
                    return;
                }
            }
        }
        // A conferência da resposta final ganha uma rodada só por pergunta.
        let mut answer_retried = false;
        // Só a conferência pedida na última rodada estende o laço: a que
        // ocorreu antes já cabe nas rodadas restantes.
        let mut retry_on_last = false;
        let mut retry_used = false;
        let mut seen_call_ids = HashSet::new();
        let mut seen_results = SeenResults::default();
        let mut cached_loaded = loaded.clone();
        let mut tools = if small_talk {
            Vec::new()
        } else {
            registry.definitions_for(&loaded)
        };
        let mut tools_tokens = estimate_tokens(&serde_json::to_string(&tools).unwrap_or_default());
        let mut round = 0usize;
        while has_round_left(round, retry_on_last, retry_used) {
            if round >= MAX_ITERATIONS {
                retry_used = true;
            }
            round += 1;
            // Antes da poda: o resultado podado continua valendo na conferência.
            seen_results.record(&conversation);
            // Recalculado só quando `loaded` muda (`load_tools` ou chamada direta).
            if loaded != cached_loaded {
                tools = if small_talk {
                    Vec::new()
                } else {
                    registry.definitions_for(&loaded)
                };
                tools_tokens = estimate_tokens(&serde_json::to_string(&tools).unwrap_or_default());
                cached_loaded = loaded.clone();
            }
            let repeated = supersede_repeated_results(&mut conversation);
            if repeated > 0 {
                tracing::debug!(
                    repeated,
                    "consultas repetidas: resultados antigos substituídos"
                );
            }
            if prune_tool_results(&mut conversation, tools_tokens, &budget, round_start) {
                tracing::info!("resultados de ferramenta antigos podados para caber na janela");
            }
            metrics.estimated_context = estimate_messages(&conversation) + tools_tokens;
            metrics.transient = estimate_messages(&conversation[question_at + 1..]);
            metrics.tool_groups = loaded.ids();
            metrics.start_round();
            let (assistant_text, mut pending_tools) = match run_round(
                driver.as_ref(),
                &sink,
                &mut metrics,
                &conversation,
                &tools,
                options,
            )
            .await
            {
                RoundRead::Done { text, tools } => (text, tools),
                RoundRead::Failed(message) => {
                    return sink.fail(message, &mut metrics, driver.as_ref()).await
                }
                RoundRead::Disconnected => return,
            };
            unique_call_ids(&mut pending_tools, &mut seen_call_ids);

            if pending_tools.is_empty() {
                // Resposta que cita recurso que nada mostrou (`agente-west-01`
                // sem consulta) ou desenha um gráfico em texto: avisa e dá uma
                // rodada para a IA conferir com os dados. Se ela insistir, fica
                // o aviso claro — sem laço.
                let check = if small_talk {
                    AnswerCheck::default()
                } else {
                    AnswerCheck::run(
                        &ctx.db,
                        &assistant_text,
                        &conversation,
                        question_at,
                        &seen_results,
                        |name| registry.kind_of(name).is_some(),
                    )
                    .await
                };
                if answer_retried {
                    // A versão conferida chegou à tela enquanto era escrita; se
                    // ainda inventa, ela é trocada pela redigida. O aviso de
                    // "conferindo" vira o desfecho, nos dois casos.
                    if !check.needs_redaction() {
                        let _ = sink.send(HarnessEvent::answer_check(ANSWER_CHECKED)).await;
                    } else {
                        let notice = HarnessEvent::answer_check(check.final_notice());
                        if !sink.send(notice).await || !sink.send(HarnessEvent::Retract).await {
                            return;
                        }
                        let shown = check.redact(&assistant_text);
                        let _ = sink.send(HarnessEvent::TextDelta { content: shown }).await;
                    }
                    return sink.finish(&mut metrics, driver.as_ref()).await;
                }
                if check.is_clean() {
                    return sink.finish(&mut metrics, driver.as_ref()).await;
                }
                answer_retried = true;
                retry_on_last = round >= MAX_ITERATIONS;
                let notice = HarnessEvent::answer_check(check.retry_notice());
                if !sink.send(notice).await || !sink.send(HarnessEvent::Retract).await {
                    return;
                }
                round_start = conversation.len();
                conversation.push(AiMessage {
                    role: "assistant".to_string(),
                    content: Some(assistant_text),
                    tool_calls: None,
                    tool_call_id: None,
                });
                conversation.push(AiMessage {
                    role: "user".to_string(),
                    content: Some(check.retry_request()),
                    tool_calls: None,
                    tool_call_id: None,
                });
                continue;
            }

            round_start = conversation.len();
            conversation.push(AiMessage {
                role: "assistant".to_string(),
                content: (!assistant_text.is_empty()).then_some(assistant_text),
                tool_calls: Some(pending_tools.clone()),
                tool_call_id: None,
            });
            let mut ends_turn = false;
            let mut suppress_interactive = false;
            for tool in pending_tools {
                if suppress_interactive
                    && registry.kind_of(&tool.name) == Some(ToolKind::Interactive)
                {
                    conversation.push(AiMessage {
                        role: "tool".to_string(),
                        content: Some(
                            json!({
                                "error": "Outra chamada nesta mesma rodada já pediu confirmação ou falhou; trate o resultado dela antes de usar ask_user."
                            })
                            .to_string(),
                        ),
                        tool_calls: None,
                        tool_call_id: Some(tool.id),
                    });
                    continue;
                }
                match handle_tool_call(
                    &ctx,
                    &registry,
                    &sink,
                    &mut conversation,
                    &mut loaded,
                    &mut device_session,
                    tool,
                )
                .await
                {
                    ToolFlow::Disconnected => return,
                    ToolFlow::EndTurn => {
                        ends_turn = true;
                        suppress_interactive = true;
                    }
                    ToolFlow::SuppressInteractive => {
                        suppress_interactive = true;
                    }
                    ToolFlow::Continue => {}
                }
            }
            metrics.tool_groups = loaded.ids();
            if ends_turn {
                return sink.finish(&mut metrics, driver.as_ref()).await;
            }
        }
        // As rodadas acabaram sem resposta (ainda havia ferramenta): uma chamada
        // sem ferramentas fecha com o que as consultas trouxeram — a tela nunca
        // fica sem texto.
        conversation.push(AiMessage {
            role: "user".to_string(),
            content: Some(ROUND_LIMIT_NOTE.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
        metrics.transient = estimate_messages(&conversation[question_at + 1..]);
        metrics.start_round();
        match run_round(
            driver.as_ref(),
            &sink,
            &mut metrics,
            &conversation,
            &[],
            options,
        )
        .await
        {
            RoundRead::Done { text, .. } if text.trim().is_empty() => {
                sink.fail(NO_ANSWER.to_string(), &mut metrics, driver.as_ref())
                    .await;
            }
            RoundRead::Done { text, .. } => {
                if answer_retried {
                    let check = AnswerCheck::run(
                        &ctx.db,
                        &text,
                        &conversation,
                        question_at,
                        &seen_results,
                        |name| registry.kind_of(name).is_some(),
                    )
                    .await;
                    if !check.needs_redaction() {
                        let _ = sink.send(HarnessEvent::answer_check(ANSWER_CHECKED)).await;
                    } else {
                        let notice = HarnessEvent::answer_check(check.final_notice());
                        if !sink.send(notice).await || !sink.send(HarnessEvent::Retract).await {
                            return;
                        }
                        let shown = check.redact(&text);
                        let _ = sink.send(HarnessEvent::TextDelta { content: shown }).await;
                    }
                }
                sink.finish(&mut metrics, driver.as_ref()).await;
            }
            RoundRead::Failed(message) => sink.fail(message, &mut metrics, driver.as_ref()).await,
            RoundRead::Disconnected => {}
        }
    });

    Box::pin(ReceiverStream::new(receiver))
}

/// Pedido de fechamento quando as rodadas acabam.
const ROUND_LIMIT_NOTE: &str =
    "Limite de consultas desta pergunta atingido. Responda agora, curto, \
só com o que as consultas acima trouxeram; diga o que ficou sem verificar.";

/// Quando nem a rodada de fechamento traz texto.
const NO_ANSWER: &str =
    "A IA não chegou a uma resposta dentro do limite de consultas. Reformule a pergunta ou peça algo mais específico.";

/// Como terminou a leitura de uma rodada com o provedor.
enum RoundRead {
    Done {
        text: String,
        tools: Vec<AiToolCall>,
    },
    Failed(String),
    Disconnected,
}

/// Pede uma rodada ao provedor e a lê. Largada no meio quando a tela fecha —
/// também enquanto o modelo pensa sem escrever nada.
async fn run_round(
    driver: &dyn AiDriver,
    sink: &EventSink,
    metrics: &mut TurnMetrics,
    conversation: &[AiMessage],
    tools: &[AiTool],
    options: AiChatOptions,
) -> RoundRead {
    match sink
        .unless_closed(driver.chat_stream(conversation, tools, options))
        .await
    {
        None => RoundRead::Disconnected,
        Some(Err(err)) => RoundRead::Failed(err.to_string()),
        Some(Ok(stream)) => read_round(stream, sink, metrics).await,
    }
}

/// Lê a resposta de uma rodada: repassa os avisos, as métricas e o texto à
/// tela enquanto chega.
async fn read_round(
    mut stream: AiChunkStream,
    sink: &EventSink,
    metrics: &mut TurnMetrics,
) -> RoundRead {
    let mut text = String::new();
    let mut tools = Vec::new();
    let mut first_output: Option<Instant> = None;
    loop {
        let Some(next) = sink.unless_closed(stream.next()).await else {
            return RoundRead::Disconnected;
        };
        let Some(chunk) = next else {
            break;
        };
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(err) => return RoundRead::Failed(err.to_string()),
        };
        if chunk.text_delta.is_some() || !chunk.tool_calls.is_empty() {
            first_output.get_or_insert_with(Instant::now);
        }
        // O aviso do provedor é de configuração (modelo trocado por limite).
        if let Some(message) = chunk.notice {
            let notice = HarnessEvent::notice(message, true);
            if !sink.send(notice).await {
                return RoundRead::Disconnected;
            }
        }
        if let Some(model) = chunk.model {
            metrics.model = Some(model);
        }
        if let Some(usage) = chunk.usage {
            metrics.add_usage(usage);
        }
        if let Some(delta) = chunk.text_delta {
            text.push_str(&delta);
            if !sink.send(HarnessEvent::TextDelta { content: delta }).await {
                return RoundRead::Disconnected;
            }
        }
        tools.extend(chunk.tool_calls);
    }
    if let Some(first) = first_output {
        metrics.generation += first.elapsed();
    }
    RoundRead::Done { text, tools }
}

/// Provedor que repete o id entre rodadas (`call_0` em toda resposta) faria o
/// resultado de uma chamada valer pelo de outra — na conversa, na poda de
/// repetidas e no cartão da tela. O id já visto nesta vez ganha sufixo único.
fn unique_call_ids(calls: &mut [AiToolCall], seen: &mut HashSet<String>) {
    for call in calls {
        if seen.contains(&call.id) {
            call.id = format!(
                "{}_{}",
                call.id,
                &uuid::Uuid::new_v4().simple().to_string()[..8]
            );
        }
        seen.insert(call.id.clone());
    }
}

/// Resposta completa de uma sessão sem tela (rotinas automáticas).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentAnswer {
    pub text: String,
    pub usage: AiUsage,
}

/// Consome o stream e junta o texto, respeitando `Retract` quando a conferência
/// descarta a última rodada e emite a versão corrigida.
///
/// # Errors
///
/// O erro do provedor, quando a sessão falha, ou resposta vazia.
pub async fn collect_answer(mut stream: HarnessEventStream) -> AppResult<AgentAnswer> {
    let mut answer = AgentAnswer::default();
    let mut round_start = 0usize;
    while let Some(event) = stream.next().await {
        match event {
            HarnessEvent::TextDelta { content } => answer.text.push_str(&content),
            HarnessEvent::ToolResult { .. } | HarnessEvent::ConfirmationRequired { .. } => {
                round_start = answer.text.len();
            }
            HarnessEvent::Retract => answer.text.truncate(round_start),
            HarnessEvent::Usage {
                prompt_tokens,
                completion_tokens,
                cached_tokens,
                ..
            } => {
                answer.usage = AiUsage {
                    prompt_tokens,
                    completion_tokens,
                    cached_tokens,
                };
            }
            HarnessEvent::Error { message } => {
                return Err(AppError::service_unavailable(message));
            }
            HarnessEvent::Done => break,
            _ => {}
        }
    }
    answer.text = answer.text.trim().to_string();
    if answer.text.is_empty() {
        return Err(AppError::service_unavailable(
            "O provedor de IA não devolveu texto",
        ));
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtos::{AiChartAxis, AiChartSeries, AiChartUnit};

    #[test]
    fn rodada_extra_so_quando_a_conferencia_foi_na_ultima_rodada() {
        assert!(has_round_left(MAX_ITERATIONS - 1, false, false));
        // Conferência numa rodada do meio: o teto continua valendo.
        assert!(!has_round_left(MAX_ITERATIONS, false, false));
        // Conferência pedida na última rodada: ganha uma, uma vez só.
        assert!(has_round_left(MAX_ITERATIONS, true, false));
        assert!(!has_round_left(MAX_ITERATIONS + 1, true, true));
    }

    #[tokio::test]
    async fn collect_answer_descarta_texto_retratado_apos_ultima_ferramenta() {
        let events: HarnessEventStream = Box::pin(futures::stream::iter(vec![
            HarnessEvent::TextDelta {
                content: "Consultando...".into(),
            },
            HarnessEvent::ToolResult {
                id: "t1".into(),
                name: "get_alerts".into(),
                result: json!({}),
                chart: None,
            },
            HarnessEvent::TextDelta {
                content: "O `agente-falso-01` caiu.".into(),
            },
            HarnessEvent::Retract,
            HarnessEvent::TextDelta {
                content: " Nenhum alerta aberto.".into(),
            },
            HarnessEvent::Done,
        ]));
        let answer = collect_answer(events).await.unwrap();
        assert_eq!(answer.text, "Consultando... Nenhum alerta aberto.");
    }

    fn msg(role: &str, content: &str) -> ChatMessageInput {
        ChatMessageInput {
            role: role.into(),
            content: content.into(),
        }
    }

    #[test]
    fn ferramenta_inventada_carrega_o_grupo_certo() {
        let grupo = infer_group_from_unknown_tool;
        assert_eq!(grupo("get_logs"), Some(ToolGroup::Logs));
        assert_eq!(grupo("search_syslog"), Some(ToolGroup::Logs));
        assert_eq!(grupo("docker_logs_tail"), Some(ToolGroup::Docker));
        assert_eq!(grupo("get_audit_log"), Some(ToolGroup::Audit));
        assert_eq!(grupo("topology_graph"), Some(ToolGroup::Platform));
        assert_eq!(grupo("cpu_graph"), Some(ToolGroup::Charts));
        assert_eq!(grupo("get_uptime"), Some(ToolGroup::History));
        assert_eq!(grupo("qualquer_coisa"), None);
    }

    #[test]
    fn id_repetido_entre_rodadas_ganha_sufixo() {
        let chamada = |id: &str| AiToolCall {
            id: id.into(),
            name: "get_alerts".into(),
            arguments: "{}".into(),
        };
        let mut vistos = HashSet::new();
        let mut primeira = vec![chamada("call_0")];
        unique_call_ids(&mut primeira, &mut vistos);
        assert_eq!(primeira[0].id, "call_0");
        let mut segunda = vec![chamada("call_0"), chamada("call_1")];
        unique_call_ids(&mut segunda, &mut vistos);
        assert_ne!(segunda[0].id, "call_0");
        assert!(segunda[0].id.starts_with("call_0_"));
        assert_eq!(segunda[1].id, "call_1");
    }

    #[test]
    fn historico_mantem_as_ultimas_e_comeca_pelo_usuario() {
        let historico = vec![
            msg("user", "1"),
            msg("assistant", "2"),
            msg("user", "3"),
            msg("assistant", "4"),
            msg("user", "5"),
        ];
        let recentes = recent_history(historico.clone(), 4);
        let conteudos: Vec<&str> = recentes.iter().map(|m| m.content.as_str()).collect();
        assert_eq!(
            conteudos,
            vec!["3", "4", "5"],
            "a resposta órfã '2' cai fora"
        );

        assert_eq!(recent_history(historico, 50).len(), 5);
    }

    #[test]
    fn eventos_novos_serializam_em_camel_case() {
        let uso = serde_json::to_value(HarnessEvent::Usage {
            prompt_tokens: 1200,
            completion_tokens: 80,
            cached_tokens: 1024,
            model: Some("openai/gpt-4o-mini".into()),
            tool_groups: vec!["docker".into()],
            context_tokens: 900,
            context_window: Some(128_000),
            context_window_reported: true,
            generation_ms: 1500,
            duration_ms: 4200,
        })
        .unwrap();
        assert_eq!(uso["type"], "usage");
        assert_eq!(uso["promptTokens"], 1200);
        assert_eq!(uso["completionTokens"], 80);
        assert_eq!(uso["model"], "openai/gpt-4o-mini");
        assert_eq!(uso["contextTokens"], 900);
        assert_eq!(uso["contextWindow"], 128_000);
        assert_eq!(uso["contextWindowReported"], true);
        assert_eq!(uso["generationMs"], 1500);
        assert_eq!(uso["durationMs"], 4200);
        assert_eq!(uso["cachedTokens"], 1024);
        assert_eq!(uso["toolGroups"], serde_json::json!(["docker"]));

        let pedido = serde_json::to_value(HarnessEvent::ConfirmationRequired {
            id: "c1".into(),
            name: "silence_alert".into(),
            arguments: json!({ "alert_id": 3 }),
            summary: "Silenciar o alerta #3 por 60 min".into(),
            risk: AiToolRisk::High,
            reason: "Um problema real pode passar despercebido".into(),
        })
        .unwrap();
        assert_eq!(pedido["type"], "confirmationRequired");
        assert_eq!(pedido["summary"], "Silenciar o alerta #3 por 60 min");
        assert_eq!(pedido["risk"], "high");
        assert_eq!(
            pedido["reason"],
            "Um problema real pode passar despercebido"
        );

        let automatica = serde_json::to_value(HarnessEvent::ToolCall {
            id: "c2".into(),
            name: "docker_container_action".into(),
            arguments: json!({}),
            auto_approved: false,
            risk: Some(AiToolRisk::Low),
            reason: Some("Inicia um container parado".into()),
        })
        .unwrap();
        assert_eq!(automatica["risk"], "low");
        assert!(automatica.get("autoApproved").is_none());

        let consulta = serde_json::to_value(HarnessEvent::ToolCall {
            id: "c3".into(),
            name: "get_alerts".into(),
            arguments: json!({}),
            auto_approved: false,
            risk: None,
            reason: None,
        })
        .unwrap();
        assert!(
            consulta.get("risk").is_none(),
            "consulta não tem risco a mostrar"
        );
    }

    #[test]
    fn metricas_somam_rodadas_mas_contexto_e_so_da_ultima() {
        let mut metricas = TurnMetrics::new(Some("llama3.2"));
        assert!(
            metricas.event().is_some(),
            "o modelo sozinho já vale mostrar"
        );

        metricas.start_round();
        metricas.add_usage(AiUsage {
            prompt_tokens: 1000,
            completion_tokens: 50,
            cached_tokens: 0,
        });
        metricas.start_round();
        metricas.add_usage(AiUsage {
            prompt_tokens: 1400,
            completion_tokens: 120,
            cached_tokens: 0,
        });
        metricas.model = Some("meta-llama/llama-3.3-70b-instruct".into());

        let Some(HarnessEvent::Usage {
            prompt_tokens,
            completion_tokens,
            context_tokens,
            context_window,
            model,
            ..
        }) = metricas.event()
        else {
            panic!("esperava o evento de uso");
        };
        assert_eq!((prompt_tokens, completion_tokens), (2400, 170));
        assert_eq!(context_tokens, 1520);
        assert_eq!(context_window, Some(131_072));
        assert_eq!(model.as_deref(), Some("meta-llama/llama-3.3-70b-instruct"));
    }

    #[test]
    fn contexto_carregado_desconta_o_trafego_da_resposta() {
        let mut metricas = TurnMetrics::new(Some("llama3.2"));
        metricas.estimated_context = 9_000;
        metricas.transient = 6_000;
        metricas.start_round();
        metricas.add_usage(AiUsage {
            prompt_tokens: 9_500,
            completion_tokens: 100,
            cached_tokens: 0,
        });
        assert_eq!(metricas.carried_context(), 3_600, "medido menos o tráfego");
        metricas.transient = 9_550;
        assert_eq!(
            metricas.carried_context(),
            50,
            "o desconto não passa da medida"
        );
        metricas.last_round = AiUsage::default();
        metricas.transient = 6_000;
        assert_eq!(
            metricas.carried_context(),
            3_000,
            "sem medida, só a estimativa"
        );
    }

    #[test]
    fn sem_modelo_nem_tokens_nao_ha_evento() {
        assert!(TurnMetrics::new(None).event().is_none());
        assert!(TurnMetrics::new(Some("  ")).event().is_none());
    }

    #[test]
    fn resultado_de_ferramenta_so_leva_grafico_quando_existe() {
        let sem = serde_json::to_value(HarnessEvent::ToolResult {
            id: "c1".into(),
            name: "get_alerts".into(),
            result: json!({ "total": 0 }),
            chart: None,
        })
        .unwrap();
        assert_eq!(sem["type"], "toolResult");
        assert!(sem.get("chart").is_none());

        let com = serde_json::to_value(HarnessEvent::ToolResult {
            id: "c2".into(),
            name: "chart_device_metric".into(),
            result: json!({ "chart_shown": true }),
            chart: Some(AiChart {
                title: "CPU — Borda".into(),
                subtitle: None,
                unit: AiChartUnit::Percentage,
                x_axis: AiChartAxis::Time,
                series: vec![AiChartSeries {
                    id: "cpu_usage".into(),
                    label: "CPU".into(),
                    points: Vec::new(),
                }],
                avg_value: Some(12.5),
            }),
        })
        .unwrap();
        assert_eq!(com["chart"]["unit"], "percentage");
        assert_eq!(com["chart"]["xAxis"], "time");
        assert_eq!(com["chart"]["avgValue"], 12.5);
    }
}
