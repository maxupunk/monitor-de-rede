//! Endpoint MCP Streamable HTTP em `/mcp` (JSON-RPC 2.0, ADR 024).
//!
//! Montado fora de `business_auth` (ao lado de `/v2`) para não cair no fallback
//! da SPA nem exigir sessão de navegador. Autentica exclusivamente via token
//! pessoal `nmm_*` ([`crate::auth`]) e **nunca** consulta `AiSettings.enabled`.

use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use loco_rs::app::AppContext;
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use netmonitor_core::{
    ai::{
        args::{McpToolContext, ToolArgs},
        tools::{all_handlers, AiToolHandler, ToolKind},
    },
    areas::{self, AreaSet},
    audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType, VIA_MCP},
    shared::errors::AppError,
};

use crate::{
    auth::{self, AuthenticatedMcp},
    scopes::{self, McpScope},
};

/// Versões do protocolo aceitas, da mais nova para a mais antiga. A
/// `2025-03-26` exige lote JSON-RPC: [`post_mcp`] aceita.
const SUPPORTED_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const MCP_SESSION_ID_HEADER: &str = "mcp-session-id";
const MCP_PROTOCOL_VERSION_HEADER: &str = "mcp-protocol-version";

const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INVALID_PARAMS: i64 = -32602;
const JSONRPC_FORBIDDEN: i64 = -32003;

const SERVER_INSTRUCTIONS: &str = "Servidor MCP do NetMonitor. Use tools/list para ver as ferramentas autorizadas para o seu token. Para deploy contínuo, consulte get_deploys e get_deploy_readiness antes de executar deploy_action.";

#[derive(Debug, Serialize)]
struct JsonRpcSuccess {
    jsonrpc: &'static str,
    id: Value,
    result: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcErrorBody {
    code: i64,
    message: String,
}

#[derive(Debug, Serialize)]
struct JsonRpcFailure {
    jsonrpc: &'static str,
    id: Value,
    error: JsonRpcErrorBody,
}

fn rpc_ok(id: Value, result: Value) -> Value {
    serde_json::to_value(JsonRpcSuccess {
        jsonrpc: "2.0",
        id,
        result,
    })
    .unwrap_or(Value::Null)
}

fn rpc_err(id: Value, code: i64, message: impl Into<String>) -> Value {
    serde_json::to_value(JsonRpcFailure {
        jsonrpc: "2.0",
        id,
        error: JsonRpcErrorBody {
            code,
            message: message.into(),
        },
    })
    .unwrap_or(Value::Null)
}

/// A versão pedida, se aceita; senão a mais nova (o cliente decide se segue).
fn negotiate_protocol_version(params: &Value) -> &'static str {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .map(str::trim);
    SUPPORTED_PROTOCOLS
        .into_iter()
        .find(|version| Some(*version) == requested)
        .unwrap_or(SUPPORTED_PROTOCOLS[0])
}

fn tool_descriptor(handler: &dyn AiToolHandler) -> Value {
    let kind = handler.kind();
    let read_only = kind == ToolKind::Passive;
    let destructive = matches!(kind, ToolKind::Action | ToolKind::ContainerAction);
    let idempotent = kind == ToolKind::Passive;
    let open_world = matches!(kind, ToolKind::Active) || handler.name() == "web_search";

    json!({
        "name": handler.name(),
        "description": handler.description(),
        "inputSchema": handler.parameters(),
        "annotations": {
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "idempotentHint": idempotent,
            "openWorldHint": open_world,
        }
    })
}

fn handle_initialize(id: Value, params: &Value) -> Value {
    let version = negotiate_protocol_version(params);
    rpc_ok(
        id,
        json!({
            "protocolVersion": version,
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "netmonitor",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": SERVER_INSTRUCTIONS,
        }),
    )
}

async fn handle_tools_list(ctx: &AppContext, principal: &AuthenticatedMcp, id: Value) -> Value {
    let enabled_areas = match areas::load(&ctx.db).await {
        Ok(enabled) => AreaSet::from_enabled(&enabled),
        Err(error) => {
            tracing::error!(%error, "falha ao carregar áreas ligadas no MCP tools/list");
            return rpc_err(id, -32603, "Falha interna ao carregar catálogo MCP.");
        }
    };

    let tools: Vec<Value> = all_handlers()
        .into_iter()
        .filter(|handler| {
            scopes::authorize_listing(
                handler.as_ref(),
                principal.user.active,
                &principal.access,
                enabled_areas,
                &principal.scopes,
            )
        })
        .map(|handler| tool_descriptor(handler.as_ref()))
        .collect();

    rpc_ok(id, json!({ "tools": tools }))
}

async fn handle_tools_call(
    ctx: &AppContext,
    headers: &HeaderMap,
    principal: &AuthenticatedMcp,
    id: Value,
    params: &Value,
) -> Value {
    let Some(name) = params
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return rpc_err(
            id,
            JSONRPC_INVALID_PARAMS,
            "Informe 'name' com o nome da ferramenta MCP.",
        );
    };

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return rpc_err(
            id,
            JSONRPC_INVALID_PARAMS,
            "O campo 'arguments' precisa ser um objeto JSON.",
        );
    }

    let enabled_areas = match areas::load(&ctx.db).await {
        Ok(enabled) => AreaSet::from_enabled(&enabled),
        Err(error) => {
            tracing::error!(%error, "falha ao carregar áreas ligadas no MCP tools/call");
            return rpc_err(id, -32603, "Falha interna ao validar áreas.");
        }
    };

    let handlers = all_handlers();
    let Some(handler) = handlers.iter().find(|h| h.name() == name) else {
        return rpc_err(
            id,
            JSONRPC_INVALID_PARAMS,
            format!("Ferramenta MCP desconhecida: '{name}'."),
        );
    };

    let actor = AuditActor::from_user_id(principal.user.id, headers);
    let mcp_ctx = McpToolContext {
        token_id: principal.token.id,
        token_name: principal.token.name.clone(),
        allowed_application_ids: principal.allowed_application_ids.clone(),
        respect_target_approval: true,
        can_approve_deploys: principal.scopes.contains(&McpScope::DeploysApprove),
    };
    let tool_args = ToolArgs::from_value(arguments)
        .with_actor(actor.clone())
        .with_mcp(mcp_ctx);

    if let Err(error) = scopes::authorize_call(
        handler.as_ref(),
        &tool_args,
        principal.user.active,
        &principal.access,
        enabled_areas,
        &principal.scopes,
    ) {
        return rpc_err(id, JSONRPC_FORBIDDEN, error.to_string());
    }

    let execution = handler.execute(ctx, &tool_args).await;

    let (text, is_error, audit_suffix) = match execution {
        Ok(output) => {
            let is_business_error = output.data.as_object().is_some_and(|map| {
                map.len() == 1 && map.get("error").and_then(Value::as_str).is_some()
            });
            let serialized =
                serde_json::to_string(&output.data).unwrap_or_else(|_| "{}".to_owned());
            (
                serialized,
                is_business_error,
                if is_business_error { "erro" } else { "ok" },
            )
        }
        Err(error) => {
            let message = match &error {
                AppError::Internal(_) => {
                    tracing::error!(%error, tool = name, "falha interna ao executar ferramenta MCP");
                    "Erro interno ao executar a ferramenta.".to_owned()
                }
                other => other.to_string(),
            };
            (message, true, "falha")
        }
    };

    let _ = AuditService::new(&ctx.db)
        .log(
            actor,
            AuditEntryInput {
                action: AuditAction::Execute,
                resource_type: ResourceType::McpTool,
                resource_id: Some(principal.token.id),
                resource_label: Some(handler.name().to_owned()),
                description: Some(format!(
                    "Ferramenta '{}' executada ({audit_suffix}) ({VIA_MCP}: {})",
                    handler.name(),
                    principal.token.name
                )),
                changes: None,
            },
        )
        .await;

    rpc_ok(
        id,
        json!({
            "content": [
                {
                    "type": "text",
                    "text": text,
                }
            ],
            "isError": is_error,
        }),
    )
}

/// Uma mensagem JSON-RPC: a resposta, ou `None` para notificação e para
/// resposta do cliente (que não se respondem).
async fn dispatch(
    ctx: &AppContext,
    headers: &HeaderMap,
    principal: &AuthenticatedMcp,
    message: &Value,
    in_batch: bool,
) -> Option<Value> {
    let Some(object) = message.as_object() else {
        return Some(rpc_err(
            Value::Null,
            JSONRPC_INVALID_REQUEST,
            "Mensagem JSON-RPC precisa ser um objeto.",
        ));
    };
    let id = object.get("id").cloned();
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        // Resposta do cliente (a um pedido do servidor): nada a responder.
        if object.contains_key("result") || object.contains_key("error") {
            return None;
        }
        return Some(rpc_err(
            id.unwrap_or(Value::Null),
            JSONRPC_INVALID_REQUEST,
            "Informe 'method' na mensagem JSON-RPC.",
        ));
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Some(rpc_err(
            id.unwrap_or(Value::Null),
            JSONRPC_INVALID_REQUEST,
            "Campo 'jsonrpc' deve ser \"2.0\".",
        ));
    }
    // Notificações (sem `id`) não têm resposta.
    let id = id?;
    if method.starts_with("notifications/") {
        return None;
    }
    let params = object.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" if in_batch => rpc_err(
            id,
            JSONRPC_INVALID_REQUEST,
            "'initialize' não pode vir num lote JSON-RPC.",
        ),
        "initialize" => handle_initialize(id, &params),
        "ping" => rpc_ok(id, json!({})),
        "tools/list" => handle_tools_list(ctx, principal, id).await,
        "tools/call" => handle_tools_call(ctx, headers, principal, id, &params).await,
        method => rpc_err(
            id,
            JSONRPC_METHOD_NOT_FOUND,
            format!("Método MCP não suportado: '{method}'."),
        ),
    })
}

/// `MCP-Protocol-Version` ausente vale (clientes de 2025-03-26 não mandam);
/// presente, precisa ser uma versão aceita.
fn unsupported_protocol_header(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(MCP_PROTOCOL_VERSION_HEADER)?;
    let version = value.to_str().unwrap_or_default().trim();
    (!SUPPORTED_PROTOCOLS.contains(&version)).then(|| version.to_owned())
}

fn is_initialize(message: &Value) -> bool {
    message.get("method").and_then(Value::as_str) == Some("initialize")
}

async fn post_mcp(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let principal = match auth::authenticate_request(&ctx, &headers).await {
        Ok(principal) => principal,
        Err(error) => {
            let mut response = error.into_response();
            if response.status() == StatusCode::UNAUTHORIZED {
                response.headers_mut().insert(
                    header::WWW_AUTHENTICATE,
                    HeaderValue::from_static("Bearer realm=\"netmonitor-mcp\""),
                );
            }
            return response;
        }
    };

    if let Some(version) = unsupported_protocol_header(&headers) {
        return (
            StatusCode::BAD_REQUEST,
            Json(rpc_err(
                Value::Null,
                JSONRPC_INVALID_REQUEST,
                format!(
                    "Versão do protocolo MCP não suportada: '{version}'. Aceitas: {}.",
                    SUPPORTED_PROTOCOLS.join(", ")
                ),
            )),
        )
            .into_response();
    }

    let Ok(payload) = serde_json::from_slice::<Value>(&body) else {
        return Json(rpc_err(
            Value::Null,
            JSONRPC_PARSE_ERROR,
            "Corpo JSON-RPC 2.0 inválido.",
        ))
        .into_response();
    };

    if let Value::Array(batch) = &payload {
        if batch.is_empty() {
            return Json(rpc_err(
                Value::Null,
                JSONRPC_INVALID_REQUEST,
                "Lote JSON-RPC vazio.",
            ))
            .into_response();
        }
        let mut replies = Vec::with_capacity(batch.len());
        for message in batch {
            if let Some(reply) = dispatch(&ctx, &headers, &principal, message, true).await {
                replies.push(reply);
            }
        }
        return if replies.is_empty() {
            StatusCode::ACCEPTED.into_response()
        } else {
            Json(Value::Array(replies)).into_response()
        };
    }

    let Some(reply) = dispatch(&ctx, &headers, &principal, &payload, false).await else {
        return StatusCode::ACCEPTED.into_response();
    };
    let mut response = Json(reply).into_response();
    if is_initialize(&payload) {
        if let Ok(session_id) = HeaderValue::from_str(&Uuid::new_v4().to_string()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(MCP_SESSION_ID_HEADER), session_id);
        }
    }
    response
}

async fn get_mcp() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, HeaderValue::from_static("POST, DELETE"))],
    )
        .into_response()
}

async fn delete_mcp() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Rotas raiz do servidor MCP (`/mcp`), montadas fora de `/api` e do guarda JWT.
pub fn routes() -> Router<AppContext> {
    Router::new().route("/mcp", post(post_mcp).get(get_mcp).delete(delete_mcp))
}
