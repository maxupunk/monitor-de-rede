//! Testes de integração do servidor MCP Streamable HTTP e da API `/api/mcp/*` (ADR 024).

use backend::app::App;
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use netmonitor_core::{
    audit::{AuditAction, ResourceType, VIA_MCP},
    events::EventBus,
    models::{
        audit_logs, deploy_applications, deploy_runs, deploy_targets, mcp_tokens, probes,
        system_settings, users,
    },
};
use netmonitor_nucleo::services::users::Role;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

const INTERNAL: &str = "10.8.0.5";
const PUBLIC: &str = "203.0.113.9";
const COMMIT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn body(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

async fn make_user_with_jwt(
    ctx: &loco_rs::app::AppContext,
    name: &str,
    email: &str,
    role: Role,
    area_roles: Option<Value>,
) -> (users::Model, String) {
    let user = prepare_data::create_user(ctx, name, email, "Secret-1234").await;
    let mut active = user.into_active_model();
    active.role = Set(role.as_str().to_owned());
    active.area_roles = Set(area_roles);
    let user = active.update(&ctx.db).await.expect("atualizar papel");
    let jwt = ctx.config.get_jwt_config().expect("jwt config");
    let token = user
        .generate_jwt(&jwt.secret, jwt.expiration)
        .expect("gerar jwt");
    (user, format!("Bearer {token}"))
}

async fn insert_deploy_fixture(
    ctx: &loco_rs::app::AppContext,
    name: &str,
    slug: &str,
    requires_approval: bool,
) -> (deploy_applications::Model, deploy_targets::Model) {
    let agent = probes::ActiveModel {
        name: Set(format!("agent-{slug}")),
        token_hash: Set(format!("hash-{slug}")),
        status: Set("offline".into()),
        configuration: Set(Some(json!({ "role": "agent" }))),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("agente");

    let app = deploy_applications::ActiveModel {
        name: Set(name.into()),
        slug: Set(slug.into()),
        repository: Set(format!("acme/{slug}")),
        compose_files: Set(json!(["docker-compose.yml"])),
        image_tag_variable: Set("IMAGE_TAG".into()),
        images: Set(json!([])),
        rollout: Set("parallel".into()),
        builder: Set("netmonitor".into()),
        build_args: Set(json!([])),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("aplicação");

    let target = deploy_targets::ActiveModel {
        application_id: Set(app.id),
        name: Set(format!("Prod {name}")),
        probe_id: Set(Some(agent.id)),
        project: Set(format!("{slug}-prod")),
        reference: Set("main".into()),
        requires_approval: Set(requires_approval),
        wait_timeout_secs: Set(180),
        auto_rollback: Set(true),
        keep_images: Set(3),
        enabled: Set(true),
        position: Set(0),
        vulnerability_policy: Set("warn".into()),
        require_provenance: Set(false),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("alvo");

    (app, target)
}

#[tokio::test]
#[serial]
async fn gestao_rest_crud_escalacao_sse_e_auditoria() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let (_admin, admin_auth) =
            make_user_with_jwt(&ctx, "Admin", "admin-mcp@test.local", Role::Admin, None).await;
        let (_op, op_auth) = make_user_with_jwt(
            &ctx,
            "Operador",
            "op-mcp@test.local",
            Role::Operator,
            Some(json!({ "servers": "operator" })),
        )
        .await;
        let (_viewer, viewer_auth) = make_user_with_jwt(
            &ctx,
            "Leitor",
            "viewer-mcp@test.local",
            Role::Viewer,
            Some(json!({ "servers": "viewer" })),
        )
        .await;

        let (app1, _t1) = insert_deploy_fixture(&ctx, "SGC", "sgc", true).await;

        let bus = EventBus::from_context(&ctx).expect("event bus");
        let mut rx = bus.subscribe();

        // Overview para admin inclui apps de deploy, 7 escopos e canManageSettings=true
        let overview = request
            .get("/api/mcp/overview")
            .add_header("Authorization", admin_auth.clone())
            .await;
        assert_eq!(overview.status_code(), 200, "{}", overview.text());
        let ov = body(&overview.text());
        assert_eq!(ov["settings"]["enabled"], true);
        assert_eq!(ov["settings"]["allowPublicOrigin"], false);
        assert_eq!(ov["settings"]["endpointPath"], "/mcp");
        assert_eq!(ov["canManageSettings"], true);
        assert_eq!(ov["scopes"].as_array().unwrap().len(), 7);
        assert_eq!(ov["deployApplications"].as_array().unwrap().len(), 1);
        assert_eq!(ov["deployApplications"][0]["id"], app1.id);
        assert!(ov["toolsCount"].as_u64().unwrap() > 10);

        // Overview para viewer tem canManageSettings=false e deploys:write bloqueado
        let viewer_ov = request
            .get("/api/mcp/overview")
            .add_header("Authorization", viewer_auth.clone())
            .await;
        assert_eq!(viewer_ov.status_code(), 200, "{}", viewer_ov.text());
        let vov = body(&viewer_ov.text());
        assert_eq!(vov["canManageSettings"], false);
        let deploys_write = vov["scopes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == "deploys:write")
            .unwrap();
        assert_eq!(deploys_write["grantedByRole"], false);
        assert_eq!(deploys_write["requiresAdmin"], true);

        // PUT /api/mcp/settings bloqueado para operator e viewer
        for auth in [&op_auth, &viewer_auth] {
            let denied = request
                .put("/api/mcp/settings")
                .add_header("Authorization", auth.clone())
                .json(&json!({ "enabled": false, "allowPublicOrigin": true }))
                .await;
            assert_eq!(denied.status_code(), 403);
        }

        // PUT /api/mcp/settings funciona para admin e publica SSE mcp:changed
        let updated = request
            .put("/api/mcp/settings")
            .add_header("Authorization", admin_auth.clone())
            .json(&json!({ "enabled": true, "allowPublicOrigin": true }))
            .await;
        assert_eq!(updated.status_code(), 200, "{}", updated.text());
        assert_eq!(body(&updated.text())["allowPublicOrigin"], true);
        let sse_event = rx.try_recv().expect("evento SSE mcp:changed");
        assert_eq!(sse_event.event_type, "mcp:changed");

        // Recusa escalação de privilégio: viewer não cria operations:write nem deploys:write
        for bad_scope in ["operations:write", "deploys:write", "docker:write"] {
            let res = request
                .post("/api/mcp/tokens")
                .add_header("Authorization", viewer_auth.clone())
                .json(&json!({ "name": "escalar", "scopes": [bad_scope], "ttlDays": 30 }))
                .await;
            assert_eq!(res.status_code(), 403, "scope={bad_scope}: {}", res.text());
        }
        // Operator não cria deploys:write nem deploys:approve
        let op_esc = request
            .post("/api/mcp/tokens")
            .add_header("Authorization", op_auth.clone())
            .json(&json!({ "name": "op-esc", "scopes": ["deploys:write"], "ttlDays": 30 }))
            .await;
        assert_eq!(op_esc.status_code(), 403);

        // Viewer cria token somente leitura
        let viewer_tok = request
            .post("/api/mcp/tokens")
            .add_header("Authorization", viewer_auth.clone())
            .json(&json!({ "name": "leitura-viewer", "scopes": ["read"], "ttlDays": 30 }))
            .await;
        assert_eq!(viewer_tok.status_code(), 201, "{}", viewer_tok.text());
        let viewer_created = body(&viewer_tok.text());
        let viewer_secret = viewer_created["secret"].as_str().unwrap().to_owned();
        let viewer_tok_id = viewer_created["token"]["id"].as_i64().unwrap();
        assert!(viewer_secret.starts_with("nmm_"));
        assert_eq!(rx.try_recv().unwrap().event_type, "mcp:changed");

        // Admin cria token de deploy restrito a app1
        let admin_tok = request
            .post("/api/mcp/tokens")
            .add_header("Authorization", admin_auth.clone())
            .json(&json!({
                "name": "ide-sgc",
                "scopes": ["deploys:read", "deploys:write"],
                "allowedApplicationIds": [app1.id],
                "ttlDays": 90
            }))
            .await;
        assert_eq!(admin_tok.status_code(), 201, "{}", admin_tok.text());
        let admin_tok_id = body(&admin_tok.text())["token"]["id"].as_i64().unwrap();
        assert_eq!(rx.try_recv().unwrap().event_type, "mcp:changed");

        // Admin vê ambos os tokens (com owner); viewer vê só o seu; segredo nunca vaza
        let all_tokens = request
            .get("/api/mcp/tokens")
            .add_header("Authorization", admin_auth.clone())
            .await;
        assert_eq!(all_tokens.status_code(), 200);
        assert!(!all_tokens.text().contains(&viewer_secret));
        let all_list = body(&all_tokens.text());
        assert_eq!(all_list.as_array().unwrap().len(), 2);
        assert!(!all_list[0]["owner"].as_str().unwrap().is_empty());

        let viewer_list = request
            .get("/api/mcp/tokens")
            .add_header("Authorization", viewer_auth.clone())
            .await;
        assert_eq!(body(&viewer_list.text()).as_array().unwrap().len(), 1);

        // Viewer não revoga token do admin (404)
        let foreign_del = request
            .delete(&format!("/api/mcp/tokens/{admin_tok_id}"))
            .add_header("Authorization", viewer_auth.clone())
            .await;
        assert_eq!(foreign_del.status_code(), 404);

        // Admin revoga token do viewer (204) e publica SSE
        let revoked = request
            .delete(&format!("/api/mcp/tokens/{viewer_tok_id}"))
            .add_header("Authorization", admin_auth.clone())
            .await;
        assert_eq!(revoked.status_code(), 204);
        assert_eq!(rx.try_recv().unwrap().event_type, "mcp:changed");

        // Auditoria gravou create e delete de McpToken
        let token_audits = audit_logs::Entity::find()
            .filter(audit_logs::Column::ResourceType.eq(ResourceType::McpToken.as_str()))
            .all(&ctx.db)
            .await
            .expect("auditoria");
        assert!(token_audits
            .iter()
            .any(|row| row.action == AuditAction::Create.as_str()));
        assert!(token_audits
            .iter()
            .any(|row| row.action == AuditAction::Delete.as_str()));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn streamable_http_handshake_tools_e_seguranca_sem_ia_local() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        // Desliga explicitamente a IA local para provar que MCP independe dela
        system_settings::Model::set(
            &ctx.db,
            "ai",
            Some(json!({ "enabled": false, "provider": "none" }).to_string()),
        )
        .await
        .expect("desligar ia local");

        let (admin, admin_auth) = make_user_with_jwt(
            &ctx,
            "Admin MCP",
            "admin-proto@test.local",
            Role::Admin,
            None,
        )
        .await;

        let created = request
            .post("/api/mcp/tokens")
            .add_header("Authorization", admin_auth.clone())
            .json(&json!({
                "name": "claude-code",
                "scopes": ["read", "diagnostics", "deploys:read", "operations:write"],
                "ttlDays": 60
            }))
            .await;
        assert_eq!(created.status_code(), 201);
        let secret = body(&created.text())["secret"].as_str().unwrap().to_owned();
        let token_id = body(&created.text())["token"]["id"].as_i64().unwrap();
        let mcp_bearer = format!("Bearer {secret}");

        // GET /mcp -> 405 Method Not Allowed; DELETE /mcp -> 204 No Content
        let get_res = request.get("/mcp").await;
        assert_eq!(get_res.status_code(), 405);
        let del_res = request.delete("/mcp").await;
        assert_eq!(del_res.status_code(), 204);

        // Origem pública bloqueada por padrão (403)
        let pub_blocked = request
            .post("/mcp")
            .add_header("Authorization", mcp_bearer.clone())
            .add_header("x-forwarded-for", PUBLIC)
            .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }))
            .await;
        assert_eq!(pub_blocked.status_code(), 403, "{}", pub_blocked.text());

        // Habilita borda pública e confirma que passa
        let toggle = request
            .put("/api/mcp/settings")
            .add_header("Authorization", admin_auth.clone())
            .json(&json!({ "enabled": true, "allowPublicOrigin": true }))
            .await;
        assert_eq!(toggle.status_code(), 200);

        let pub_ok = request
            .post("/mcp")
            .add_header("Authorization", mcp_bearer.clone())
            .add_header("x-forwarded-for", PUBLIC)
            .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }))
            .await;
        assert_eq!(pub_ok.status_code(), 200, "{}", pub_ok.text());
        assert_eq!(body(&pub_ok.text())["result"], json!({}));

        // Restaura bloqueio de borda pública
        request
            .put("/api/mcp/settings")
            .add_header("Authorization", admin_auth.clone())
            .json(&json!({ "enabled": true, "allowPublicOrigin": false }))
            .await;

        // Handshake initialize (2025-03-26 e 2024-11-05)
        for version in ["2025-03-26", "2024-11-05"] {
            let init = request
                .post("/mcp")
                .add_header("Authorization", mcp_bearer.clone())
                .add_header("x-forwarded-for", INTERNAL)
                .json(&json!({
                    "jsonrpc": "2.0",
                    "id": 10,
                    "method": "initialize",
                    "params": { "protocolVersion": version }
                }))
                .await;
            assert_eq!(init.status_code(), 200, "{}", init.text());
            assert!(
                !init.header("mcp-session-id").to_str().unwrap().is_empty(),
                "Mcp-Session-Id obrigatório"
            );
            let res = body(&init.text());
            assert_eq!(res["result"]["protocolVersion"], version);
            assert_eq!(res["result"]["serverInfo"]["name"], "netmonitor");
            assert_eq!(res["result"]["capabilities"]["tools"]["listChanged"], false);
            assert!(res["result"]["instructions"].as_str().is_some());
        }

        // notifications/initialized -> 202 Accepted
        let notif = request
            .post("/mcp")
            .add_header("Authorization", mcp_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .await;
        assert_eq!(notif.status_code(), 202);

        // tools/list com IA local desligada funciona, inclui annotations e exclui ask_user / DeviceAccess
        let list_res = request
            .post("/mcp")
            .add_header("Authorization", mcp_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({ "jsonrpc": "2.0", "id": 11, "method": "tools/list" }))
            .await;
        assert_eq!(list_res.status_code(), 200, "{}", list_res.text());
        let tools = body(&list_res.text())["result"]["tools"]
            .as_array()
            .expect("lista de tools")
            .clone();
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"get_system_summary"));
        assert!(names.contains(&"get_deploy_readiness"));
        assert!(names.contains(&"ping_host"));
        assert!(names.contains(&"silence_alert"));
        assert!(
            !names.contains(&"ask_user"),
            "Interactive nunca entra no MCP"
        );
        assert!(
            !names.contains(&"device_ssh_exec"),
            "DeviceAccess nunca entra no MCP"
        );
        assert!(!names.contains(&"run_plugin_action"));
        assert!(
            !names.contains(&"docker_container_action"),
            "token não tem docker:write"
        );
        assert!(
            !names.contains(&"deploy_action"),
            "token não tem deploys:write/approve"
        );

        let summary_tool = tools
            .iter()
            .find(|t| t["name"] == "get_system_summary")
            .unwrap();
        assert_eq!(summary_tool["annotations"]["readOnlyHint"], true);
        assert_eq!(summary_tool["annotations"]["destructiveHint"], false);
        assert_eq!(summary_tool["annotations"]["idempotentHint"], true);

        // Tentar chamar ask_user ou device_ssh_exec diretamente é recusado
        for forbidden_tool in ["ask_user", "device_ssh_exec", "docker_container_action"] {
            let call = request
                .post("/mcp")
                .add_header("Authorization", mcp_bearer.clone())
                .add_header("x-forwarded-for", INTERNAL)
                .json(&json!({
                    "jsonrpc": "2.0",
                    "id": 12,
                    "method": "tools/call",
                    "params": { "name": forbidden_tool, "arguments": {} }
                }))
                .await;
            assert_eq!(call.status_code(), 200);
            assert!(
                body(&call.text()).get("error").is_some(),
                "{forbidden_tool} deve ser recusada"
            );
        }

        // tools/call em get_system_summary executa e grava auditoria McpTool com VIA_MCP
        let call_ok = request
            .post("/mcp")
            .add_header("Authorization", mcp_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 13,
                "method": "tools/call",
                "params": { "name": "get_system_summary", "arguments": {} }
            }))
            .await;
        assert_eq!(call_ok.status_code(), 200, "{}", call_ok.text());
        let call_body = body(&call_ok.text());
        assert_eq!(call_body["result"]["isError"], false);
        assert!(!call_body["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .is_empty());

        let tool_audits = audit_logs::Entity::find()
            .filter(audit_logs::Column::ResourceType.eq(ResourceType::McpTool.as_str()))
            .all(&ctx.db)
            .await
            .expect("auditoria mcp_tool");
        assert!(tool_audits.iter().any(|row| {
            row.action == AuditAction::Execute.as_str()
                && row.resource_label.as_deref() == Some("get_system_summary")
                && row
                    .description
                    .as_deref()
                    .is_some_and(|d| d.contains(VIA_MCP))
        }));

        // Desativar servidor MCP devolve 503
        request
            .put("/api/mcp/settings")
            .add_header("Authorization", admin_auth.clone())
            .json(&json!({ "enabled": false, "allowPublicOrigin": false }))
            .await;
        let disabled_res = request
            .post("/mcp")
            .add_header("Authorization", mcp_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({ "jsonrpc": "2.0", "id": 14, "method": "ping" }))
            .await;
        assert_eq!(disabled_res.status_code(), 503);
        request
            .put("/api/mcp/settings")
            .add_header("Authorization", admin_auth.clone())
            .json(&json!({ "enabled": true, "allowPublicOrigin": false }))
            .await;

        // Usuário desativado devolve 401
        let mut active_user = admin.clone().into_active_model();
        active_user.active = Set(false);
        let admin = active_user.update(&ctx.db).await.expect("desativar user");
        let inactive_res = request
            .post("/mcp")
            .add_header("Authorization", mcp_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({ "jsonrpc": "2.0", "id": 15, "method": "ping" }))
            .await;
        assert_eq!(inactive_res.status_code(), 401);

        // Reativa usuário e testa token expirado (401)
        let mut active_user = admin.into_active_model();
        active_user.active = Set(true);
        active_user.update(&ctx.db).await.expect("reativar user");

        let tok_row = mcp_tokens::Entity::find_by_id(token_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let mut tok_active = tok_row.into_active_model();
        tok_active.expires_at = Set((Utc::now() - Duration::hours(1)).into());
        tok_active.update(&ctx.db).await.expect("expirar token");
        let expired_res = request
            .post("/mcp")
            .add_header("Authorization", mcp_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({ "jsonrpc": "2.0", "id": 16, "method": "ping" }))
            .await;
        assert_eq!(expired_res.status_code(), 401);

        // Restaura validade e testa revogação (401)
        let tok_row = mcp_tokens::Entity::find_by_id(token_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let mut tok_active = tok_row.into_active_model();
        tok_active.expires_at = Set((Utc::now() + Duration::days(30)).into());
        tok_active
            .update(&ctx.db)
            .await
            .expect("restaurar validade");
        let rev = request
            .delete(&format!("/api/mcp/tokens/{token_id}"))
            .add_header("Authorization", admin_auth.clone())
            .await;
        assert_eq!(rev.status_code(), 204);
        let revoked_res = request
            .post("/mcp")
            .add_header("Authorization", mcp_bearer)
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({ "jsonrpc": "2.0", "id": 17, "method": "ping" }))
            .await;
        assert_eq!(revoked_res.status_code(), 401);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn json_rpc_lote_resposta_do_cliente_versao_e_origin() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let (_admin, admin_auth) = make_user_with_jwt(
            &ctx,
            "Admin Lote",
            "admin-lote@test.local",
            Role::Admin,
            None,
        )
        .await;
        let created = request
            .post("/api/mcp/tokens")
            .add_header("Authorization", admin_auth)
            .json(&json!({ "name": "lote", "scopes": ["read"], "ttlDays": 30 }))
            .await;
        assert_eq!(created.status_code(), 201);
        let bearer = format!(
            "Bearer {}",
            body(&created.text())["secret"].as_str().expect("segredo")
        );
        let post = |payload: Value| {
            request
                .post("/mcp")
                .add_header("Authorization", bearer.clone())
                .add_header("x-forwarded-for", INTERNAL)
                .json(&payload)
        };

        // Lote (2025-03-26): responde só os pedidos, na ordem
        let batch = post(json!([
            { "jsonrpc": "2.0", "id": 1, "method": "ping" },
            { "jsonrpc": "2.0", "method": "notifications/initialized" },
            { "jsonrpc": "2.0", "id": 2, "method": "nao/existe" },
            { "jsonrpc": "2.0", "id": 3, "method": "initialize", "params": {} }
        ]))
        .await;
        assert_eq!(batch.status_code(), 200, "{}", batch.text());
        let replies = body(&batch.text());
        let replies = replies.as_array().expect("lote de respostas");
        assert_eq!(replies.len(), 3);
        assert_eq!(replies[0]["id"], 1);
        assert_eq!(replies[0]["result"], json!({}));
        assert_eq!(replies[1]["error"]["code"], -32601);
        assert_eq!(
            replies[2]["error"]["code"], -32600,
            "initialize fora de lote"
        );

        // Lote só de notificações e lote vazio
        let only_notifications =
            post(json!([{ "jsonrpc": "2.0", "method": "notifications/initialized" }])).await;
        assert_eq!(only_notifications.status_code(), 202);
        let empty = post(json!([])).await;
        assert_eq!(body(&empty.text())["error"]["code"], -32600);

        // Resposta do cliente (sem `method`) -> 202; pedido sem `method` -> -32600
        let client_reply = post(json!({ "jsonrpc": "2.0", "id": 9, "result": {} })).await;
        assert_eq!(client_reply.status_code(), 202);
        let no_method = post(json!({ "jsonrpc": "2.0", "id": 10 })).await;
        assert_eq!(body(&no_method.text())["error"]["code"], -32600);

        // Versão nova negociada; desconhecida no initialize cai na mais nova
        let init = post(json!({
            "jsonrpc": "2.0", "id": 11, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18" }
        }))
        .await;
        assert_eq!(
            body(&init.text())["result"]["protocolVersion"],
            "2025-06-18"
        );
        let unknown = post(json!({
            "jsonrpc": "2.0", "id": 12, "method": "initialize",
            "params": { "protocolVersion": "1999-01-01" }
        }))
        .await;
        assert_eq!(
            body(&unknown.text())["result"]["protocolVersion"],
            "2025-06-18"
        );

        // Cabeçalho MCP-Protocol-Version: aceita conhecida, recusa desconhecida
        let known_header = request
            .post("/mcp")
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .add_header("mcp-protocol-version", "2025-06-18")
            .json(&json!({ "jsonrpc": "2.0", "id": 13, "method": "ping" }))
            .await;
        assert_eq!(known_header.status_code(), 200);
        let bad_header = request
            .post("/mcp")
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .add_header("mcp-protocol-version", "1999-01-01")
            .json(&json!({ "jsonrpc": "2.0", "id": 14, "method": "ping" }))
            .await;
        assert_eq!(bad_header.status_code(), 400);

        // Origin de outro site (DNS rebinding) -> 403, mesmo com token válido
        let foreign = request
            .post("/mcp")
            .add_header("Authorization", bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .add_header("origin", "http://evil.example")
            .json(&json!({ "jsonrpc": "2.0", "id": 15, "method": "ping" }))
            .await;
        assert_eq!(foreign.status_code(), 403, "{}", foreign.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn autorizacao_ao_vivo_escopos_deploy_e_isolamento_de_aplicacoes() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        let (_admin, admin_auth) = make_user_with_jwt(
            &ctx,
            "Admin Deploy",
            "admin-deploy-mcp@test.local",
            Role::Admin,
            None,
        )
        .await;
        let (operator, op_auth) = make_user_with_jwt(
            &ctx,
            "Op MCP",
            "op-live-mcp@test.local",
            Role::Operator,
            Some(json!({ "servers": "operator" })),
        )
        .await;

        let (app1, target1) = insert_deploy_fixture(&ctx, "SGC", "sgc", true).await;
        let (app2, target2) = insert_deploy_fixture(&ctx, "Outro", "outro", false).await;

        // 1) Operator cria token com read + operations:write; depois é rebaixado para Viewer!
        let op_tok = request
            .post("/api/mcp/tokens")
            .add_header("Authorization", op_auth.clone())
            .json(&json!({
                "name": "op-downgrade",
                "scopes": ["read", "operations:write"],
                "ttlDays": 30
            }))
            .await;
        assert_eq!(op_tok.status_code(), 201);
        let op_secret = body(&op_tok.text())["secret"].as_str().unwrap().to_owned();
        let op_bearer = format!("Bearer {op_secret}");

        // Antes do rebaixamento: operator NÃO vê get_audit_log (admin_only), mas vê silence_alert
        let op_list = request
            .post("/mcp")
            .add_header("Authorization", op_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
            .await;
        let op_names: Vec<String> = body(&op_list.text())["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str().map(str::to_owned))
            .collect();
        assert!(!op_names.contains(&"get_audit_log".to_owned()));
        assert!(op_names.contains(&"silence_alert".to_owned()));

        // Rebaixa usuário para Viewer ao vivo no banco -> silence_alert desaparece e tools/call falha!
        let mut active_op = operator.into_active_model();
        active_op.role = Set("viewer".to_owned());
        active_op.update(&ctx.db).await.expect("rebaixar");

        let downgraded_list = request
            .post("/mcp")
            .add_header("Authorization", op_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }))
            .await;
        let downgraded_names: Vec<String> = body(&downgraded_list.text())["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str().map(str::to_owned))
            .collect();
        assert!(
            !downgraded_names.contains(&"silence_alert".to_owned()),
            "Viewer global não pode gravar no Core"
        );
        let blocked_core_write = request
            .post("/mcp")
            .add_header("Authorization", op_bearer)
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": { "name": "silence_alert", "arguments": { "alertId": 1, "minutes": 10 } }
            }))
            .await;
        assert!(body(&blocked_core_write.text()).get("error").is_some());

        // 2) Token deploys:read + deploys:write restrito exclusivamente a [app1.id]
        let write_tok = request
            .post("/api/mcp/tokens")
            .add_header("Authorization", admin_auth.clone())
            .json(&json!({
                "name": "deploy-sgc",
                "scopes": ["deploys:read", "deploys:write"],
                "allowedApplicationIds": [app1.id],
                "ttlDays": 30
            }))
            .await;
        assert_eq!(write_tok.status_code(), 201);
        let write_bearer = format!(
            "Bearer {}",
            body(&write_tok.text())["secret"].as_str().unwrap()
        );

        // get_deploys vê apenas app1
        let deploys_res = request
            .post("/mcp")
            .add_header("Authorization", write_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": { "name": "get_deploys", "arguments": {} }
            }))
            .await;
        let deploys_payload = body(&deploys_res.text());
        assert_eq!(deploys_payload["result"]["isError"], false);
        let deploys_json: Value = serde_json::from_str(
            deploys_payload["result"]["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(deploys_json["applications"].as_array().unwrap().len(), 1);
        assert_eq!(deploys_json["applications"][0]["applicationId"], app1.id);

        // get_deploy_readiness para app2/target2 retorna erro de acesso negado
        let forbidden_app2 = request
            .post("/mcp")
            .add_header("Authorization", write_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": { "name": "get_deploy_readiness", "arguments": { "targetId": target2.id } }
            }))
            .await;
        assert_eq!(body(&forbidden_app2.text())["result"]["isError"], true);

        // deploy_action(deploy) em target1 (requires_approval=true) cria run em awaiting_approval
        let deploy_call = request
            .post("/mcp")
            .add_header("Authorization", write_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "tools/call",
                "params": {
                    "name": "deploy_action",
                    "arguments": {
                        "action": "deploy",
                        "targetId": target1.id,
                        "commitSha": COMMIT_A
                    }
                }
            }))
            .await;
        let deploy_body = body(&deploy_call.text());
        assert_eq!(deploy_body["result"]["isError"], false, "{deploy_body}");
        let deploy_out: Value = serde_json::from_str(
            deploy_body["result"]["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(deploy_out["status"], "awaiting_approval");
        let run_id = deploy_out["runId"].as_i64().unwrap();

        // Mesmo token (só deploys:write) NÃO pode aprovar ou recusar (precisa deploys:approve!)
        let approve_denied = request
            .post("/mcp")
            .add_header("Authorization", write_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "tools/call",
                "params": { "name": "deploy_action", "arguments": { "action": "approve", "runId": run_id } }
            }))
            .await;
        assert!(
            body(&approve_denied.text()).get("error").is_some(),
            "deploys:write não aprova deploy"
        );

        // 3) Token exclusivamente com deploys:approve aprova/recusa, mas NÃO dispara deploy
        let approve_tok = request
            .post("/api/mcp/tokens")
            .add_header("Authorization", admin_auth)
            .json(&json!({
                "name": "aprovador-sgc",
                "scopes": ["deploys:approve"],
                "allowedApplicationIds": [app1.id],
                "ttlDays": 30
            }))
            .await;
        assert_eq!(approve_tok.status_code(), 201);
        let approve_bearer = format!(
            "Bearer {}",
            body(&approve_tok.text())["secret"].as_str().unwrap()
        );

        let deploy_denied = request
            .post("/mcp")
            .add_header("Authorization", approve_bearer.clone())
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 8,
                "method": "tools/call",
                "params": {
                    "name": "deploy_action",
                    "arguments": { "action": "deploy", "targetId": target1.id, "commitSha": COMMIT_A }
                }
            }))
            .await;
        assert!(
            body(&deploy_denied.text()).get("error").is_some(),
            "deploys:approve não dispara deploy"
        );

        // Recusa o deploy pendente usando deploys:approve
        let reject_ok = request
            .post("/mcp")
            .add_header("Authorization", approve_bearer)
            .add_header("x-forwarded-for", INTERNAL)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 9,
                "method": "tools/call",
                "params": {
                    "name": "deploy_action",
                    "arguments": { "action": "reject", "runId": run_id, "reason": "aguardando janela" }
                }
            }))
            .await;
        let reject_body = body(&reject_ok.text());
        assert_eq!(reject_body["result"]["isError"], false, "{reject_body}");
        let stored_run = deploy_runs::Entity::find_by_id(run_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored_run.status, "rejected");

        let _ = app2;
    })
    .await;
}
