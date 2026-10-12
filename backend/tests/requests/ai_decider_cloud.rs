//! Laya com um provedor de decisão na nuvem.
//!
//! O que se afirma aqui, contra um servidor falso em `127.0.0.1` que fala os
//! três formatos (System One da TypeSafe, Workers AI da Cloudflare e
//! Decisions da OpenAI) e exige a chave: o teste da tela funciona em cada um;
//! a chave vai mascarada à tela e volta da máscara no teste; chave errada
//! vira uma mensagem que diz o que conferir; e a nuvem não oferece download.

use axum::{
    extract::Path,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use backend::{
    app::App,
    dtos::ai::{LayaModelsResponse, TestLayaResponse},
    services::ai::laya::{
        config::AiLayaSettings,
        providers::{DecisionProtocol, LayaProvider},
    },
};
use loco_rs::testing::prelude::*;
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

const KEY: &str = "chave-da-nuvem";

fn authorized(headers: &HeaderMap) -> bool {
    headers.get("authorization").and_then(|v| v.to_str().ok()) == Some(&format!("Bearer {KEY}"))
}

fn denied() -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": { "message": "Incorrect API key", "code": "invalid_api_key" } })),
    )
}

/// Toda pergunta sim/não: "docker" é provável, o resto não.
fn yes(id: &str) -> f64 {
    if id == "docker" {
        0.9
    } else {
        0.1
    }
}

/// Respostas no formato System One para as perguntas recebidas.
fn systemone_answers(questions: &Value) -> Value {
    let answers: serde_json::Map<String, Value> = questions
        .as_object()
        .unwrap()
        .keys()
        .map(|id| (id.clone(), json!({ "type": "noul", "noul": yes(id) })))
        .collect();
    json!({ "model": "jev-1.13.0", "answers": answers, "usage": { "input_tokens": 10 } })
}

async fn start() -> String {
    async fn systemone(headers: HeaderMap, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return denied();
        }
        (StatusCode::OK, Json(systemone_answers(&body["questions"])))
    }

    async fn cloudflare(
        Path((account, model)): Path<(String, String)>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return denied();
        }
        assert_eq!(account, "conta-1");
        assert_eq!(body["model"], model.as_str());
        (
            StatusCode::OK,
            Json(json!({
                "result": systemone_answers(&body["questions"]),
                "success": true, "errors": [], "messages": []
            })),
        )
    }

    async fn decisions(headers: HeaderMap, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
        if !authorized(&headers) {
            return denied();
        }
        assert!(body["input"].is_string(), "OpenAI recebe `input`");
        let answers: Vec<Value> = body["questions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|question| {
                let name = question["name"].as_str().unwrap();
                json!({ "type": "predicate", "name": name, "probability": yes(name) })
            })
            .collect();
        (StatusCode::OK, Json(json!({ "answers": answers })))
    }

    let app = Router::new()
        .route("/v1/systemone", post(systemone))
        .route(
            "/accounts/{account}/ai/run/@cf/cloudflare/{model}",
            post(cloudflare),
        )
        .route("/v1/decisions", post(decisions));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

fn laya(provider: LayaProvider, base_url: &str, protocol: DecisionProtocol) -> AiLayaSettings {
    AiLayaSettings {
        enabled: true,
        provider,
        custom_protocol: protocol,
        base_url: base_url.to_string(),
        model: String::new(),
        api_key: Some(KEY.into()),
        account_id: Some("conta-1".into()),
        timeout_ms: 5_000,
        ..AiLayaSettings::default()
    }
}

async fn test_with(
    request: &loco_rs::TestServer,
    token: &str,
    laya: &AiLayaSettings,
) -> TestLayaResponse {
    let (h, v) = prepare_data::auth_header(token);
    let response = request
        .post("/api/ai/laya/test")
        .add_header(h, v)
        .json(&json!({ "laya": laya, "question": "o container do banco caiu" }))
        .await;
    response.assert_status_ok();
    response.json()
}

#[tokio::test]
#[serial]
async fn teste_da_tela_funciona_nos_tres_formatos_de_nuvem() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let base_url = start().await;
        let sessao = prepare_data::init_user_login(&request, &ctx).await;

        for (nome, settings) in [
            (
                "System One",
                laya(LayaProvider::Custom, &base_url, DecisionProtocol::Systemone),
            ),
            (
                "Cloudflare",
                laya(
                    LayaProvider::Cloudflare,
                    &base_url,
                    DecisionProtocol::Systemone,
                ),
            ),
            (
                "OpenAI",
                laya(LayaProvider::Custom, &base_url, DecisionProtocol::Openai),
            ),
        ] {
            let result = test_with(&request, &sessao.token, &settings).await;
            assert!(result.success, "{nome}: {}", result.message);
            let docker = result
                .groups
                .iter()
                .find(|group| group.id == "docker")
                .unwrap();
            assert!(docker.selected, "{nome}: docker acima do limiar");
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn chave_vai_mascarada_a_tela_e_volta_da_mascara_no_teste() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let base_url = start().await;
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);

        request
            .put("/api/ai/laya")
            .add_header(h.clone(), v.clone())
            .json(&laya(
                LayaProvider::Custom,
                &base_url,
                DecisionProtocol::Systemone,
            ))
            .await
            .assert_status_ok();
        let tela: AiLayaSettings = request
            .get("/api/ai/laya")
            .add_header(h.clone(), v.clone())
            .await
            .json();
        assert_eq!(tela.api_key.as_deref(), Some("********"));

        // A tela devolve o que recebeu, máscara incluída: o teste usa a gravada.
        let result = test_with(&request, &sessao.token, &tela).await;
        assert!(result.success, "{}", result.message);

        let errada = AiLayaSettings {
            api_key: Some("outra".into()),
            ..tela
        };
        let result = test_with(&request, &sessao.token, &errada).await;
        assert!(!result.success);
        assert!(result.message.contains("401"), "{}", result.message);
        assert!(
            result.message.contains("confira a chave"),
            "{}",
            result.message
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn nuvem_lista_os_modelos_do_preset_sem_download() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let sessao = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&sessao.token);
        let typesafe = AiLayaSettings {
            provider: LayaProvider::Typesafe,
            base_url: String::new(),
            model: String::new(),
            ..AiLayaSettings::default()
        };
        let models: LayaModelsResponse = request
            .post("/api/ai/laya/models")
            .add_header(h.clone(), v.clone())
            .json(&json!({ "laya": typesafe }))
            .await
            .json();
        assert!(models.online && !models.manages_models);
        assert_eq!(models.options[0].name, "jev-latest");

        let download = request
            .post("/api/ai/laya/pull")
            .add_header(h.clone(), v.clone())
            .json(&json!({ "laya": typesafe }))
            .await;
        assert_eq!(download.status_code(), 422, "nuvem não baixa modelo");

        let providers: Value = request
            .get("/api/ai/laya/providers")
            .add_header(h, v)
            .await
            .json();
        let ids: Vec<&str> = providers
            .as_array()
            .unwrap()
            .iter()
            .map(|provider| provider["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            [
                "ollaya",
                "typesafe",
                "openrouter",
                "cloudflare",
                "openai",
                "custom"
            ]
        );
    })
    .await;
}
