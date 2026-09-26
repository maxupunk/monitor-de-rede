//! Orquestrador do Agent Harness (ReAct Loop) da IA.
//!
//! Coordena a troca de mensagens com o provedor de IA e a execução iterativa de ferramentas.

use std::{
    pin::Pin,
    time::{Duration, Instant},
};

use futures::{Stream, StreamExt};
use loco_rs::prelude::AppContext;
use serde::Serialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    compaction::{
        estimate_history, estimate_messages, estimate_tokens, fallback_summary, fold_count,
        prune_tool_results, small_window_notice, summarize, worth_folding, ContextBudget,
    },
    prompt::{build_small_talk_prompt, build_system_prompt, build_turn_context, ChatContext},
    tools::{ToolGroup, ToolGroups, ToolPolicy, ToolRegistry, LOAD_TOOLS},
    turn::{compact_for_model, is_small_talk, preselect_groups, requested_groups},
};
use crate::{
    dtos::ai::{AiChart, ChatMessageInput, ChatStreamRequest},
    services::{
        ai::{
            context_window::{self, ContextWindow},
            drivers::traits::{AiChatOptions, AiDriver, AiMessage, AiToolCall, AiUsage},
            mentions,
            settings::AiSettings,
        },
        audit::AuditActor,
        shared::errors::{AppError, AppResult},
    },
};

/// Rodadas de raciocínio (chamada ao provedor + ferramentas) por pergunta.
const MAX_ITERATIONS: usize = 6;

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
        /// Tokens da última rodada (entrada + saída): o tamanho da conversa
        /// que a próxima pergunta vai carregar.
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
    },
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
            },
            actor,
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

    /// O evento para a tela; `None` quando nada foi medido.
    fn event(&self) -> Option<HarnessEvent> {
        if self.usage == AiUsage::default() && self.model.is_none() {
            return None;
        }
        let measured = self.last_round.prompt_tokens + self.last_round.completion_tokens;
        let window = self.current_window();
        Some(HarnessEvent::Usage {
            prompt_tokens: self.usage.prompt_tokens,
            completion_tokens: self.usage.completion_tokens,
            cached_tokens: self.usage.cached_tokens,
            tool_groups: self.tool_groups.clone(),
            context_tokens: if measured > 0 {
                measured
            } else {
                self.estimated_context
            },
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

/// O que vem depois de uma chamada de ferramenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolFlow {
    Continue,
    /// A ferramenta perguntou ao usuário: a rodada termina sem nova chamada.
    EndTurn,
    /// A tela fechou a conexão.
    Disconnected,
}

/// Resolve uma chamada de ferramenta: executa, ou devolve o pedido de
/// confirmação.
async fn handle_tool_call(
    ctx: &AppContext,
    registry: &ToolRegistry,
    sink: &EventSink,
    conversation: &mut Vec<AiMessage>,
    loaded: &mut ToolGroups,
    actor: &AuditActor,
    tool: AiToolCall,
) -> ToolFlow {
    let arguments: serde_json::Value =
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

    let mut flow = ToolFlow::Continue;
    let result = if registry.needs_confirmation(&tool.name) {
        let summary = registry
            .preview(ctx, &tool.name, &tool.arguments)
            .await
            .unwrap_or_else(|error| error.to_string());
        if !sink
            .send(HarnessEvent::ConfirmationRequired {
                id: tool.id.clone(),
                name: tool.name.clone(),
                arguments,
                summary: summary.clone(),
            })
            .await
        {
            return ToolFlow::Disconnected;
        }
        json!({
            "status": "awaiting_user_confirmation",
            "summary": summary,
            "note": "A ação foi apresentada ao usuário com botões de confirmar/cancelar. Não repita a chamada.",
        })
    } else {
        if !sink
            .send(HarnessEvent::ToolCall {
                id: tool.id.clone(),
                name: tool.name.clone(),
                arguments,
            })
            .await
        {
            return ToolFlow::Disconnected;
        }
        let (result, chart) = match registry
            .execute_as(ctx, &tool.name, &tool.arguments, actor)
            .await
        {
            Ok(output) => {
                if output.ends_turn {
                    flow = ToolFlow::EndTurn;
                }
                (output.data, output.chart)
            }
            Err(err) => (json!({ "error": err.to_string() }), None),
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
        let registry = ToolRegistry::new(request.policy);
        let options = AiChatOptions {
            max_tokens: settings.response_style.max_output_tokens(),
        };
        let requested = request.messages.clone();
        let mut history = recent_history(request.messages, MAX_HISTORY_MESSAGES);
        let small_talk = request.tool_loading == ToolLoading::OnDemand && is_small_talk(&history);
        let available = registry.available_groups();
        let preselected: ToolGroups = match request.tool_loading {
            ToolLoading::Everything => available.iter().copied().collect(),
            ToolLoading::OnDemand => {
                let question = history
                    .last()
                    .map_or("", |message| message.content.as_str());
                let mut groups = preselect_groups(question, &request.context.mentions);
                if request.context.alert_id.is_some() || request.context.rule_id.is_some() {
                    groups.insert(ToolGroup::AlertRules);
                    groups.insert(ToolGroup::Analysis);
                }
                groups
                    .iter()
                    .filter(|group| available.contains(group))
                    .collect()
            }
        };
        // O que a conversa já carregou vem primeiro, na mesma ordem: a lista
        // de ferramentas repete a da pergunta anterior e o cache vale.
        let mut loaded: ToolGroups = request
            .memory
            .tool_groups
            .iter()
            .filter(|group| available.contains(group))
            .chain(preselected.iter())
            .collect();
        let system_prompt = if small_talk {
            history = recent_history(history, SMALL_TALK_HISTORY);
            build_small_talk_prompt(&settings)
        } else {
            build_system_prompt(
                &settings,
                request.policy,
                request.tool_loading == ToolLoading::OnDemand,
            )
        };

        let mut metrics = TurnMetrics::new(driver.model());
        let configured_model = driver.model().unwrap_or_default().to_string();
        let mut window = context_window::resolve(driver.as_ref(), &configured_model).await;
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
        if !small_talk {
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
                if !sink.send(HarnessEvent::Notice { message }).await {
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
                    let text = match summarize(
                        driver.as_ref(),
                        &budget,
                        summary.as_deref(),
                        &requested[..folded],
                    )
                    .await
                    {
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

        // Início da rodada mais recente: os resultados dela a IA ainda não leu.
        let mut round_start = conversation.len();
        for _ in 0..MAX_ITERATIONS {
            // Recalculado a cada rodada: um `load_tools` vale já na seguinte.
            let tools = if small_talk {
                Vec::new()
            } else {
                registry.definitions_for(&loaded)
            };
            let tools_tokens = estimate_tokens(&serde_json::to_string(&tools).unwrap_or_default());
            if prune_tool_results(&mut conversation, tools_tokens, &budget, round_start) {
                tracing::info!("resultados de ferramenta antigos podados para caber na janela");
            }
            metrics.estimated_context = estimate_messages(&conversation) + tools_tokens;
            metrics.tool_groups = loaded.ids();
            metrics.start_round();
            let mut stream = match driver.chat_stream(&conversation, &tools, options).await {
                Ok(stream) => stream,
                Err(err) => {
                    return sink
                        .fail(err.to_string(), &mut metrics, driver.as_ref())
                        .await
                }
            };

            let mut assistant_text = String::new();
            let mut pending_tools = Vec::new();
            let mut first_output: Option<Instant> = None;
            while let Some(chunk) = stream.next().await {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(err) => {
                        return sink
                            .fail(err.to_string(), &mut metrics, driver.as_ref())
                            .await
                    }
                };
                if chunk.text_delta.is_some() || !chunk.tool_calls.is_empty() {
                    first_output.get_or_insert_with(Instant::now);
                }
                if let Some(model) = chunk.model {
                    metrics.model = Some(model);
                }
                if let Some(chunk_usage) = chunk.usage {
                    metrics.add_usage(chunk_usage);
                }
                if let Some(delta) = chunk.text_delta {
                    assistant_text.push_str(&delta);
                    if !sink.send(HarnessEvent::TextDelta { content: delta }).await {
                        return;
                    }
                }
                pending_tools.extend(chunk.tool_calls);
            }
            if let Some(first) = first_output {
                metrics.generation += first.elapsed();
            }

            if pending_tools.is_empty() {
                return sink.finish(&mut metrics, driver.as_ref()).await;
            }

            round_start = conversation.len();
            conversation.push(AiMessage {
                role: "assistant".to_string(),
                content: (!assistant_text.is_empty()).then_some(assistant_text),
                tool_calls: Some(pending_tools.clone()),
                tool_call_id: None,
            });
            let mut ends_turn = false;
            for tool in pending_tools {
                match handle_tool_call(
                    &ctx,
                    &registry,
                    &sink,
                    &mut conversation,
                    &mut loaded,
                    &request.actor,
                    tool,
                )
                .await
                {
                    ToolFlow::Disconnected => return,
                    ToolFlow::EndTurn => ends_turn = true,
                    ToolFlow::Continue => {}
                }
            }
            metrics.tool_groups = loaded.ids();
            if ends_turn {
                return sink.finish(&mut metrics, driver.as_ref()).await;
            }
        }
        sink.finish(&mut metrics, driver.as_ref()).await;
    });

    Box::pin(ReceiverStream::new(receiver))
}

/// Resposta completa de uma sessão sem tela (rotinas automáticas).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentAnswer {
    pub text: String,
    pub usage: AiUsage,
}

/// Consome o stream e junta o texto.
///
/// # Errors
///
/// O erro do provedor, quando a sessão falha, ou resposta vazia.
pub async fn collect_answer(mut stream: HarnessEventStream) -> AppResult<AgentAnswer> {
    let mut answer = AgentAnswer::default();
    while let Some(event) = stream.next().await {
        match event {
            HarnessEvent::TextDelta { content } => answer.text.push_str(&content),
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
    use crate::dtos::ai::{AiChartAxis, AiChartSeries, AiChartUnit};

    fn msg(role: &str, content: &str) -> ChatMessageInput {
        ChatMessageInput {
            role: role.into(),
            content: content.into(),
        }
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
        })
        .unwrap();
        assert_eq!(pedido["type"], "confirmationRequired");
        assert_eq!(pedido["summary"], "Silenciar o alerta #3 por 60 min");
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
