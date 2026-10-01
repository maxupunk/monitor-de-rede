//! A IA e os equipamentos: todo acesso passa pelo usuário, com o aviso de
//! alucinação — a menos que o modo "Aceitar automaticamente" esteja ligado
//! para aquela conversa e aquele equipamento.

use std::sync::Mutex;

use async_trait::async_trait;
use axum::{routing::get, Router};
use backend::{
    app::App,
    dtos::ai::{ChatMessageInput, TestConnectionResponse},
    models::{devices, plugin_runs},
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
                tools::ToolPolicy,
            },
            settings::AiSettings,
        },
        audit::AuditActor,
        plugins::{auto_accept, credentials},
        shared::errors::AppResult,
        users::Role,
    },
};
use futures::StreamExt;
use loco_rs::{prelude::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, PaginatorTrait, Set};
use serde_json::json;
use serial_test::serial;

use super::prepare_data;

/// Chama a ferramenta na primeira rodada e encerra na segunda.
struct Roteiro(Mutex<Vec<AiChatChunk>>);

#[async_trait]
impl AiDriver for Roteiro {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn display_name(&self) -> &'static str {
        "Fake"
    }

    async fn chat_stream(
        &self,
        _messages: &[AiMessage],
        _tools: &[AiTool],
        _options: AiChatOptions,
    ) -> AppResult<AiChunkStream> {
        let resposta = self.0.lock().unwrap().pop().unwrap_or(AiChatChunk {
            text_delta: Some("pronto".into()),
            ..AiChatChunk::default()
        });
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

async fn cenario(ctx: &AppContext) -> (devices::Model, i64) {
    let app = Router::new().route(
        "/",
        get(|| async { "<html><title>Roteador de Teste</title></html>" }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let device = devices::ActiveModel {
        name: Set("Roteador".into()),
        r#type: Set("router".into()),
        status: Set("online".into()),
        ip_address: Set(Some("127.0.0.1".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
    credentials::upsert(
        &ctx.db,
        device.id,
        serde_json::from_value(json!({
            "kind": "http", "username": "admin", "secret": "s3nh4", "storage": "vault", "port": port
        }))
        .unwrap(),
    )
    .await
    .unwrap();

    let user = prepare_data::create_user(ctx, "admin", "admin@loco.com", "Admin-1234").await;
    let mut active = user.into_active_model();
    active.role = Set(Role::Admin.as_str().to_string());
    let user = active.update(&ctx.db).await.unwrap();
    (device, user.id)
}

async fn conversar(
    ctx: &AppContext,
    user_id: i64,
    conversation: Option<&str>,
) -> Vec<HarnessEvent> {
    let chamada = AiChatChunk {
        tool_calls: vec![AiToolCall {
            id: "c-1".into(),
            name: "device_http_request".into(),
            arguments: json!({
                "device": "Roteador", "path": "/", "effect": "read",
                "reason": "ler o título da página para identificar o modelo"
            })
            .to_string(),
        }],
        ..AiChatChunk::default()
    };
    let settings = AiSettings::default();
    run_agent_loop(
        ctx.clone(),
        settings.clone(),
        Box::new(Roteiro(Mutex::new(vec![chamada]))),
        AgentRequest {
            messages: vec![ChatMessageInput {
                role: "user".into(),
                content: "abra a página web do roteador e me diga o modelo".into(),
            }],
            context: ChatContext::default(),
            policy: ToolPolicy::from_settings(&settings),
            tool_loading: ToolLoading::Everything,
            memory: ConversationMemory {
                conversation_key: conversation.map(str::to_owned),
                ..ConversationMemory::default()
            },
            actor: AuditActor {
                user_id: Some(user_id),
                ..AuditActor::default()
            },
        },
    )
    .await
    .collect()
    .await
}

#[tokio::test]
#[serial]
async fn acesso_da_ia_pede_confirmacao_com_aviso_e_motivo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (_, user_id) = cenario(&ctx).await;
        let eventos = conversar(&ctx, user_id, Some("conversa-1")).await;

        assert!(eventos.iter().any(|evento| matches!(
            evento,
            HarnessEvent::Notice { message } if message.contains("alucinar")
        )));
        let resumo = eventos
            .iter()
            .find_map(|evento| match evento {
                HarnessEvent::ConfirmationRequired { name, summary, .. }
                    if name == "device_http_request" =>
                {
                    Some(summary.clone())
                }
                _ => None,
            })
            .expect("pedido de confirmação");
        assert!(resumo.contains("GET /"), "{resumo}");
        assert!(resumo.contains("Motivo: ler o título"), "{resumo}");
        assert_eq!(
            plugin_runs::Entity::find().count(&ctx.db).await.unwrap(),
            0,
            "nada tocou o equipamento sem o usuário"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn modo_automatico_executa_so_na_conversa_aceita() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (device, user_id) = cenario(&ctx).await;
        auto_accept::accept(&ctx.db, "conversa-1", device.id, Some(user_id), true)
            .await
            .unwrap();

        // Outra conversa continua pedindo confirmação.
        let outra = conversar(&ctx, user_id, Some("conversa-2")).await;
        assert!(outra
            .iter()
            .any(|evento| matches!(evento, HarnessEvent::ConfirmationRequired { .. })));

        let eventos = conversar(&ctx, user_id, Some("conversa-1")).await;
        assert!(eventos.iter().any(|evento| matches!(
            evento,
            HarnessEvent::ToolCall {
                auto_approved: true,
                ..
            }
        )));
        let resultado = eventos
            .iter()
            .find_map(|evento| match evento {
                HarnessEvent::ToolResult { result, .. } => Some(result.clone()),
                _ => None,
            })
            .expect("resultado da ferramenta");
        assert_eq!(resultado["status"], "succeeded", "{resultado}");
        assert!(resultado["output"]["body"]
            .as_str()
            .unwrap()
            .contains("Roteador de Teste"));
        assert!(!resultado.to_string().contains("s3nh4"));
    })
    .await;
}
