//! Laya (via Ollaya) decidindo quais ferramentas vão para a IA do chat.
//!
//! O que se afirma aqui, contra um Ollaya falso em `127.0.0.1`: a chave vem
//! de `OLLAYA_API_KEY`, nunca da tela; o autocomplete junta o catálogo ao que
//! está instalado; o teste da tela mostra a probabilidade de cada grupo; e no
//! chat, um grupo que as palavras-chave não acharam chega ao provedor porque
//! o Laya o indicou.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::{
    app::App,
    dtos::ai::{ChatMessageInput, LayaModelsResponse, TestConnectionResponse, TestLayaResponse},
    services::{
        ai::{
            drivers::traits::{
                AiChatChunk, AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool,
            },
            harness::{
                agent::{run_agent_loop, AgentRequest, ConversationMemory, ToolLoading},
                prompt::ChatContext,
                tools::{ToolGroup, ToolPolicy},
                turn::preselect_groups,
            },
            laya::config::AiLayaSettings,
            settings::{self, AiSettings},
        },
        shared::errors::AppResult,
    },
};
use futures::StreamExt;
use loco_rs::testing::prelude::*;
use serde_json::json;
use serial_test::serial;

use super::{
    fake_ollaya::{self, by_keyword, with_key},
    prepare_data,
};

/// Acha que toda pergunta é sobre Docker.
async fn fake_ollaya() -> String {
    fake_ollaya::start(by_keyword(&[("", "docker")]))
        .await
        .base_url
}

fn laya(base_url: String) -> AiLayaSettings {
    AiLayaSettings {
        enabled: true,
        base_url,
        timeout_ms: 5_000,
        ..AiLayaSettings::default()
    }
}

#[tokio::test]
#[serial]
async fn autocomplete_junta_catalogo_e_instalados() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        with_key();
        let base_url = fake_ollaya().await;
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);
        let response = request
            .get(&format!("/api/ai/laya/models?base_url={base_url}"))
            .add_header(h, v)
            .await;
        response.assert_status_ok();
        let models: LayaModelsResponse = response.json();

        assert!(models.online, "{:?}", models.error_message);
        assert_eq!(models.installed, ["laya:multilingual"]);
        let installed: Vec<&str> = models
            .options
            .iter()
            .filter(|option| option.installed)
            .map(|option| option.name.as_str())
            .collect();
        // O roteador `laya` casa com a variante instalada.
        assert_eq!(installed, ["laya", "laya:multilingual"]);
        assert!(models
            .options
            .iter()
            .any(|option| option.name == "laya:en" && !option.installed));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn teste_da_tela_usa_a_chave_do_ambiente_e_mostra_cada_grupo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        with_key();
        let base_url = fake_ollaya().await;
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);
        let response = request
            .post("/api/ai/laya/test")
            .add_header(h, v)
            .json(&json!({ "laya": laya(base_url), "question": "e o financeiro?" }))
            .await;
        response.assert_status_ok();
        let result: TestLayaResponse = response.json();

        assert!(result.success, "{}", result.message);
        assert_eq!(result.model.as_deref(), Some("laya:multilingual"));
        assert_eq!(result.groups.len(), ToolGroup::DEFERRED.len());
        let selected: Vec<&str> = result
            .groups
            .iter()
            .filter(|group| group.selected)
            .map(|group| group.id.as_str())
            .collect();
        assert_eq!(selected, ["docker"]);
    })
    .await;
}

/// Anota as ferramentas recebidas e responde texto.
struct Anota(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl AiDriver for Anota {
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
        *self.0.lock().unwrap() = tools
            .iter()
            .map(|tool| tool.function.name.clone())
            .collect();
        let chunk = AiChatChunk {
            text_delta: Some("Tudo de pé.".into()),
            finish_reason: Some("stop".into()),
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
async fn grupo_indicado_pelo_laya_chega_ao_provedor() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let question = "a aplicação do financeiro está de pé?";
        assert!(
            !preselect_groups(question, &[]).contains(&ToolGroup::Docker),
            "a pergunta não pode achar Docker pelas palavras-chave, senão o teste não prova nada"
        );

        with_key();
        let settings = AiSettings {
            laya: laya(fake_ollaya().await),
            ..AiSettings::default()
        };
        let tools = Arc::new(Mutex::new(Vec::new()));
        run_agent_loop(
            ctx.clone(),
            settings.clone(),
            Box::new(Anota(tools.clone())),
            AgentRequest {
                messages: vec![ChatMessageInput {
                    role: "user".into(),
                    content: question.into(),
                }],
                context: ChatContext::default(),
                policy: ToolPolicy::from_settings(&settings),
                tool_loading: ToolLoading::OnDemand,
                memory: ConversationMemory::default(),
                actor: backend::services::audit::AuditActor::default(),
            },
        )
        .await
        .collect::<Vec<_>>()
        .await;

        let tools = tools.lock().unwrap().clone();
        assert!(
            tools.iter().any(|name| name == "get_docker_containers"),
            "{tools:?}"
        );
        assert!(
            !tools.iter().any(|name| name == "ping_host"),
            "só o que o Laya indicou entra: {tools:?}"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn laya_e_provedor_gravam_cada_um_a_sua_parte() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);

        let response = request
            .put("/api/ai/laya")
            .add_header(h.clone(), v.clone())
            .json(&laya("http://ollaya:11435".into()))
            .await;
        response.assert_status_ok();
        assert!(settings::load(&ctx.db).await.unwrap().laya.enabled);

        // O card do provedor manda a própria cópia, com o Laya desligado:
        // a gravação dele não pode desfazer o que o card do Laya salvou.
        let mut provider = serde_json::to_value(AiSettings::default()).unwrap();
        provider["responseStyle"] = json!("normal");
        request
            .put("/api/ai/settings")
            .add_header(h, v)
            .json(&provider)
            .await
            .assert_status_ok();

        let stored = settings::load(&ctx.db).await.unwrap();
        assert!(stored.laya.enabled, "o Laya continua ligado");
        assert_eq!(
            serde_json::to_value(stored.response_style).unwrap(),
            json!("normal")
        );
    })
    .await;
}
