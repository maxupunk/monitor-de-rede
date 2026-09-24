//! Quanto o chat manda ao provedor.
//!
//! Um "oi" já custou 22 mil tokens: o método mandava começar consultando e o
//! catálogo inteiro ia em cada uma das quatro rodadas. O que se afirma aqui:
//! cortesia vai sem ferramenta e com prompt curto; pergunta de verdade leva só
//! o núcleo e o que ela obviamente pede, e o resto chega com `load_tools`, sem
//! aparecer na tela; rotina automática continua com o catálogo inteiro.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::{
    app::App,
    dtos::ai::{ChatMessageInput, TestConnectionResponse},
    services::{
        ai::{
            drivers::traits::{
                AiChatChunk, AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool, AiToolCall,
            },
            harness::{
                agent::{
                    run_agent_loop, AgentRequest, ConversationMemory, HarnessEvent, ToolLoading,
                },
                prompt::ChatContext,
                tools::{ToolPolicy, LOAD_TOOLS},
            },
            settings::AiSettings,
        },
        shared::errors::AppResult,
    },
};
use futures::StreamExt;
use loco_rs::{prelude::AppContext, testing::prelude::*};
use serde_json::json;
use serial_test::serial;

/// O que o provedor recebeu numa rodada.
#[derive(Debug, Clone)]
struct Rodada {
    ferramentas: Vec<String>,
    prompt: String,
}

/// Responde o roteiro, uma resposta por rodada, e grava cada pedido.
struct Gravador {
    roteiro: Mutex<Vec<AiChatChunk>>,
    rodadas: Arc<Mutex<Vec<Rodada>>>,
}

impl Gravador {
    fn novo(roteiro: Vec<AiChatChunk>) -> (Self, Arc<Mutex<Vec<Rodada>>>) {
        let rodadas = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                roteiro: Mutex::new(roteiro.into_iter().rev().collect()),
                rodadas: rodadas.clone(),
            },
            rodadas,
        )
    }
}

fn texto(conteudo: &str) -> AiChatChunk {
    AiChatChunk {
        text_delta: Some(conteudo.into()),
        ..AiChatChunk::default()
    }
}

fn chamada(nome: &str, argumentos: serde_json::Value) -> AiChatChunk {
    AiChatChunk {
        tool_calls: vec![AiToolCall {
            id: format!("c-{nome}"),
            name: nome.into(),
            arguments: argumentos.to_string(),
        }],
        ..AiChatChunk::default()
    }
}

#[async_trait]
impl AiDriver for Gravador {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn display_name(&self) -> &'static str {
        "Fake"
    }

    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        tools: &[AiTool],
        _options: AiChatOptions,
    ) -> AppResult<AiChunkStream> {
        self.rodadas.lock().unwrap().push(Rodada {
            ferramentas: tools
                .iter()
                .map(|tool| tool.function.name.clone())
                .collect(),
            prompt: messages[0].content.clone().unwrap_or_default(),
        });
        let resposta = self
            .roteiro
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| texto("fim"));
        Ok(Box::pin(futures::stream::iter(vec![Ok(resposta)])))
    }

    async fn test_connection(&self) -> AppResult<TestConnectionResponse> {
        Ok(TestConnectionResponse {
            success: true,
            latency_ms: 0.0,
            message: String::new(),
            model: None,
        })
    }
}

async fn conversar(
    ctx: &AppContext,
    pergunta: &str,
    roteiro: Vec<AiChatChunk>,
    carga: ToolLoading,
) -> (Vec<Rodada>, Vec<HarnessEvent>) {
    let (driver, rodadas) = Gravador::novo(roteiro);
    let settings = AiSettings::default();
    let eventos: Vec<HarnessEvent> = run_agent_loop(
        ctx.clone(),
        settings.clone(),
        Box::new(driver),
        AgentRequest {
            messages: vec![ChatMessageInput {
                role: "user".into(),
                content: pergunta.into(),
            }],
            context: ChatContext::default(),
            policy: ToolPolicy::from_settings(&settings),
            tool_loading: carga,
            memory: ConversationMemory::default(),
        },
    )
    .await
    .collect()
    .await;
    let rodadas = rodadas.lock().unwrap().clone();
    (rodadas, eventos)
}

#[tokio::test]
#[serial]
async fn oi_vai_sem_ferramenta_e_com_prompt_curto() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (rodadas, eventos) = conversar(
            &ctx,
            "Oi",
            vec![texto("Oi! Em que posso ajudar?")],
            ToolLoading::OnDemand,
        )
        .await;

        assert_eq!(rodadas.len(), 1, "uma rodada só");
        assert!(
            rodadas[0].ferramentas.is_empty(),
            "{:?}",
            rodadas[0].ferramentas
        );
        assert!(
            rodadas[0].prompt.len() < 600,
            "prompt de cortesia sem método nem catálogo: {} caracteres",
            rodadas[0].prompt.len()
        );
        assert!(!eventos
            .iter()
            .any(|evento| matches!(evento, HarnessEvent::ToolCall { .. })));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pergunta_leva_o_nucleo_e_carrega_o_resto_sob_demanda() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let roteiro = vec![
            chamada(LOAD_TOOLS, json!({ "groups": ["charts"] })),
            texto("Pronto."),
        ];
        let (rodadas, eventos) = conversar(
            &ctx,
            "quais dispositivos estão offline?",
            roteiro,
            ToolLoading::OnDemand,
        )
        .await;

        let primeira = &rodadas[0].ferramentas;
        assert!(primeira.contains(&"list_devices".to_string()));
        assert!(primeira.contains(&LOAD_TOOLS.to_string()));
        assert!(!primeira.contains(&"chart_monitor_latency".to_string()));
        assert!(rodadas[0].prompt.contains("FERRAMENTAS SOB DEMANDA"));
        assert!(rodadas[0].prompt.contains("charts:"));

        assert!(
            rodadas[1]
                .ferramentas
                .contains(&"chart_monitor_latency".to_string()),
            "carregado vale na rodada seguinte"
        );
        assert!(
            !eventos.iter().any(|evento| matches!(
                evento,
                HarnessEvent::ToolCall { name, .. } if name == LOAD_TOOLS
            )),
            "carregar ferramentas não aparece na tela"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pergunta_que_pede_grafico_ja_chega_com_os_graficos() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (rodadas, _) = conversar(
            &ctx,
            "mostre o gráfico de latência do gateway",
            vec![texto("ok")],
            ToolLoading::OnDemand,
        )
        .await;
        assert!(rodadas[0]
            .ferramentas
            .contains(&"chart_monitor_latency".to_string()));
        assert!(!rodadas[0].prompt.contains("- charts:"), "já carregado");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn rotina_automatica_recebe_o_catalogo_inteiro() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (rodadas, _) = conversar(&ctx, "Oi", vec![texto("ok")], ToolLoading::Everything).await;
        let ferramentas = &rodadas[0].ferramentas;
        assert!(ferramentas.contains(&"analyze_root_cause".to_string()));
        assert!(!ferramentas.contains(&LOAD_TOOLS.to_string()));
        assert!(!rodadas[0].prompt.contains("FERRAMENTAS SOB DEMANDA"));
    })
    .await;
}
