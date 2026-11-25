//! `/api/ai/ollama/models` — listar e apagar os modelos baixados no Ollama.
//!
//! O que se afirma aqui, contra um Ollama falso em `127.0.0.1`: a lista marca
//! o modelo do chat como em uso e traz o digest; qualquer papel lista pelo que
//! está salvo, mas outra URL ou chave na query é do administrador; só o
//! administrador apaga; o
//! modelo do chat é recusado sem chamar o Ollama; o modelo apagado sai pelo
//! `DELETE /api/delete` com o nome exato, devolve o espaço liberado e fica na
//! auditoria; um nome que divide o arquivo com outro não devolve disco —
//! mas, apagado o outro nome, o arquivo que sobra devolve.

use std::sync::{Arc, Mutex};

use axum::{
    routing::{delete, get, post},
    Json, Router,
};
use backend::app::App;
use loco_rs::testing::prelude::*;
use netmonitor_assistant::settings::{self, AiSettings};
use netmonitor_core::{models::audit_logs, roles::Role};
use sea_orm::EntityTrait;
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data::session_with_role;

/// Sobe um Ollama falso com três modelos e anota cada `DELETE /api/delete`.
async fn ollama_falso() -> (String, Arc<Mutex<Vec<String>>>) {
    let apagados = Arc::new(Mutex::new(Vec::<String>::new()));
    let anota = Arc::clone(&apagados);
    let lidos = Arc::clone(&apagados);
    let rotas = Router::new()
        .route(
            "/api/tags",
            get(move || {
                let lidos = Arc::clone(&lidos);
                async move {
                    let fora = lidos.lock().unwrap().clone();
                    let modelos: Vec<Value> = [
                        json!({ "name": "qwen3:8b", "size": 5_000_000_000_u64, "digest": "aaa",
                                "modified_at": "2026-01-10T10:00:00Z" }),
                        json!({ "name": "llama3.2:3b", "size": 2_000_000_000_u64, "digest": "bbb",
                                "modified_at": "2025-06-01T10:00:00Z" }),
                        json!({ "name": "llama3.2:latest", "size": 2_000_000_000_u64,
                                "digest": "bbb" }),
                    ]
                    .into_iter()
                    .filter(|m| !fora.iter().any(|nome| m["name"] == nome.as_str()))
                    .collect();
                    Json(json!({ "models": modelos }))
                }
            }),
        )
        .route("/api/show", post(|| async { Json(json!({})) }))
        .route(
            "/api/delete",
            delete(move |Json(corpo): Json<Value>| {
                let anota = Arc::clone(&anota);
                async move {
                    let nome = corpo["model"].as_str().unwrap_or_default().to_string();
                    anota.lock().unwrap().push(nome);
                    Json(json!({}))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("porta local");
    let endereco = listener.local_addr().expect("endereço");
    tokio::spawn(async move {
        let _ = axum::serve(listener, rotas).await;
    });
    (format!("http://{endereco}/v1"), apagados)
}

async fn configura(ctx: &loco_rs::app::AppContext, base: &str) {
    settings::save(
        &ctx.db,
        AiSettings {
            ollama_base_url: Some(base.to_string()),
            ollama_model: Some("qwen3:8b".into()),
            ..AiSettings::default()
        },
    )
    .await
    .expect("configurações da IA");
}

#[tokio::test]
#[serial]
async fn quem_so_ve_lista_pelo_salvo_mas_nao_aponta_outra_url() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (base, _) = ollama_falso().await;
        configura(&ctx, &base).await;
        session_with_role(&mut request, &ctx, Role::Viewer).await;

        let res = request.get("/api/ai/ollama/models").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());

        let res = request
            .get("/api/ai/ollama/models")
            .add_query_param("base_url", "http://127.0.0.1:9/v1")
            .await;
        assert_eq!(res.status_code(), 403, "{}", res.text());

        let res = request
            .get("/api/ai/openrouter/models")
            .add_query_param("api_key", "sk-digitada")
            .await;
        assert_eq!(res.status_code(), 403, "{}", res.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_lista_marca_o_modelo_do_chat() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (base, _) = ollama_falso().await;
        configura(&ctx, &base).await;
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let res = request.get("/api/ai/ollama/models").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let corpo: Value = res.json();
        let instalados = corpo["installed"].as_array().expect("installed");
        let em_uso: Vec<&str> = instalados
            .iter()
            .filter(|m| m["inUse"] == true)
            .map(|m| m["name"].as_str().unwrap())
            .collect();
        assert_eq!(em_uso, ["qwen3:8b"]);
        assert!(instalados.iter().all(|m| m["digest"].is_string()));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn so_o_administrador_apaga() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (base, apagados) = ollama_falso().await;
        configura(&ctx, &base).await;
        session_with_role(&mut request, &ctx, Role::Operator).await;

        let res = request
            .delete("/api/ai/ollama/models")
            .json(&json!({ "model": "llama3.2:3b" }))
            .await;
        assert_eq!(res.status_code(), 403, "{}", res.text());
        assert!(apagados.lock().unwrap().is_empty());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_modelo_do_chat_e_recusado_sem_chamar_o_ollama() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (base, apagados) = ollama_falso().await;
        configura(&ctx, &base).await;
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let res = request
            .delete("/api/ai/ollama/models")
            .json(&json!({ "model": "qwen3:8b" }))
            .await;
        assert_eq!(res.status_code(), 409, "{}", res.text());
        assert!(res.text().contains("modelo usado pelo Assistente"));
        assert!(apagados.lock().unwrap().is_empty());

        let res = request
            .delete("/api/ai/ollama/models")
            .json(&json!({ "model": "phi4:14b" }))
            .await;
        assert_eq!(res.status_code(), 404, "{}", res.text());
        assert!(apagados.lock().unwrap().is_empty());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn apaga_o_modelo_libera_o_disco_e_audita() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let (base, apagados) = ollama_falso().await;
        configura(&ctx, &base).await;
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let res = request
            .delete("/api/ai/ollama/models")
            .json(&json!({ "model": "llama3.2:latest" }))
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let corpo: Value = res.json();
        assert_eq!(corpo["freedBytes"], 0, "divide o arquivo com llama3.2:3b");
        assert_eq!(corpo["sharedWith"], json!(["llama3.2:3b"]));

        let res = request
            .delete("/api/ai/ollama/models")
            .json(&json!({ "model": "llama3.2:3b", "baseUrl": base }))
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let corpo: Value = res.json();
        assert_eq!(corpo["name"], "llama3.2:3b");
        assert_eq!(corpo["freedBytes"], 2_000_000_000_u64);

        assert_eq!(
            *apagados.lock().unwrap(),
            ["llama3.2:latest", "llama3.2:3b"]
        );

        let auditoria = audit_logs::Entity::find()
            .all(&ctx.db)
            .await
            .expect("auditoria");
        assert!(
            auditoria.iter().any(|linha| linha.action == "delete"
                && linha.resource_label.as_deref() == Some("ollama:llama3.2:3b")),
            "{auditoria:?}"
        );
    })
    .await;
}
