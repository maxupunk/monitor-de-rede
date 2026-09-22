//! Orquestrador do Agent Harness (ReAct Loop) da IA.
//!
//! Coordena a troca de mensagens com o provedor de IA e a execução iterativa de ferramentas.

use std::pin::Pin;

use futures::{Stream, StreamExt};
use loco_rs::prelude::AppContext;
use serde::Serialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    prompt::{build_system_prompt, ChatContext},
    tools::{ToolPolicy, ToolRegistry},
};
use crate::{
    dtos::ai::{AiChart, ChatMessageInput, ChatStreamRequest},
    services::{
        ai::{
            drivers::traits::{AiChatOptions, AiDriver, AiMessage, AiToolCall, AiUsage},
            mentions,
            settings::AiSettings,
        },
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
    /// Tokens somados de todas as rodadas da resposta.
    #[serde(rename_all = "camelCase")]
    Usage {
        prompt_tokens: u64,
        completion_tokens: u64,
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

/// Uma sessão do agente: a pergunta, as ferramentas liberadas e o contexto.
pub struct AgentRequest {
    pub messages: Vec<ChatMessageInput>,
    pub context: ChatContext,
    pub policy: ToolPolicy,
}

impl AgentRequest {
    /// Pedido vindo do chat: ferramentas conforme as configurações.
    #[must_use]
    pub fn from_chat(request: ChatStreamRequest, settings: &AiSettings) -> Self {
        Self {
            messages: request.messages,
            context: ChatContext {
                device_id: request.device_id,
                monitor_id: request.monitor_id,
                alert_id: request.alert_id,
                mentions: mentions::sanitize(request.mentions),
            },
            policy: ToolPolicy::from_settings(settings),
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

    async fn finish(&self, usage: AiUsage) {
        if usage != AiUsage::default() {
            let _ = self
                .send(HarnessEvent::Usage {
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                })
                .await;
        }
        let _ = self.send(HarnessEvent::Done).await;
    }

    async fn fail(&self, message: String, usage: AiUsage) {
        let _ = self.send(HarnessEvent::Error { message }).await;
        self.finish(usage).await;
    }
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
    tool: AiToolCall,
) -> ToolFlow {
    let arguments: serde_json::Value =
        serde_json::from_str(&tool.arguments).unwrap_or_else(|_| json!({}));

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
        let (result, chart) = match registry.execute(ctx, &tool.name, &tool.arguments).await {
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
        content: Some(result.to_string()),
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
        let tools = registry.definitions();
        let options = AiChatOptions {
            max_tokens: settings.response_style.max_output_tokens(),
        };
        let system_prompt =
            build_system_prompt(&ctx.db, &settings, request.policy, &request.context).await;

        let mut conversation = vec![AiMessage {
            role: "system".to_string(),
            content: Some(system_prompt),
            tool_calls: None,
            tool_call_id: None,
        }];
        conversation.extend(
            recent_history(request.messages, MAX_HISTORY_MESSAGES)
                .into_iter()
                .map(|message| AiMessage {
                    role: message.role,
                    content: Some(message.content),
                    tool_calls: None,
                    tool_call_id: None,
                }),
        );

        let mut usage = AiUsage::default();
        for _ in 0..MAX_ITERATIONS {
            let mut stream = match driver.chat_stream(&conversation, &tools, options).await {
                Ok(stream) => stream,
                Err(err) => return sink.fail(err.to_string(), usage).await,
            };

            let mut assistant_text = String::new();
            let mut pending_tools = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(err) => return sink.fail(err.to_string(), usage).await,
                };
                if let Some(chunk_usage) = chunk.usage {
                    usage.add(chunk_usage);
                }
                if let Some(delta) = chunk.text_delta {
                    assistant_text.push_str(&delta);
                    if !sink.send(HarnessEvent::TextDelta { content: delta }).await {
                        return;
                    }
                }
                pending_tools.extend(chunk.tool_calls);
            }

            if pending_tools.is_empty() {
                return sink.finish(usage).await;
            }

            conversation.push(AiMessage {
                role: "assistant".to_string(),
                content: (!assistant_text.is_empty()).then_some(assistant_text),
                tool_calls: Some(pending_tools.clone()),
                tool_call_id: None,
            });
            let mut ends_turn = false;
            for tool in pending_tools {
                match handle_tool_call(&ctx, &registry, &sink, &mut conversation, tool).await {
                    ToolFlow::Disconnected => return,
                    ToolFlow::EndTurn => ends_turn = true,
                    ToolFlow::Continue => {}
                }
            }
            if ends_turn {
                return sink.finish(usage).await;
            }
        }
        sink.finish(usage).await;
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
            } => {
                answer.usage = AiUsage {
                    prompt_tokens,
                    completion_tokens,
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
        })
        .unwrap();
        assert_eq!(uso["type"], "usage");
        assert_eq!(uso["promptTokens"], 1200);
        assert_eq!(uso["completionTokens"], 80);

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
