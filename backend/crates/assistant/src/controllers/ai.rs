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
    central::Central,
    chat_benchmark,
    decider::{
        catalog, client::DeciderClient, config::AiDeciderSettings, decisions_benchmark,
        diagnostics, providers, runtime::DeciderRuntime,
    },
    drivers,
    dtos::{
        AiMentionQuery, ChatStreamRequest, DeciderFormInput, ExecuteToolRequest,
        ExecuteToolResponse, OllamaDeleteRequest, OllamaPullRequest, TestConnectionInput,
        TestConnectionResponse, TestDeciderInput,
    },
    harness::{
        agent::{run_agent_loop, AgentRequest},
        confirmation,
    },
    mentions, model_cleanup, ollama, opencode, openrouter,
    proactive::{digest, incident, runner::ProviderDrivers, schedule::period_hours},
    settings::{self, AiSettings},
};
use netmonitor_core::{
    areas::{self, Area, AreaSet},
    audit::{self, require_admin, AuditAction, AuditActor, ResourceType},
    shared::errors::{AppError, AppResult},
};

const ADMIN_CONFIG_FORBIDDEN: &str =
    "Apenas administradores podem gerenciar configurações, provedores e modelos de IA.";

/// Extrai o ator autenticado dos cabeçalhos HTTP e recusa requisições sem
/// usuário resolvido (fail-closed antes de aplicar `AreaAccess`).
async fn require_authenticated_actor(
    ctx: &AppContext,
    headers: &HeaderMap,
) -> AppResult<AuditActor> {
    let actor = AuditActor::from_headers(headers, &ctx.db).await?;
    if actor.user_id.is_none() {
        return Err(AppError::unauthorized("Não autenticado"));
    }
    Ok(actor)
}

/// `GET /api/ai/settings` — configurações atuais da IA com chaves mascaradas.
async fn show_settings(State(ctx): State<AppContext>) -> AppResult<Response> {
    let s = settings::load(&ctx.db).await?;
    Ok(format::json(s.masked_view())?)
}

/// `PUT /api/ai/settings` — grava novas preferências da IA e devolve o estado resultante mascarado.
async fn update_settings(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(entrada): Json<AiSettings>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
    let saved = settings::save(&ctx.db, entrada).await?;
    Ok(format::json(saved.masked_view())?)
}

/// `POST /api/ai/test-connection` — testa a conectividade e autenticação com o provedor selecionado.
async fn test_connection(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<TestConnectionInput>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
    let existing = settings::load(&ctx.db).await?;
    let driver_name = input.driver.clone();
    let Some(driver) = drivers::from_test_input(&existing, input).await? else {
        return Ok(format::json(TestConnectionResponse {
            success: false,
            latency_ms: 0.0,
            message: format!("Driver '{driver_name}' não suportado"),
            model: None,
        })?);
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

    let actor = require_authenticated_actor(&ctx, &headers).await?;
    let driver = drivers::connect(&settings).await?;
    let request = AgentRequest::from_chat(input, &settings, actor);
    let event_stream = run_agent_loop(ctx, settings, driver, request).await;

    Ok(sse_json_response(event_stream))
}

/// `POST /api/ai/tools/execute` — executa a ação (ou teste ativo) que o
/// usuário confirmou no chat, em nome dele.
async fn execute_tool(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<ExecuteToolRequest>,
) -> AppResult<Response> {
    let settings = settings::load(&ctx.db).await?;
    let actor = require_authenticated_actor(&ctx, &headers).await?;
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
    let actor = require_authenticated_actor(&ctx, &headers).await?;
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
    let actor = require_authenticated_actor(&ctx, &headers).await?;
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
    let enabled_areas = AreaSet::from_enabled(&areas::load(&ctx.db).await?);
    let docker = if enabled_areas.contains(Area::Servers) {
        let central = Central::from_context(&ctx)?;
        central.local_containers(&ctx).await
    } else {
        None
    };
    Ok(format::json(
        mentions::search_with_areas(&ctx.db, docker.as_deref(), &term, enabled_areas).await?,
    )?)
}

/// `POST /api/ai/digest/run` — gera o resumo agora, cobrindo o período da
/// frequência configurada (24 h quando o envio automático está desligado).
/// Não envia notificação: quem pediu está olhando a tela. Só o administrador:
/// o resumo cobre a rede inteira, sem o recorte de área de quem pede.
async fn run_digest(State(ctx): State<AppContext>, headers: HeaderMap) -> AppResult<Response> {
    require_admin(
        &ctx,
        &headers,
        "Apenas administradores podem gerar o resumo da rede manualmente.",
    )
    .await?;
    let settings = settings::load(&ctx.db).await?;
    let hours = period_hours(settings.proactive.digest);
    Ok(format::json(
        digest::generate(&ctx, &settings, &ProviderDrivers, hours).await?,
    )?)
}

/// `POST /api/ai/alerts/{id}/summary` — gera o resumo do incidente agora,
/// quando a triagem do Decisor o pulou ou o teto por hora o adiou. `summary`
/// nulo: o alerta já tinha resumo ou já resolveu. Quem pode escrever (operador
/// ou administrador) pede: a guarda da API já recusa o `POST` de quem só vê.
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
    headers: HeaderMap,
    Query(query): Query<OllamaModelsQuery>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
    let settings = settings::load(&ctx.db).await?;
    let base_url = model_cleanup::resolve_base_url(&settings, query.base_url);
    let models = ollama::list_models(&base_url, model_cleanup::configured_model(&settings)).await?;
    Ok(format::json(models)?)
}

/// `DELETE /api/ai/ollama/models` — apaga um modelo baixado no Ollama para
/// devolver o disco. Só o administrador; o modelo do chat é recusado.
async fn delete_ollama_model(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<OllamaDeleteRequest>,
) -> AppResult<Response> {
    require_admin(
        &ctx,
        &headers,
        "Apenas administradores podem apagar modelos de IA do servidor.",
    )
    .await?;
    let settings = settings::load(&ctx.db).await?;
    let removal = model_cleanup::remove(&settings, input.base_url, &input.model).await?;

    audit::record(
        &ctx.db,
        &headers,
        AuditAction::Delete,
        ResourceType::SystemSetting,
        (0, &format!("ollama:{}", removal.name)),
        format!(
            "Modelo de IA '{}' apagado do Ollama ({} bytes liberados)",
            removal.name, removal.freed_bytes
        ),
    )
    .await;

    Ok(format::json(removal)?)
}

/// `POST /api/ai/ollama/pull` — streaming SSE do download (pull) de modelo no Ollama com porcentagem e status.
async fn pull_ollama_model(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<OllamaPullRequest>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
    let settings = settings::load(&ctx.db).await?;
    let base_url = model_cleanup::resolve_base_url(&settings, input.base_url);

    let progress_stream = ollama::pull_model_stream(&base_url, &input.model).await?;
    Ok(sse_json_response(progress_stream))
}

/// Encapsula qualquer stream serializável em resposta SSE sem buffer de proxy.
fn sse_json_response<T: serde::Serialize>(
    items: impl futures::Stream<Item = T> + Send + 'static,
) -> Response {
    let sse_stream = items.map(|item| {
        let json_str = serde_json::to_string(&item).unwrap_or_else(|_| "{}".into());
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
    headers: HeaderMap,
    Json(decider): Json<AiDeciderSettings>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
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
    headers: HeaderMap,
    Json(input): Json<DeciderFormInput>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
    let decider = settings::resolve_decider(&ctx.db, input.decider).await?;
    Ok(format::json(catalog::list(&decider).await)?)
}

/// `GET /api/ai/decider/benchmark` — os modelos medidos nas decisões do
/// NetMonitor, para a página de benchmark.
async fn show_decider_benchmark() -> AppResult<Response> {
    let report = decisions_benchmark::report()
        .ok_or_else(|| AppError::NotFound("benchmark dos modelos indisponível".into()))?;
    Ok(format::json(report)?)
}

/// `GET /api/ai/chat/benchmark` — os modelos de chat (Ollama local) medidos
/// nos cenários oficiais e cegos de NOC/ISP do NetMonitor.
async fn show_chat_benchmark() -> AppResult<Response> {
    let report = chat_benchmark::report()
        .ok_or_else(|| AppError::NotFound("benchmark dos modelos de chat indisponível".into()))?;
    Ok(format::json(report)?)
}

/// `POST /api/ai/decider/test` — o Ollaya responde, o modelo está lá, e o que ele
/// decide para a pergunta de exemplo.
async fn test_decider(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<TestDeciderInput>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
    let decider = settings::resolve_decider(&ctx.db, input.decider).await?;
    let runtime = DeciderRuntime::from_context(&ctx);
    let result = diagnostics::test(&runtime, &decider, input.question.as_deref()).await;
    Ok(format::json(result)?)
}

/// `POST /api/ai/decider/load` — carrega o modelo na memória em segundo plano.
/// Responde na hora; o progresso chega pelo SSE (`decider:model_state`).
async fn load_decider_model(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<DeciderFormInput>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
    let decider = settings::resolve_decider(&ctx.db, input.decider).await?;
    DeciderRuntime::from_context(&ctx).warm_in_background(&decider);
    Ok(format::json(serde_json::json!({ "started": true }))?)
}

/// `POST /api/ai/decider/pull` — streaming SSE do download do modelo do Decisor.
async fn pull_decider_model(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<DeciderFormInput>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
    let decider = settings::resolve_decider(&ctx.db, input.decider).await?;
    let progress = DeciderClient::connect(&decider).await.pull().await?;
    Ok(sse_json_response(progress))
}

#[derive(Debug, Deserialize)]
struct OpenCodeModelsQuery {
    api_key: Option<String>,
    base_url: Option<String>,
}

/// `GET /api/ai/opencode/models` — lista modelos disponíveis na API do OpenCode Zen.
async fn list_opencode_models(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(query): Query<OpenCodeModelsQuery>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
    let settings = settings::load(&ctx.db).await?;
    let base_url = query
        .base_url
        .filter(|u| !u.trim().is_empty())
        .or_else(|| settings.opencode_base_url.clone())
        .unwrap_or_else(|| settings::OPENCODE_DEFAULT_BASE_URL.to_string());

    let mut api_key = query.api_key;
    if api_key.as_deref() == Some(settings::MASKED_KEY) || api_key.is_none() {
        if settings::opencode_url_matches(Some(&base_url), settings.opencode_base_url.as_deref()) {
            api_key = settings.opencode_api_key;
        } else {
            api_key = None;
        }
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
    headers: HeaderMap,
    Query(query): Query<OpenRouterModelsQuery>,
) -> AppResult<Response> {
    require_admin(&ctx, &headers, ADMIN_CONFIG_FORBIDDEN).await?;
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
        .add(
            "/ollama/models",
            get(list_ollama_models).delete(delete_ollama_model),
        )
        .add("/ollama/pull", post(pull_ollama_model))
        .add("/chat/benchmark", get(show_chat_benchmark))
        .add("/decider", get(show_decider).put(update_decider))
        .add("/decider/providers", get(list_decider_providers))
        .add("/decider/models", post(list_decider_models))
        .add("/decider/benchmark", get(show_decider_benchmark))
        .add("/decider/test", post(test_decider))
        .add("/decider/pull", post(pull_decider_model))
        .add("/decider/load", post(load_decider_model))
}
