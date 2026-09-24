//! Janela de contexto: a consulta aos provedores e a compactação da conversa.
//!
//! O que se afirma aqui: as três fontes (Ollama, OpenRouter, models.dev) são
//! lidas por HTTP de verdade, contra um servidor local com as respostas no
//! formato de cada provedor; a conversa que passa do gatilho vira resumo, o
//! resumo vai no prompt e o histórico enviado encolhe; sem resumo do
//! provedor, as mensagens antigas saem mesmo assim; conversa curta não é
//! compactada, a não ser que a tela peça.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::{
    routing::{get, post},
    Json, Router,
};
use backend::{
    app::App,
    dtos::ai::{ChatMessageInput, TestConnectionResponse},
    services::{
        ai::{
            context_window::{
                self,
                sources::{LearnedWindow, ModelsDevLookup, OllamaLookup, OpenRouterLookup},
            },
            drivers::traits::{
                AiChatChunk, AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool, AiUsage,
            },
            harness::{
                agent::{
                    run_agent_loop, AgentRequest, ConversationMemory, HarnessEvent, ToolLoading,
                },
                prompt::ChatContext,
                tools::ToolPolicy,
            },
            settings::AiSettings,
        },
        shared::errors::{AppError, AppResult},
    },
};
use futures::StreamExt;
use loco_rs::{prelude::AppContext, testing::prelude::*};
use serde_json::{json, Value};
use serial_test::serial;

/// Sobe um servidor HTTP local e devolve a URL base.
async fn servidor(rotas: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("porta local");
    let endereco = listener.local_addr().expect("endereço");
    tokio::spawn(async move {
        let _ = axum::serve(listener, rotas).await;
    });
    format!("http://{endereco}")
}

#[tokio::test]
async fn ollama_informa_a_janela_do_modelo_carregado() {
    let base = servidor(
        Router::new()
            .route(
                "/api/ps",
                get(|| async {
                    Json(json!({ "models": [
                        { "name": "qwen3:8b", "model": "qwen3:8b", "context_length": 16_384 }
                    ]}))
                }),
            )
            .route(
                "/api/show",
                post(|Json(corpo): Json<Value>| async move {
                    assert!(corpo["model"].is_string());
                    Json(json!({
                        "parameters": "num_ctx 8192",
                        "model_info": { "qwen3.context_length": 40_960 }
                    }))
                }),
            ),
    )
    .await;

    let fonte = OllamaLookup::from_openai_base(&format!("{base}/v1"));
    assert_eq!(
        context_window::lookup(&fonte, "qwen3:8b").await,
        Some(LearnedWindow::confident(16_384)),
        "o modelo carregado diz a janela em uso"
    );
    assert_eq!(
        context_window::lookup(&fonte, "llama3.2").await,
        Some(LearnedWindow {
            tokens: 8_192,
            confident: false
        }),
        "sem modelo carregado vale o num_ctx, mas como presumido"
    );
}

#[tokio::test]
async fn openrouter_e_models_dev_informam_o_catalogo() {
    let base = servidor(
        Router::new()
            .route(
                "/api/v1/models",
                get(|| async {
                    Json(json!({ "data": [
                        { "id": "openrouter/free", "context_length": 200_000 },
                        { "id": "meta/llama-3.3", "context_length": 131_072,
                          "top_provider": { "context_length": 65_536 } }
                    ]}))
                }),
            )
            .route(
                "/api.json",
                get(|| async {
                    Json(json!({ "opencode": { "models": {
                        "muse-spark-1.3-contributor-free": { "limit": { "context": 1_048_576 } }
                    }}}))
                }),
            ),
    )
    .await;

    let openrouter = OpenRouterLookup {
        base_url: format!("{base}/api/v1"),
        api_key: Some("chave".into()),
    };
    assert_eq!(
        context_window::lookup(&openrouter, "meta/llama-3.3")
            .await
            .map(|janela| janela.tokens),
        Some(65_536)
    );
    assert_eq!(
        context_window::lookup(&openrouter, "openrouter/free")
            .await
            .map(|janela| janela.tokens),
        Some(200_000)
    );

    let models_dev = ModelsDevLookup {
        url: format!("{base}/api.json"),
        provider: "opencode".into(),
    };
    assert_eq!(
        context_window::lookup(&models_dev, "muse-spark-1.3-contributor-free")
            .await
            .map(|janela| janela.tokens),
        Some(1_048_576)
    );
    assert_eq!(
        context_window::lookup(&models_dev, "inexistente").await,
        None
    );
}

#[tokio::test]
async fn provedor_fora_do_ar_nao_quebra_a_consulta() {
    // Porta fechada: a consulta devolve `None` e o chat cai na estimativa.
    let fonte = OllamaLookup {
        base_url: "http://127.0.0.1:9".into(),
    };
    assert_eq!(context_window::lookup(&fonte, "llama3.2").await, None);
}

/// O que o provedor recebeu numa chamada de conversa (não de resumo).
#[derive(Debug, Clone)]
struct Chamada {
    prompt: String,
    mensagens: usize,
}

/// Provedor com janela fixa. Responde ao pedido de resumo (reconhecido pelo
/// prompt) com `resumo`, ou falha quando ele é `None`.
struct Provedor {
    janela: u64,
    resumo: Option<String>,
    chamadas: Arc<Mutex<Vec<Chamada>>>,
}

#[async_trait]
impl AiDriver for Provedor {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn display_name(&self) -> &'static str {
        "Fake"
    }

    fn model(&self) -> Option<&str> {
        Some("modelo-de-teste")
    }

    async fn context_window(&self, _model: &str) -> Option<LearnedWindow> {
        Some(LearnedWindow::confident(self.janela))
    }

    async fn chat_stream(
        &self,
        messages: &[AiMessage],
        _tools: &[AiTool],
        _options: AiChatOptions,
    ) -> AppResult<AiChunkStream> {
        let prompt = messages[0].content.clone().unwrap_or_default();
        if prompt.starts_with("Você resume") {
            let Some(resumo) = self.resumo.clone() else {
                return Err(AppError::service_unavailable("provedor indisponível"));
            };
            let pedaco = AiChatChunk {
                text_delta: Some(resumo),
                ..AiChatChunk::default()
            };
            return Ok(Box::pin(futures::stream::iter(vec![Ok(pedaco)])));
        }
        self.chamadas.lock().unwrap().push(Chamada {
            prompt,
            mensagens: messages.len(),
        });
        let pedacos = vec![
            Ok(AiChatChunk {
                text_delta: Some("ok".into()),
                ..AiChatChunk::default()
            }),
            Ok(AiChatChunk {
                usage: Some(AiUsage {
                    prompt_tokens: 1000,
                    completion_tokens: 10,
                }),
                ..AiChatChunk::default()
            }),
        ];
        Ok(Box::pin(futures::stream::iter(pedacos)))
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

fn conversa_longa(trocas: usize) -> Vec<ChatMessageInput> {
    let mut mensagens = Vec::new();
    for indice in 0..trocas {
        mensagens.push(ChatMessageInput {
            role: "user".into(),
            content: format!("pergunta {indice}: {}", "detalhe da rede ".repeat(375)),
        });
        mensagens.push(ChatMessageInput {
            role: "assistant".into(),
            content: format!("resposta {indice}: {}", "diagnóstico longo ".repeat(333)),
        });
    }
    mensagens.push(ChatMessageInput {
        role: "user".into(),
        content: "e o gateway?".into(),
    });
    mensagens
}

async fn conversar(
    ctx: &AppContext,
    janela: u64,
    resumo: Option<&str>,
    mensagens: Vec<ChatMessageInput>,
    memoria: ConversationMemory,
) -> (Vec<Chamada>, Vec<HarnessEvent>) {
    let chamadas = Arc::new(Mutex::new(Vec::new()));
    let provedor = Provedor {
        janela,
        resumo: resumo.map(ToString::to_string),
        chamadas: chamadas.clone(),
    };
    let settings = AiSettings::default();
    let eventos = run_agent_loop(
        ctx.clone(),
        settings.clone(),
        Box::new(provedor),
        AgentRequest {
            messages: mensagens,
            context: ChatContext::default(),
            policy: ToolPolicy::from_settings(&settings),
            tool_loading: ToolLoading::OnDemand,
            memory: memoria,
        },
    )
    .await
    .collect()
    .await;
    let chamadas = chamadas.lock().unwrap().clone();
    (chamadas, eventos)
}

fn compactacao(eventos: &[HarnessEvent]) -> Option<(String, usize, u64, u64, bool)> {
    eventos.iter().find_map(|evento| match evento {
        HarnessEvent::ContextCompacted {
            summary,
            folded_messages,
            tokens_before,
            tokens_after,
            summarized,
        } => Some((
            summary.clone(),
            *folded_messages,
            *tokens_before,
            *tokens_after,
            *summarized,
        )),
        _ => None,
    })
}

#[tokio::test]
#[serial]
async fn conversa_acima_do_gatilho_vira_resumo_e_o_envio_encolhe() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let mensagens = conversa_longa(8);
        let total = mensagens.len();
        let (chamadas, eventos) = conversar(
            &ctx,
            24_000,
            Some("- borda com perda de 20% desde ontem"),
            mensagens,
            ConversationMemory::default(),
        )
        .await;

        let (resumo, dobradas, antes, depois, resumido) =
            compactacao(&eventos).expect("a conversa passou do gatilho");
        assert!(resumido);
        assert_eq!(resumo, "- borda com perda de 20% desde ontem");
        assert!(dobradas >= 2 && dobradas < total, "{dobradas} de {total}");
        assert!(antes >= 19_200, "passou dos 80% da janela: {antes}");
        assert!(depois <= 12_000, "voltou para até 50% da janela: {depois}");

        let chamada = &chamadas[0];
        assert!(chamada.prompt.contains("RESUMO DA CONVERSA ATÉ AQUI"));
        assert!(chamada.prompt.contains("borda com perda de 20%"));
        // Sistema + o que sobrou da conversa.
        assert_eq!(chamada.mensagens, 1 + total - dobradas);

        let janela = eventos.iter().find_map(|evento| match evento {
            HarnessEvent::Usage {
                context_window,
                context_window_reported,
                ..
            } => Some((*context_window, *context_window_reported)),
            _ => None,
        });
        assert_eq!(
            janela,
            Some((Some(24_000), true)),
            "janela informada pelo provedor"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sem_resumo_do_provedor_as_mensagens_antigas_saem_mesmo_assim() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (chamadas, eventos) = conversar(
            &ctx,
            24_000,
            None,
            conversa_longa(8),
            ConversationMemory {
                summary: Some("resumo de antes".into()),
                ..ConversationMemory::default()
            },
        )
        .await;
        let (resumo, dobradas, _, _, resumido) = compactacao(&eventos).expect("compactou");
        assert!(!resumido);
        assert!(resumo.starts_with("resumo de antes\n"), "{resumo}");
        assert!(resumo.contains(&format!("{dobradas} mensagens")));
        assert!(
            chamadas.len() == 1,
            "a pergunta foi respondida mesmo sem resumo"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn conversa_curta_so_compacta_quando_a_tela_pede() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let curta = || {
            vec![
                ChatMessageInput {
                    role: "user".into(),
                    content: "status da rede".into(),
                },
                ChatMessageInput {
                    role: "assistant".into(),
                    content: "tudo no ar".into(),
                },
                ChatMessageInput {
                    role: "user".into(),
                    content: "e a borda?".into(),
                },
            ]
        };
        let (_, eventos) = conversar(
            &ctx,
            128_000,
            Some("resumo"),
            curta(),
            ConversationMemory {
                summary: Some("já conversamos sobre a VPN".into()),
                ..ConversationMemory::default()
            },
        )
        .await;
        assert!(compactacao(&eventos).is_none());

        let (chamadas, eventos) = conversar(
            &ctx,
            128_000,
            Some("- status verificado, tudo no ar"),
            curta(),
            ConversationMemory {
                force_compaction: true,
                ..ConversationMemory::default()
            },
        )
        .await;
        let (_, dobradas, _, _, _) = compactacao(&eventos).expect("a tela pediu");
        assert_eq!(dobradas, 2);
        assert_eq!(chamadas[0].mensagens, 2, "sistema + a pergunta atual");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn resumo_anterior_vai_no_prompt() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (chamadas, _) = conversar(
            &ctx,
            128_000,
            None,
            vec![ChatMessageInput {
                role: "user".into(),
                content: "e o link da filial?".into(),
            }],
            ConversationMemory {
                summary: Some("filial norte com link instável".into()),
                ..ConversationMemory::default()
            },
        )
        .await;
        assert!(chamadas[0]
            .prompt
            .contains("filial norte com link instável"));
    })
    .await;
}
