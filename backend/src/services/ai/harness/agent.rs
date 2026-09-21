//! Orquestrador do Agent Harness (ReAct Loop) da IA.
//!
//! Coordena a troca de mensagens com o provedor de IA e a execução iterativa de ferramentas.

use std::pin::Pin;

use chrono::Utc;
use futures::{Stream, StreamExt};
use loco_rs::prelude::AppContext;
use sea_orm::EntityTrait;
use serde::Serialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::tools::{execute_tool, get_available_tools};
use crate::{
    dtos::ai::ChatStreamRequest,
    models::devices,
    services::ai::{
        drivers::traits::{AiDriver, AiMessage},
        settings::AiSettings,
    },
};

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
    },
    Done,
    Error {
        message: String,
    },
}

pub type HarnessEventStream = Pin<Box<dyn Stream<Item = HarnessEvent> + Send>>;

/// Constrói o System Prompt contextualizado para a sessão.
async fn build_system_prompt(
    ctx: &AppContext,
    settings: &AiSettings,
    device_id: Option<i64>,
) -> String {
    let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");

    let mut prompt = format!(
        "Você é o NetMonitor AI, um Engenheiro de Redes Sênior e Especialista em Infraestrutura integrado ao NetMonitor.
Data/Hora atual do sistema: {now}.

Suas atribuições fundamentais:
1. DIAGNÓSTICO DE REDE: Quando o usuário relatar falhas, lentidão, pacotes perdidos ou instabilidade, utilize ativamente as ferramentas de diagnóstico (ping_host, traceroute, scan_ports, run_playbook, get_device_detail, get_active_alerts) para investigar e fundamentar seu diagnóstico em evidências concretas antes de emitir conclusões.
2. DÚVIDAS DO SISTEMA: Quando o usuário tiver dúvidas operacionais sobre como configurar ou utilizar o NetMonitor (monitores, alertas, regras de flapping, WireGuard VPN, probes, descoberta de redes, janelas de manutenção, etc.), forneça instruções claras e objetivas. Utilize a ferramenta 'search_system_docs' sempre que necessário para consultar a documentação interna.
3. CONCISÃO E PRECISÃO: Seja direto, técnico e profissional. Apresente métricas numéricas formatadas (latência em ms, taxas de perda em %, portas abertas) e conclua com recomendações acionáveis de correção.
"
    );

    if let Some(custom) = &settings.custom_system_prompt {
        if !custom.trim().is_empty() {
            prompt.push_str("\nInstruções adicionais do administrador:\n");
            prompt.push_str(custom.trim());
            prompt.push('\n');
        }
    }

    if let Some(dev_id) = device_id {
        if let Ok(Some(dev)) = devices::Entity::find_by_id(dev_id).one(&ctx.db).await {
            let last_seen_str = dev
                .last_seen_at
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "N/A".into());
            prompt.push_str(&format!(
                "\nCONTEXTO DO DISPOSITIVO ATUALMENTE EM VISUALIZAÇÃO:
- Nome: {}
- IP: {}
- Fabricante: {}
- Status atual: {}
- Última atividade: {}
",
                dev.name,
                dev.ip_address.unwrap_or_else(|| "N/A".into()),
                dev.vendor.unwrap_or_else(|| "Desconhecido".into()),
                dev.status,
                last_seen_str
            ));
        }
    }

    prompt
}

/// Executa o loop do Agent Harness transmitindo eventos em tempo real.
pub async fn run_agent_loop(
    ctx: AppContext,
    settings: AiSettings,
    driver: Box<dyn AiDriver>,
    req: ChatStreamRequest,
) -> HarnessEventStream {
    let (sender, receiver) = mpsc::channel(64);

    tokio::spawn(async move {
        let tools = get_available_tools(settings.allow_active_tools);
        let system_prompt = build_system_prompt(&ctx, &settings, req.device_id).await;

        let mut conversation: Vec<AiMessage> = Vec::new();
        conversation.push(AiMessage {
            role: "system".to_string(),
            content: Some(system_prompt),
            tool_calls: None,
            tool_call_id: None,
        });

        for msg in req.messages {
            conversation.push(AiMessage {
                role: msg.role,
                content: Some(msg.content),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        const MAX_ITERATIONS: usize = 5;

        for iteration in 0..MAX_ITERATIONS {
            let mut stream = match driver.chat_stream(&conversation, &tools).await {
                Ok(s) => s,
                Err(err) => {
                    let _ = sender
                        .send(HarnessEvent::Error {
                            message: err.to_string(),
                        })
                        .await;
                    let _ = sender.send(HarnessEvent::Done).await;
                    return;
                }
            };

            let mut assistant_text = String::new();
            let mut pending_tools = Vec::new();

            while let Some(chunk_res) = stream.next().await {
                match chunk_res {
                    Ok(chunk) => {
                        if let Some(delta) = chunk.text_delta {
                            assistant_text.push_str(&delta);
                            if sender
                                .send(HarnessEvent::TextDelta { content: delta })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        if !chunk.tool_calls.is_empty() {
                            pending_tools.extend(chunk.tool_calls);
                        }
                    }
                    Err(err) => {
                        let _ = sender
                            .send(HarnessEvent::Error {
                                message: err.to_string(),
                            })
                            .await;
                        let _ = sender.send(HarnessEvent::Done).await;
                        return;
                    }
                }
            }

            // Se não houver chamadas de ferramentas, a resposta foi concluída
            if pending_tools.is_empty() {
                let _ = sender.send(HarnessEvent::Done).await;
                return;
            }

            // Registra a mensagem do assistente com as tool calls solicitadas
            conversation.push(AiMessage {
                role: "assistant".to_string(),
                content: if assistant_text.is_empty() {
                    None
                } else {
                    Some(assistant_text)
                },
                tool_calls: Some(pending_tools.clone()),
                tool_call_id: None,
            });

            // Executa cada ferramenta e adiciona o resultado
            for tool in pending_tools {
                let args_json: serde_json::Value =
                    serde_json::from_str(&tool.arguments).unwrap_or_else(|_| json!({}));

                if sender
                    .send(HarnessEvent::ToolCall {
                        id: tool.id.clone(),
                        name: tool.name.clone(),
                        arguments: args_json,
                    })
                    .await
                    .is_err()
                {
                    return;
                }

                let tool_result = match execute_tool(&ctx, &tool.name, &tool.arguments).await {
                    Ok(res) => res,
                    Err(err) => json!({ "error": err.to_string() }),
                };

                if sender
                    .send(HarnessEvent::ToolResult {
                        id: tool.id.clone(),
                        name: tool.name.clone(),
                        result: tool_result.clone(),
                    })
                    .await
                    .is_err()
                {
                    return;
                }

                conversation.push(AiMessage {
                    role: "tool".to_string(),
                    content: Some(tool_result.to_string()),
                    tool_calls: None,
                    tool_call_id: Some(tool.id),
                });
            }

            // Próxima iteração continuará o raciocínio com os resultados das ferramentas
            if iteration == MAX_ITERATIONS - 1 {
                let _ = sender.send(HarnessEvent::Done).await;
                return;
            }
        }

        let _ = sender.send(HarnessEvent::Done).await;
    });

    Box::pin(ReceiverStream::new(receiver))
}
