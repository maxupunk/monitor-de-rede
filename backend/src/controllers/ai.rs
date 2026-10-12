//! Endpoints do Assistente IA: configurações, teste de conexão e streaming de chat com ferramentas.

use std::convert::Infallible;

use axum::{
    extract::Query,
    http::{header, HeaderMap, HeaderValue},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use futures::StreamExt;
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::{
    dtos::ai::{
        AiMentionQuery, ChatStreamRequest, DeciderFormInput, ExecuteToolRequest,
        ExecuteToolResponse, ModelPullProgress, OllamaPullRequest, TestConnectionInput,
        TestConnectionResponse, TestDeciderInput,
    },
    services::{
        ai::{
            decider::{
                catalog, client::DeciderClient, config::AiDeciderSettings, diagnostics, providers,
                runtime::DeciderRuntime,
            },
            drivers::{create_driver, openai_compatible::OpenAiCompatibleDriver, traits::AiDriver},
            harness::{
                agent::{run_agent_loop, AgentRequest},
                confirmation,
            },
            mentions, ollama, opencode, openrouter,
            proactive::{digest, incident, runner::ProviderDrivers, schedule::period_hours},
            settings::{self, AiSettings},
        },
        audit::AuditActor,
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
    headers: HeaderMap,
    Json(input): Json<ChatStreamRequest>,
) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    if !settings.enabled {
        return Err(AppError::validation(
            "O Assistente IA está desativado nas configurações do sistema.",
        ));
    }

    let driver = create_driver(&settings)?;
    let actor = AuditActor::from_headers(&headers, &ctx.db)
        .await
        .unwrap_or_default();
    let request = AgentRequest::from_chat(input, &settings, actor);
    let event_stream = run_agent_loop(ctx, settings, driver, request).await;

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

/// `POST /api/ai/tools/execute` — executa a ação (ou teste ativo) que o
/// usuário confirmou no chat, em nome dele.
async fn execute_tool(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<ExecuteToolRequest>,
) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    let actor = AuditActor::from_headers(&headers, &ctx.db)
        .await
        .unwrap_or_default();
    let output = confirmation::confirm_in_chat(
        &ctx,
        &settings,
        input.call_id.as_deref(),
        &input.name,
        input.arguments,
        actor,
        input.conversation_key,
    )
    .await?;
    Ok(format::json(ExecuteToolResponse {
        result: output.data,
        chart: output.chart,
    })?)
}

/// `POST /api/ai/approvals/{call_id}/confirm` — confirma um pedido da IA
/// fora do chat (o aviso de outra tela), executando a proposta guardada.
async fn confirm_approval(
    State(ctx): State<AppContext>,
    Path(call_id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    let actor = AuditActor::from_headers(&headers, &ctx.db)
        .await
        .unwrap_or_default();
    let output = confirmation::confirm_pending(&ctx, &settings, &call_id, actor).await?;
    Ok(format::json(ExecuteToolResponse {
        result: output.data,
        chart: output.chart,
    })?)
}

/// `POST /api/ai/approvals/{call_id}/dismiss` — o usuário cancelou o pedido.
async fn dismiss_approval(
    State(ctx): State<AppContext>,
    Path(call_id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let actor = AuditActor::from_headers(&headers, &ctx.db)
        .await
        .unwrap_or_default();
    confirmation::dismiss(&ctx, &actor, &call_id);
    Ok(format::empty()?)
}

/// `GET /api/ai/digest/latest` — último resumo da rede gerado pela IA (ou `null`).
async fn latest_digest(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(digest::latest(&ctx.db).await?)?)
}

/// `GET /api/ai/mentions?q=` — sugestões para o `@` do chat.
async fn search_mentions(
    State(ctx): State<AppContext>,
    Query(query): Query<AiMentionQuery>,
) -> AppResult<Response> {
    let term = query.q.unwrap_or_default();
    Ok(format::json(mentions::search(&ctx.db, &term).await?)?)
}

/// `POST /api/ai/digest/run` — gera o resumo agora, cobrindo o período da
/// frequência configurada (24 h quando o envio automático está desligado).
/// Não envia notificação: quem pediu está olhando a tela.
async fn run_digest(State(ctx): State<AppContext>) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    let hours = period_hours(settings.proactive.digest);
    Ok(format::json(
        digest::generate(&ctx, &settings, &ProviderDrivers, hours).await?,
    )?)
}

/// `POST /api/ai/alerts/{id}/summary` — gera o resumo do incidente agora,
/// quando a triagem do Decisor o pulou ou o teto por hora o adiou. `summary`
/// nulo: o alerta já tinha resumo ou já resolveu.
async fn summarize_alert_now(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let summary = incident::summarize_alert(&ctx, &ProviderDrivers, id).await?;
    Ok(format::json(serde_json::json!({ "summary": summary }))?)
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
    Ok(pull_progress_sse(progress_stream))
}

/// Progresso de um download de modelo como SSE, sem buffer no proxy.
fn pull_progress_sse(
    progress: impl futures::Stream<Item = ModelPullProgress> + Send + 'static,
) -> Response {
    let sse_stream = progress.map(|progress| {
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
    response
}

/// `GET /api/ai/decider` — configuração do Decisor, com a chave mascarada.
async fn show_decider(State(ctx): State<AppContext>) -> AppResult<Response> {
    Ok(format::json(settings::masked_decider(
        settings::load(&ctx.db).await?.decider,
    ))?)
}

/// `GET /api/ai/decider/providers` — os provedores de decisão que a tela oferece.
async fn list_decider_providers() -> AppResult<Response> {
    Ok(format::json(
        providers::PRESETS
            .iter()
            .map(providers::ProviderPreset::view)
            .collect::<Vec<_>>(),
    )?)
}

/// `PUT /api/ai/decider` — grava só o Decisor; o resto da IA fica como está.
async fn update_decider(
    State(ctx): State<AppContext>,
    Json(decider): Json<AiDeciderSettings>,
) -> AppResult<Response> {
    let saved = settings::save_decider(&ctx.db, decider).await?;
    let runtime = DeciderRuntime::from_context(&ctx);
    runtime.invalidate();
    // Ligado agora: carrega já, para a primeira pergunta não pagar o disco.
    if saved.enabled {
        runtime.warm_in_background(&settings::resolve_decider(&ctx.db, saved.clone()).await?);
    }
    Ok(format::json(saved)?)
}

/// `POST /api/ai/decider/models` — os modelos do provedor do formulário: no
/// Ollaya, o catálogo e o que ele já tem instalado; na nuvem, os do preset.
async fn list_decider_models(
    State(ctx): State<AppContext>,
    Json(input): Json<DeciderFormInput>,
) -> AppResult<Response> {
    let decider = settings::resolve_decider(&ctx.db, input.decider).await?;
    Ok(format::json(catalog::list(&decider).await)?)
}

/// `POST /api/ai/decider/test` — o Ollaya responde, o modelo está lá, e o que ele
/// decide para a pergunta de exemplo.
async fn test_decider(
    State(ctx): State<AppContext>,
    Json(input): Json<TestDeciderInput>,
) -> AppResult<Response> {
    let decider = settings::resolve_decider(&ctx.db, input.decider).await?;
    let runtime = DeciderRuntime::from_context(&ctx);
    let result = diagnostics::test(&runtime, &decider, input.question.as_deref()).await;
    Ok(format::json(result)?)
}

/// `POST /api/ai/decider/load` — carrega o modelo na memória em segundo plano.
/// Responde na hora; o progresso chega pelo SSE (`decider:model_state`).
async fn load_decider_model(
    State(ctx): State<AppContext>,
    Json(input): Json<DeciderFormInput>,
) -> AppResult<Response> {
    let decider = settings::resolve_decider(&ctx.db, input.decider).await?;
    DeciderRuntime::from_context(&ctx).warm_in_background(&decider);
    Ok(format::json(serde_json::json!({ "started": true }))?)
}

/// `POST /api/ai/decider/pull` — streaming SSE do download do modelo do Decisor.
async fn pull_decider_model(
    State(ctx): State<AppContext>,
    Json(input): Json<DeciderFormInput>,
) -> AppResult<Response> {
    let decider = settings::resolve_decider(&ctx.db, input.decider).await?;
    let progress = DeciderClient::new(&decider).pull().await?;
    Ok(pull_progress_sse(progress))
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
        .add("/tools/execute", post(execute_tool))
        .add("/approvals/{call_id}/confirm", post(confirm_approval))
        .add("/approvals/{call_id}/dismiss", post(dismiss_approval))
        .add("/digest/latest", get(latest_digest))
        .add("/mentions", get(search_mentions))
        .add("/digest/run", post(run_digest))
        .add("/alerts/{id}/summary", post(summarize_alert_now))
        .add("/opencode/models", get(list_opencode_models))
        .add("/openrouter/models", get(list_openrouter_models))
        .add("/ollama/models", get(list_ollama_models))
        .add("/ollama/pull", post(pull_ollama_model))
        .add("/decider", get(show_decider).put(update_decider))
        .add("/decider/providers", get(list_decider_providers))
        .add("/decider/models", post(list_decider_models))
        .add("/decider/test", post(test_decider))
        .add("/decider/pull", post(pull_decider_model))
        .add("/decider/load", post(load_decider_model))
}
