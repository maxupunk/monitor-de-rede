//! `@` do chat e a pergunta da IA antes de prosseguir.
//!
//! O que se afirma aqui: a lista do `@` encontra dispositivos, monitores e
//! fontes pelo que o usuário digitou; o que foi marcado vai para o prompt como
//! alvo da pergunta (e o que sumiu do banco é dito como tal); e o `ask_user`
//! encerra a rodada sem uma segunda chamada ao provedor.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use backend::{
    app::App,
    dtos::ai::{AiMention, AiMentionKind, ChatMessageInput, TestConnectionResponse},
    models::_entities::devices,
    services::{
        ai::{
            drivers::traits::{
                AiChatChunk, AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool, AiToolCall,
            },
            harness::{
                agent::{
                    run_agent_loop, AgentRequest, ConversationMemory, HarnessEvent, ToolLoading,
                },
                prompt::{build_system_prompt, ChatContext},
                tools::{ToolGroups, ToolPolicy},
            },
            mentions,
            settings::AiSettings,
        },
        shared::errors::AppResult,
    },
};
use futures::StreamExt;
use loco_rs::{prelude::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

async fn aparelho(ctx: &AppContext, nome: &str, ip: &str) -> devices::Model {
    devices::ActiveModel {
        name: Set(nome.into()),
        r#type: Set("controller".into()),
        ip_address: Set(Some(ip.into())),
        status: Set("up".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

fn marcado(kind: AiMentionKind, id: &str, label: &str) -> AiMention {
    AiMention {
        kind,
        id: id.into(),
        label: label.into(),
        detail: None,
    }
}

#[tokio::test]
#[serial]
async fn lista_do_arroba_encontra_dispositivo_e_fonte_pelo_que_foi_digitado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        aparelho(&ctx, "Borda", "10.0.0.1").await;
        let mppt = aparelho(&ctx, "MPPT Bateria", "10.0.0.9").await;
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);

        let resposta = request
            .get("/api/ai/mentions?q=mppt")
            .add_header(h.clone(), v.clone())
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let lista: Value = resposta.json();
        let lista = lista.as_array().unwrap();
        assert_eq!(lista.len(), 1, "só o MPPT casa: {lista:?}");
        assert_eq!(lista[0]["kind"], "device");
        assert_eq!(lista[0]["id"], mppt.id.to_string());
        assert!(lista[0]["detail"].as_str().unwrap().contains("10.0.0.9"));

        let fontes: Value = request
            .get("/api/ai/mentions?q=syslog")
            .add_header(h, v)
            .await
            .json();
        assert_eq!(fontes[0]["kind"], "source");
        assert_eq!(fontes[0]["id"], "logs");

        let tudo = mentions::search(&ctx.db, "").await.unwrap();
        assert!(tudo.iter().any(|m| m.label == "Borda"));
        assert!(tudo.iter().any(|m| m.kind == AiMentionKind::Source));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn marcado_vai_para_o_prompt_como_alvo_da_pergunta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let borda = aparelho(&ctx, "Borda", "10.0.0.1").await;
        let mppt = aparelho(&ctx, "MPPT Bateria", "10.0.0.9").await;

        let contexto = ChatContext {
            device_id: Some(borda.id),
            mentions: vec![
                marcado(AiMentionKind::Device, &mppt.id.to_string(), "MPPT Bateria"),
                marcado(AiMentionKind::Source, "logs", "Logs"),
                marcado(AiMentionKind::Device, "999999", "Apagado"),
            ],
            ..ChatContext::default()
        };
        let prompt = build_system_prompt(
            &ctx.db,
            &AiSettings::default(),
            ToolPolicy::passive(),
            &contexto,
            &ToolGroups::new(),
        )
        .await;

        let marcados = prompt
            .split("MARCADOS COM @")
            .nth(1)
            .expect("seção dos marcados");
        let marcados = marcados.split("CONTEXTO DA TELA").next().unwrap();
        assert!(marcados.contains("MPPT Bateria"), "{prompt}");
        assert!(marcados.contains("10.0.0.9"));
        assert!(marcados.contains("grep source='logs'"));
        assert!(marcados.contains("'Apagado' (marcado, mas não existe mais)"));
        assert!(
            prompt.contains("CONTEXTO DA TELA"),
            "a tela continua como contexto secundário"
        );
        assert!(
            prompt.contains("ask_user"),
            "a regra de perguntar está no método"
        );
    })
    .await;
}

/// Pede `ask_user` na primeira rodada e conta as chamadas recebidas.
struct Pergunta(Arc<AtomicUsize>);

#[async_trait]
impl AiDriver for Pergunta {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn display_name(&self) -> &'static str {
        "Fake"
    }

    async fn chat_stream(
        &self,
        _messages: &[AiMessage],
        tools: &[AiTool],
        _options: AiChatOptions,
    ) -> AppResult<AiChunkStream> {
        self.0.fetch_add(1, Ordering::SeqCst);
        assert!(tools.iter().any(|tool| tool.function.name == "ask_user"));
        let chunk = AiChatChunk {
            tool_calls: vec![AiToolCall {
                id: "q1".into(),
                name: "ask_user".into(),
                arguments: json!({
                    "question": "De qual equipamento?",
                    "options": ["Borda", "MPPT Bateria", "Borda"]
                })
                .to_string(),
            }],
            ..AiChatChunk::default()
        };
        Ok(Box::pin(futures::stream::iter(vec![Ok(chunk)])))
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

#[tokio::test]
#[serial]
async fn pergunta_ao_usuario_encerra_a_rodada_sem_nova_chamada() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let chamadas = Arc::new(AtomicUsize::new(0));
        let stream = run_agent_loop(
            ctx.clone(),
            AiSettings::default(),
            Box::new(Pergunta(chamadas.clone())),
            AgentRequest {
                messages: vec![ChatMessageInput {
                    role: "user".into(),
                    content: "e a tensão da bateria?".into(),
                }],
                context: ChatContext::default(),
                policy: ToolPolicy::from_settings(&AiSettings::default()),
                tool_loading: ToolLoading::OnDemand,
                memory: ConversationMemory::default(),
            },
        )
        .await;
        let eventos: Vec<HarnessEvent> = stream.collect().await;

        assert_eq!(
            chamadas.load(Ordering::SeqCst),
            1,
            "não chama o provedor de novo"
        );
        let pergunta = eventos
            .iter()
            .find_map(|evento| match evento {
                HarnessEvent::ToolResult { name, result, .. } if name == "ask_user" => {
                    Some(result.clone())
                }
                _ => None,
            })
            .expect("a pergunta chega à tela");
        assert_eq!(pergunta["question"], "De qual equipamento?");
        assert_eq!(pergunta["options"], json!(["Borda", "MPPT Bateria"]));
        assert!(matches!(eventos.last(), Some(HarnessEvent::Done)));
    })
    .await;
}
