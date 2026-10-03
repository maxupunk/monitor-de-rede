//! Plugins de dispositivo pela API: biblioteca, quarentena, papéis, execução
//! contra um equipamento HTTP local, aprovação a cada acesso e o modo
//! automático da IA.

use std::time::Duration;

use axum::{routing::get, Router};
use backend::{app::App, models::devices, services::users::Role};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

async fn session_with_role(
    request: &mut loco_rs::TestServer,
    ctx: &loco_rs::app::AppContext,
    role: Role,
) {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.as_str().to_string());
    active.update(&ctx.db).await.unwrap();
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

/// Um "roteador" com interface web em 127.0.0.1.
async fn web_device() -> u16 {
    let app = Router::new().route(
        "/",
        get(|| async {
            (
                [("server", "uhttpd")],
                "<html><head><title>Roteador de Teste</title></head></html>",
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    port
}

async fn device(ctx: &loco_rs::app::AppContext) -> devices::Model {
    devices::ActiveModel {
        name: Set("Roteador".into()),
        r#type: Set("router".into()),
        status: Set("online".into()),
        ip_address: Set(Some("127.0.0.1".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

fn package(version: &str) -> Value {
    json!({
        "format": 1,
        "manifest": {
            "slug": "pagina-teste",
            "name": "Página de teste",
            "version": version,
            "transports": ["http"],
            "actions": [ { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" } ]
        },
        "script": "fn detect(device, params) { let p = device.get(\"/\"); #{ status: p.status, firmware: \"1.0\" } }",
        "usage": "## Detectar\nLê a página inicial.",
        "tests": {
            "unit": [ { "action": "detect",
                        "fixtures": [ { "http": "GET /", "status": 200, "body": "ok" } ],
                        "expect": { "status": 200, "firmware": "1.0" } } ],
            "functional": [ { "action": "detect", "expectKeys": ["status"] } ]
        }
    })
}

/// Espera a execução terminar (o disparo pela tela roda em segundo plano).
async fn finished_run(request: &loco_rs::TestServer, run_id: i64) -> Value {
    for _ in 0..100 {
        let run = json_of(
            &request
                .get(&format!("/api/plugin-runs/{run_id}"))
                .await
                .text(),
        );
        if run["status"] != "running" {
            return run;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("a execução {run_id} não terminou em 5 s");
}

#[tokio::test]
#[serial]
async fn biblioteca_ciclo_de_vida_e_quarentena() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let lista = json_of(&request.get("/api/plugins").await.text());
        let slugs: Vec<&str> = lista
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|plugin| plugin["slug"].as_str())
            .collect();
        assert!(slugs.contains(&"openwrt-packages"), "embutidos semeados: {slugs:?}");

        let criado = request
            .post("/api/plugins")
            .json(&json!({ "package": package("1.0.0") }))
            .await;
        criado.assert_status(axum::http::StatusCode::CREATED);
        let criado = json_of(&criado.text());
        let id = criado["summary"]["id"].as_i64().unwrap();
        assert_eq!(criado["summary"]["status"], "draft");

        let testes = json_of(&request.post(&format!("/api/plugins/{id}/test")).await.text());
        assert_eq!(testes["passed"], true, "{testes}");
        let ativo = json_of(&request.post(&format!("/api/plugins/{id}/promote")).await.text());
        assert_eq!(ativo["summary"]["status"], "active");

        let exportado = request.get(&format!("/api/plugins/{id}/export")).await;
        assert!(exportado
            .header("content-disposition")
            .to_str()
            .unwrap()
            .contains("pagina-teste-1.0.0.nmplugin.json"));
        assert_eq!(json_of(&exportado.text())["manifest"]["slug"], "pagina-teste");

        // Pacote de fora com risco crítico: quarentena e instalação bloqueada.
        let mut perigoso = package("2.0.0");
        perigoso["script"] = json!("fn detect(device, params) { device.get(\"/\"); #{ firmware: \"1\" } }\n// sysupgrade -n /tmp/fw.bin\nfn x(device) { device.run(\"sysupgrade -n /tmp/fw.bin\") }");
        let importado = json_of(
            &request
                .post("/api/plugins/import")
                .json(&json!({ "package": perigoso }))
                .await
                .text(),
        );
        assert_eq!(importado["summary"]["status"], "quarantine");
        assert_eq!(importado["review"]["risk"], "critical");
        let importado_id = importado["summary"]["id"].as_i64().unwrap();
        let recusado = request
            .post(&format!("/api/plugins/{importado_id}/accept-review"))
            .json(&json!({ "acknowledgeRisk": true }))
            .await;
        recusado.assert_status_bad_request();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn previa_roda_os_testes_sem_gravar_e_traz_as_sugestoes() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let antes = json_of(&request.get("/api/plugins").await.text())
            .as_array()
            .unwrap()
            .len();

        let mut pacote = package("1.0.0");
        pacote["manifest"]["actions"][0]["title"] = json!("detect_page");
        let previa = request
            .post("/api/plugins/preview")
            .json(&json!({ "package": pacote }))
            .await;
        previa.assert_status_success();
        let previa = json_of(&previa.text());
        assert_eq!(previa["report"]["passed"], true, "{previa}");
        assert_eq!(previa["report"]["cases"][0]["output"]["firmware"], "1.0");
        assert!(previa["list"].is_null());
        assert!(
            previa["report"]["hints"]
                .as_array()
                .unwrap()
                .iter()
                .any(|hint| hint["at"] == "detect"),
            "título técnico vira sugestão: {previa}"
        );

        let depois = json_of(&request.get("/api/plugins").await.text())
            .as_array()
            .unwrap()
            .len();
        assert_eq!(antes, depois, "a prévia não grava plugin");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn operador_consulta_mas_nao_opera_plugin() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        request.get("/api/plugins").await.assert_status_success();
        request
            .post("/api/plugins")
            .json(&json!({ "package": package("1.0.0") }))
            .await
            .assert_status(axum::http::StatusCode::FORBIDDEN);
        request
            .post("/api/plugins/preview")
            .json(&json!({ "package": package("1.0.0") }))
            .await
            .assert_status(axum::http::StatusCode::FORBIDDEN);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn acao_roda_no_equipamento_e_a_senha_nao_volta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let port = web_device().await;
        let device = device(&ctx).await;

        let credencial = request
            .put(&format!("/api/devices/{}/credentials", device.id))
            .json(
                &json!({ "kind": "http", "username": "admin", "secret": "s3nh4-secreta",
                           "storage": "vault", "port": port }),
            )
            .await;
        credencial.assert_status_success();
        assert!(!credencial.text().contains("s3nh4-secreta"));

        let aba = request
            .get(&format!("/api/devices/{}/plugins", device.id))
            .await;
        assert!(!aba.text().contains("s3nh4-secreta"));
        let aba = json_of(&aba.text());
        assert_eq!(aba["credentials"][0]["hasStoredSecret"], true);
        let plugin = aba["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["plugin"]["slug"] == "http-page-info")
            .expect("plugin embutido de página web")
            .clone();
        let plugin_id = plugin["plugin"]["id"].as_i64().unwrap();
        assert_eq!(plugin["installed"], false);

        // Sem instalar, a ação não roda: a aba do plugin nasce na instalação.
        request
            .post(&format!(
                "/api/devices/{}/plugins/{plugin_id}/actions/detect",
                device.id
            ))
            .json(&json!({}))
            .await
            .assert_status_bad_request();
        let instalada = json_of(
            &request
                .post(&format!(
                    "/api/devices/{}/plugins/{plugin_id}/install",
                    device.id
                ))
                .await
                .text(),
        );
        assert!(instalada["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["plugin"]["id"] == plugin_id && item["installed"] == true));

        let disparo = request
            .post(&format!(
                "/api/devices/{}/plugins/{plugin_id}/actions/detect",
                device.id
            ))
            .json(&json!({}))
            .await;
        disparo.assert_status(axum::http::StatusCode::ACCEPTED);
        let run_id = json_of(&disparo.text())["runId"].as_i64().unwrap();
        let run = finished_run(&request, run_id).await;
        assert_eq!(run["status"], "succeeded", "{run}");
        assert_eq!(run["output"]["title"], "Roteador de Teste");
        assert_eq!(run["output"]["server"], "uhttpd");
        assert_eq!(run["transcript"][0]["kind"], "http");

        // Ação de escrita sem confirmação explícita é recusada antes de tudo.
        let opkg = aba["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["plugin"]["slug"] == "openwrt-packages")
            .unwrap()["plugin"]["id"]
            .as_i64()
            .unwrap();
        request
            .post(&format!(
                "/api/devices/{}/plugins/{opkg}/install",
                device.id
            ))
            .await
            .assert_status_success();
        request
            .post(&format!(
                "/api/devices/{}/plugins/{opkg}/actions/install_package",
                device.id
            ))
            .json(&json!({ "params": { "name": "luci" } }))
            .await
            .assert_status_bad_request();

        // Desinstalar tira a aba; o histórico fica.
        let sem = json_of(
            &request
                .delete(&format!(
                    "/api/devices/{}/plugins/{opkg}/install",
                    device.id
                ))
                .await
                .text(),
        );
        assert!(sem["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["plugin"]["id"] != opkg || item["installed"] == false));
        assert!(!sem["runs"].as_array().unwrap().is_empty());

        let capacidades = json_of(
            &request
                .get(&format!("/api/devices/{}/capabilities", device.id))
                .await
                .text(),
        );
        assert_eq!(capacidades["plugins"], true);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn rascunho_pede_aprovacao_a_cada_acesso() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let port = web_device().await;
        let device = device(&ctx).await;
        request
            .put(&format!("/api/devices/{}/credentials", device.id))
            .json(&json!({ "kind": "http", "username": "admin", "storage": "ask", "port": port }))
            .await
            .assert_status_success();
        // "Pedir a cada sessão": sem a senha informada, não roda.
        let plugin_sem_sessao = request
            .post(&format!(
                "/api/devices/{}/plugins/1/actions/detect",
                device.id
            ))
            .json(&json!({}))
            .await;
        assert_ne!(
            plugin_sem_sessao.status_code(),
            axum::http::StatusCode::ACCEPTED
        );
        let sessao = json_of(
            &request
                .post(&format!(
                    "/api/devices/{}/credentials/http/session",
                    device.id
                ))
                .json(&json!({ "secret": "s3nh4" }))
                .await
                .text(),
        );
        assert_eq!(sessao["sessionActive"], true);
        let plugin = json_of(
            &request
                .post("/api/plugins")
                .json(&json!({ "package": package("1.0.0") }))
                .await
                .text(),
        );
        let plugin_id = plugin["summary"]["id"].as_i64().unwrap();
        request
            .post(&format!(
                "/api/devices/{}/plugins/{plugin_id}/install",
                device.id
            ))
            .await
            .assert_status_success();
        let disparar = || async {
            let resposta = request
                .post(&format!(
                    "/api/devices/{}/plugins/{plugin_id}/actions/detect",
                    device.id
                ))
                .json(&json!({}))
                .await;
            json_of(&resposta.text())["runId"].as_i64().unwrap()
        };
        let responder = |run_id: i64, approved: bool| {
            let request = &request;
            async move {
                for _ in 0..100 {
                    let resposta = request
                        .post(&format!("/api/plugin-approvals/{run_id}:1"))
                        .json(&json!({ "approved": approved }))
                        .await;
                    if resposta.status_code() == axum::http::StatusCode::NO_CONTENT {
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                panic!("o pedido de aprovação não apareceu");
            }
        };

        let aprovado = disparar().await;
        responder(aprovado, true).await;
        let run = finished_run(&request, aprovado).await;
        assert_eq!(run["status"], "succeeded", "{run}");

        let negado = disparar().await;
        responder(negado, false).await;
        let run = finished_run(&request, negado).await;
        assert_eq!(run["status"], "failed");
        assert!(run["error"].as_str().unwrap().contains("negado"), "{run}");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn modo_automatico_exige_o_termo_e_vale_por_conversa() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let device = device(&ctx).await;
        let corpo = |aceito: bool| {
            json!({ "conversationKey": "conversa-1", "deviceId": device.id, "acceptTerms": aceito })
        };
        request
            .post("/api/plugin-auto-accept")
            .json(&corpo(false))
            .await
            .assert_status(axum::http::StatusCode::UNPROCESSABLE_ENTITY);

        let ligado = json_of(&request.post("/api/plugin-auto-accept").json(&corpo(true)).await.text());
        assert_eq!(ligado["active"], true);
        assert!(ligado["terms"].as_str().unwrap().contains("NÃO se responsabilizam"));

        let outra = json_of(
            &request
                .get(&format!(
                    "/api/plugin-auto-accept?conversationKey=conversa-2&deviceId={}",
                    device.id
                ))
                .await
                .text(),
        );
        assert_eq!(outra["active"], false);

        let desligado = json_of(
            &request
                .delete("/api/plugin-auto-accept")
                .json(&json!({ "conversationKey": "conversa-1", "deviceId": device.id }))
                .await
                .text(),
        );
        assert_eq!(desligado["active"], false);
    })
    .await;
}
