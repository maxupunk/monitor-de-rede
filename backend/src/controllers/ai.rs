//! Endpoints do Assistente IA: configurações, teste de conexão e streaming de chat com ferramentas.

use std::convert::Infallible;

use axum::{
    extract::Query,
    http::{header, HeaderValue},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use futures::StreamExt;
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::{
    dtos::ai::{ChatStreamRequest, OllamaPullRequest, TestConnectionInput, TestConnectionResponse},
    services::{
        ai::{
            drivers::{create_driver, openai_compatible::OpenAiCompatibleDriver, traits::AiDriver},
            harness::agent::run_agent_loop,
            ollama, opencode, openrouter,
            settings::{self, AiSettings},
        },
        shared::errors::{AppError, AppResult},
    },
};

/// `GET /api/ai/settings` — configurações atuais da IA com chaves mascaradas.
async fn show_settings(State(ctx): State<AppContext>) -> AppResult<Response> {
    let s = settings::load(&ctx.db).await?;
    Ok(format::json(s.masked_view())?)
}

/// `PUT /api/ai/settings` — grava novas preferências da IA e devolve o estado resultante mascarado.
async fn update_settings(
    State(ctx): State<AppContext>,
    Json(entrada): Json<AiSettings>,
) -> AppResult<Response> {
    let saved = settings::save(&ctx.db, entrada).await?;
    Ok(format::json(saved.masked_view())?)
}

/// `POST /api/ai/test-connection` — testa a conectividade e autenticação com o provedor selecionado.
async fn test_connection(
    State(ctx): State<AppContext>,
    Json(input): Json<TestConnectionInput>,
) -> AppResult<Response> {
    let existing = settings::load(&ctx.db).await?;

    let driver: Box<dyn AiDriver> = match input.driver.as_str() {
        "opencode" => {
            let base_url = input
                .base_url
                .filter(|u| !u.trim().is_empty())
                .or(existing.opencode_base_url)
                .unwrap_or_else(|| settings::OPENCODE_DEFAULT_BASE_URL.to_string());
            let mut api_key = input.api_key;
            if api_key.as_deref() == Some(settings::MASKED_KEY) {
                api_key = existing.opencode_api_key;
            }
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| AppError::validation("Informe a API Key do OpenCode Go / Zen"))?;
            let model = if input.model.trim().is_empty() {
                settings::DEFAULT_OPENCODE_MODEL.to_string()
            } else {
                input.model
            };
            Box::new(OpenAiCompatibleDriver::new(
                "opencode",
                "OpenCode Go / Zen",
                base_url,
                Some(key),
                model,
                vec![],
            ))
        }
        "openrouter" => {
            let mut api_key = input.api_key;
            if api_key.as_deref() == Some(settings::MASKED_KEY) {
                api_key = existing.openrouter_api_key;
            }
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| AppError::validation("Informe a API Key do OpenRouter"))?;
            let model = if input.model.trim().is_empty()
                || input.model == "meta-llama/llama-3.3-70b-instruct:free"
            {
                settings::DEFAULT_OPENROUTER_MODEL.to_string()
            } else {
                input.model
            };
            Box::new(OpenAiCompatibleDriver::new(
                "openrouter",
                "OpenRouter",
                "https://openrouter.ai/api/v1".to_string(),
                Some(key),
                model,
                vec![
                    (
                        "HTTP-Referer".to_string(),
                        "https://netmonitor.app".to_string(),
                    ),
                    ("X-Title".to_string(), "NetMonitor AI Assistant".to_string()),
                ],
            ))
        }
        "ollama" => {
            let base_url = input
                .base_url
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string());
            Box::new(OpenAiCompatibleDriver::new(
                "ollama",
                "Ollama Local",
                base_url,
                None,
                input.model,
                vec![],
            ))
        }
        outro => {
            return Ok(format::json(TestConnectionResponse {
                success: false,
                latency_ms: 0.0,
                message: format!("Driver '{outro}' não suportado"),
                model: None,
            })?);
        }
    };

    let result = driver.test_connection().await?;
    Ok(format::json(result)?)
}

/// `POST /api/ai/chat/stream` — endpoint SSE transmitindo deltas de texto e execuções de ferramentas.
async fn chat_stream(
    State(ctx): State<AppContext>,
    Json(input): Json<ChatStreamRequest>,
) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    if !settings.enabled {
        return Err(AppError::validation(
            "O Assistente IA está desativado nas configurações do sistema.",
        ));
    }

    let driver = create_driver(&settings)?;
    let event_stream = run_agent_loop(ctx, settings, driver, input).await;

    let sse_stream = event_stream.map(|event| {
        let json_str = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
        Ok::<Event, Infallible>(Event::default().data(json_str))
    });

    let mut response = Sse::new(sse_stream)
        .keep_alive(
            KeepAlive::new()
                .interval(std::time::Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response();

    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));

    Ok(response)
}

#[derive(Debug, Deserialize)]
struct OllamaModelsQuery {
    base_url: Option<String>,
}

/// `GET /api/ai/ollama/models` — lista modelos locais instalados e recomendados do Ollama.
async fn list_ollama_models(
    State(ctx): State<AppContext>,
    Query(query): Query<OllamaModelsQuery>,
) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    let base_url = query
        .base_url
        .filter(|u| !u.trim().is_empty())
        .or(settings.ollama_base_url)
        .unwrap_or_else(|| "http://localhost:11434".to_string());

    let models = ollama::list_models(&base_url).await?;
    Ok(format::json(models)?)
}

/// `POST /api/ai/ollama/pull` — streaming SSE do download (pull) de modelo no Ollama com porcentagem e status.
async fn pull_ollama_model(
    State(ctx): State<AppContext>,
    Json(input): Json<OllamaPullRequest>,
) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    let base_url = input
        .base_url
        .filter(|u| !u.trim().is_empty())
        .or(settings.ollama_base_url)
        .unwrap_or_else(|| "http://localhost:11434".to_string());

    let progress_stream = ollama::pull_model_stream(&base_url, &input.model).await?;

    let sse_stream = progress_stream.map(|progress| {
        let json_str = serde_json::to_string(&progress).unwrap_or_else(|_| "{}".into());
        Ok::<Event, Infallible>(Event::default().data(json_str))
    });

    let mut response = Sse::new(sse_stream)
        .keep_alive(
            KeepAlive::new()
                .interval(std::time::Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response();

    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));

    Ok(response)
}

#[derive(Debug, Deserialize)]
struct OpenCodeModelsQuery {
    api_key: Option<String>,
    base_url: Option<String>,
}

/// `GET /api/ai/opencode/models` — lista modelos disponíveis na API do OpenCode Zen.
async fn list_opencode_models(
    State(ctx): State<AppContext>,
    Query(query): Query<OpenCodeModelsQuery>,
) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    let base_url = query
        .base_url
        .filter(|u| !u.trim().is_empty())
        .or(settings.opencode_base_url)
        .unwrap_or_else(|| settings::OPENCODE_DEFAULT_BASE_URL.to_string());

    let mut api_key = query.api_key;
    if api_key.as_deref() == Some(settings::MASKED_KEY) || api_key.is_none() {
        api_key = settings.opencode_api_key;
    }

    let result = opencode::list_models(&base_url, api_key.as_deref()).await;
    Ok(format::json(result)?)
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsQuery {
    api_key: Option<String>,
}

/// `GET /api/ai/openrouter/models` — lista modelos disponíveis na API do OpenRouter.
async fn list_openrouter_models(
    State(ctx): State<AppContext>,
    Query(query): Query<OpenRouterModelsQuery>,
) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    let mut api_key = query.api_key;
    if api_key.as_deref() == Some(settings::MASKED_KEY) || api_key.is_none() {
        api_key = settings.openrouter_api_key;
    }

    let result = openrouter::list_models(api_key.as_deref()).await;
    Ok(format::json(result)?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/ai")
        .add("/settings", get(show_settings).put(update_settings))
        .add("/test-connection", post(test_connection))
        .add("/chat/stream", post(chat_stream))
        .add("/opencode/models", get(list_opencode_models))
        .add("/openrouter/models", get(list_openrouter_models))
        .add("/ollama/models", get(list_ollama_models))
        .add("/ollama/pull", post(pull_ollama_model))
}
