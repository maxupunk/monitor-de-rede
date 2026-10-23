//! Decisor (via Ollaya) decidindo quais ferramentas vão para a IA do chat.
//!
//! O que se afirma aqui, contra um Ollaya falso em `127.0.0.1`: a chave vem
//! de `OLLAYA_API_KEY`, nunca da tela; o autocomplete junta o catálogo ao que
//! está instalado; o teste da tela mostra a probabilidade de cada grupo; e no
//! chat, um grupo que as palavras-chave não acharam chega ao provedor porque
//! o Decisor o indicou.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::{
    app::App,
    dtos::ai::{
        ChatMessageInput, DeciderModelsResponse, TestConnectionResponse, TestDeciderResponse,
    },
    services::{
        ai::{
            decider::config::AiDeciderSettings,
            drivers::traits::{
                AiChatChunk, AiChatOptions, AiChunkStream, AiDriver, AiMessage, AiTool,
            },
            harness::{
                agent::{run_agent_loop, AgentRequest, ConversationMemory, ToolLoading},
                prompt::ChatContext,
                tools::{ToolGroup, ToolPolicy},
            },
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

fn decider(base_url: String) -> AiDeciderSettings {
    AiDeciderSettings {
        enabled: true,
        base_url,
        // O que o Ollaya falso tem instalado (o padrão do Decisor é outro).
        model: "laya".into(),
        timeout_ms: 5_000,
        ..AiDeciderSettings::default()
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
            .post("/api/ai/decider/models")
            .add_header(h, v)
            .json(&json!({ "decider": decider(base_url) }))
            .await;
        response.assert_status_ok();
        let models: DeciderModelsResponse = response.json();

        assert!(models.online, "{:?}", models.error_message);
        assert_eq!(models.installed, ["laya:multilingual"]);
        let installed: Vec<&str> = models
            .options
            .iter()
            .filter(|option| option.installed)
            .map(|option| option.name.as_str())
            .collect();
        // O roteador `laya` casa com a variante instalada; os dois vão para o
        // fim da lista (perto do chute), da maior nota para a menor.
        assert_eq!(installed, ["laya:multilingual", "laya"]);
        assert_eq!(
            models.options[0].name, "netmonitor/decima-network:v9",
            "o recomendado em CPU abre a lista"
        );
        assert!(
            models.options[0].decisions.is_some(),
            "com a nota do benchmark do NetMonitor"
        );
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
            .post("/api/ai/decider/test")
            .add_header(h, v)
            .json(&json!({ "decider": decider(base_url), "question": "e o financeiro?" }))
            .await;
        response.assert_status_ok();
        let result: TestDeciderResponse = response.json();

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

/// As ferramentas que chegam ao provedor numa pergunta do chat.
async fn tools_sent(
    ctx: &loco_rs::app::AppContext,
    settings: AiSettings,
    question: &str,
) -> Vec<String> {
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
    let sent = tools.lock().unwrap().clone();
    sent
}

#[tokio::test]
#[serial]
async fn com_decisor_so_vai_o_que_ele_escolheu() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        with_key();
        let settings = AiSettings {
            decider: decider(fake_ollaya().await),
            ..AiSettings::default()
        };
        let tools = tools_sent(&ctx, settings, "a aplicação do financeiro está de pé?").await;
        assert!(
            tools.iter().any(|name| name == "get_docker_containers"),
            "{tools:?}"
        );
        assert!(
            !tools.iter().any(|name| name == "get_monitor_history"),
            "só o que o Decisor indicou entra: {tools:?}"
        );
        assert!(
            tools.iter().any(|name| name == "load_tools"),
            "se ele errar para menos, a IA ainda carrega o resto: {tools:?}"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sem_decisor_vai_o_catalogo_inteiro() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        // Desligado na tela e ligado sem servidor no ar: nos dois casos não há
        // quem escolha, e nada pode ficar de fora.
        let fora_do_ar = AiDeciderSettings {
            enabled: true,
            base_url: "http://192.0.2.1:9".into(),
            timeout_ms: 500,
            ..AiDeciderSettings::default()
        };
        for decider in [AiDeciderSettings::default(), fora_do_ar] {
            let settings = AiSettings {
                decider,
                ..AiSettings::default()
            };
            let tools = tools_sent(&ctx, settings, "a aplicação do financeiro está de pé?").await;
            for esperada in [
                "get_docker_containers",
                "get_monitor_history",
                "get_audit_log",
                "list_scheduled_tasks",
            ] {
                assert!(
                    tools.iter().any(|name| name == esperada),
                    "{esperada}: {tools:?}"
                );
            }
            assert!(
                !tools.iter().any(|name| name == "load_tools"),
                "não sobra nada a carregar: {tools:?}"
            );
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn decider_e_provedor_gravam_cada_um_a_sua_parte() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);

        let response = request
            .put("/api/ai/decider")
            .add_header(h.clone(), v.clone())
            .json(&decider("http://ollaya:11435".into()))
            .await;
        response.assert_status_ok();
        assert!(settings::load(&ctx.db).await.unwrap().decider.enabled);

        // O card do provedor manda a própria cópia, com o Decisor desligado:
        // a gravação dele não pode desfazer o que o card do Decisor salvou.
        let mut provider = serde_json::to_value(AiSettings::default()).unwrap();
        provider["responseStyle"] = json!("normal");
        request
            .put("/api/ai/settings")
            .add_header(h, v)
            .json(&provider)
            .await
            .assert_status_ok();

        let stored = settings::load(&ctx.db).await.unwrap();
        assert!(stored.decider.enabled, "o Decisor continua ligado");
        assert_eq!(
            serde_json::to_value(stored.response_style).unwrap(),
            json!("normal")
        );
    })
    .await;
}
