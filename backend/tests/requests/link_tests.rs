//! Teste de enlace pela API (ADR 015 §5): plano, execução de ponta a ponta
//! pelo runtime de plugin de verdade (um plugin HTTP cujo script responde o
//! contrato fixo sem tocar no equipamento), filtros do histórico,
//! cancelamento, agendamentos e os perfis de acesso.

use std::time::Duration;

use axum::http::StatusCode;
use backend::{
    app::App,
    models::{device_links, devices, plugins},
    services::{
        plugins::{
            package::PluginPackage,
            service::{self as plugin_service, source},
        },
        users::Role,
    },
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

async fn session_with_role(request: &mut loco_rs::TestServer, ctx: &AppContext, role: Role) {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.as_str().to_string());
    active.update(&ctx.db).await.unwrap();
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

const SCRIPT: &str = r#"
fn detect(device, params) { #{ firmware: "1.0" } }
fn tp_prepare(device, params) { #{ ready: true, installed: ["iperf3"] } }
fn tp_check(device, params) {
    let name = device.info.name;
    if name.contains("sem iperf3") {
        #{ ready: false, missing: ["iperf3"], canInstall: true, message: name + " não tem o iperf3" }
    } else {
        #{ ready: true, missing: [], canInstall: true, message: "" }
    }
}
fn tp_serve(device, params) { #{ port: params.port, handle: "pid:4242" } }
fn tp_stop(device, params) { #{ stopped: true } }
fn tp_run(device, params) {
    if params.direction == "send" {
        #{ engine: "iperf3", txBps: 900000000.0, samples: [] }
    } else {
        #{ engine: "iperf3", rxBps: 400000000.0, samples: [] }
    }
}
"#;

fn manifest() -> Value {
    let text = json!({ "type": "string" });
    let number = json!({ "type": "integer" });
    let secret = json!({ "type": "string", "secret": true });
    json!({
        "slug": "lt-teste", "name": "Teste de enlace", "version": "1.0.0", "transports": ["http"],
        "actions": [
            { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" },
            { "id": "tp_prepare", "title": "Preparar", "effect": "write",
              "params": { "type": "object", "properties": { "engine": text, "role": text } } },
            { "id": "tp_check", "title": "Conferir", "effect": "read",
              "params": { "type": "object", "properties": { "engine": text, "role": text } } },
            { "id": "tp_serve", "title": "Servir", "effect": "write",
              "params": { "type": "object", "properties": {
                  "engine": text, "port": number, "peer": text, "user": text,
                  "password": secret, "requireAuth": { "type": "boolean" } } } },
            { "id": "tp_stop", "title": "Parar", "effect": "write",
              "params": { "type": "object", "properties": { "engine": text, "handle": text } } },
            { "id": "tp_run", "title": "Medir", "effect": "read",
              "params": { "type": "object", "properties": {
                  "engine": text, "target": text, "port": number, "direction": text,
                  "durationS": number, "streams": number, "limitMbps": number,
                  "user": text, "password": secret } } }
        ],
        "throughput": {
            "serve": [{ "engine": "iperf3" }],
            "drive": [{ "engine": "iperf3", "auth": false }],
            "actions": { "prepare": "tp_prepare", "check": "tp_check", "serve": "tp_serve",
                         "stop": "tp_stop", "run": "tp_run" }
        }
    })
}

async fn active_plugin(ctx: &AppContext) -> plugins::Model {
    let package: PluginPackage = serde_json::from_value(json!({
        "manifest": manifest(), "script": SCRIPT, "usage": "## Teste de enlace"
    }))
    .unwrap();
    let created = plugin_service::create(&ctx.db, &package, None, source::USER)
        .await
        .expect("manifesto válido");
    let mut active = created.into_active_model();
    active.status = Set(plugin_service::status::ACTIVE.to_owned());
    active.update(&ctx.db).await.unwrap()
}

async fn device(ctx: &AppContext, name: &str) -> devices::Model {
    devices::ActiveModel {
        name: Set(name.into()),
        r#type: Set("router".into()),
        status: Set("online".into()),
        ip_address: Set(Some("127.0.0.1".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

/// Dois equipamentos com o plugin instalado e um enlace entre eles.
async fn cenario(ctx: &AppContext) -> (i64, i64, i64) {
    let plugin = active_plugin(ctx).await;
    let a = device(ctx, "Roteador A").await;
    let b = devices::ActiveModel {
        name: Set("Roteador B".into()),
        r#type: Set("router".into()),
        status: Set("online".into()),
        ip_address: Set(Some("127.0.0.2".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
    for d in [&a, &b] {
        plugin_service::install(&ctx.db, d, plugin.id, None)
            .await
            .unwrap();
    }
    let now = chrono::Utc::now().into();
    let link = device_links::ActiveModel {
        source_device_id: Set(a.id),
        target_device_id: Set(b.id),
        link_type: Set("ethernet".into()),
        discovery_method: Set("manual".into()),
        confidence: Set(100),
        confirmed: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();
    (a.id, b.id, link.id)
}

/// Espera o teste terminar (ele roda em segundo plano).
async fn finished(request: &loco_rs::TestServer, id: i64) -> Value {
    for _ in 0..200 {
        let test = json_of(&request.get(&format!("/api/link-tests/{id}")).await.text());
        if test["status"] != "queued" && test["status"] != "running" {
            return test;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("o teste {id} não terminou em 10 s");
}

fn pedido(link_id: i64) -> Value {
    json!({ "kind": "link", "linkId": link_id, "spec": { "durationS": 1 } })
}

#[tokio::test]
#[serial]
async fn plano_execucao_e_historico_pela_api() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let (a, _, link_id) = cenario(&ctx).await;

        let plano = request
            .post("/api/link-tests/plan")
            .json(&pedido(link_id))
            .await;
        plano.assert_status_ok();
        let plano = json_of(&plano.text());
        assert_eq!(plano["blocking"], json!([]), "{plano}");
        assert_eq!(plano["legs"][0]["engine"], "iperf3");
        assert_eq!(plano["legs"][0]["serverSide"], "b");
        assert_eq!(plano["legs"][0]["runnable"], true);
        assert!(plano["estimatedSeconds"].as_u64().unwrap() > 0);

        // Sem confirmação explícita, não roda.
        request
            .post("/api/link-tests")
            .json(&pedido(link_id))
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);

        let mut confirmado = pedido(link_id);
        confirmado["confirm"] = json!(true);
        let iniciado = request.post("/api/link-tests").json(&confirmado).await;
        iniciado.assert_status(StatusCode::ACCEPTED);
        let id = json_of(&iniciado.text())["id"].as_i64().unwrap();

        let teste = finished(&request, id).await;
        assert_eq!(teste["status"], "succeeded", "{teste:#}");
        assert_eq!(teste["origin"], "user");
        assert!(teste["requestedBy"]["name"].is_string());
        let perna = &teste["legs"][0];
        assert_eq!(perna["forwardBps"], 9e8);
        assert_eq!(perna["reverseBps"], 4e8);
        assert!(perna["samples"].is_array(), "amostras no detalhe");
        for etapa in perna["stages"].as_array().unwrap() {
            assert_eq!(etapa["status"], "succeeded", "{etapa}");
            assert!(etapa["runId"].is_i64(), "transcript a um clique: {etapa}");
        }
        assert_eq!(teste["summary"]["bottleneckLegSeq"], 1);

        // Histórico com filtros.
        let lista = |query: String| {
            let request = &request;
            async move {
                json_of(
                    &request
                        .get(&format!("/api/link-tests?{query}"))
                        .await
                        .text(),
                )
            }
        };
        let todos = lista(String::new()).await;
        assert_eq!(
            (todos["total"].as_u64(), todos["page"].as_u64()),
            (Some(1), Some(1))
        );
        assert!(todos["items"][0]["legs"][0].get("samples").is_none());
        assert_eq!(lista(format!("deviceId={a}")).await["total"], 1);
        assert_eq!(
            lista(format!("linkId={link_id}&status=succeeded")).await["total"],
            1
        );
        assert_eq!(lista("kind=path".into()).await["total"], 0);
        assert_eq!(lista("kind=link&perPage=5".into()).await["perPage"], 5);
        assert_eq!(lista("since=2999-01-01T00:00:00Z".into()).await["total"], 0);
        assert_eq!(
            lista("since=2000-01-01T00:00:00%2B00:00".into()).await["total"],
            1
        );

        // Terminado não se cancela; inexistente é 404.
        request
            .post(&format!("/api/link-tests/{id}/cancel"))
            .await
            .assert_status(StatusCode::CONFLICT);
        request
            .get("/api/link-tests/999999")
            .await
            .assert_status(StatusCode::NOT_FOUND);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn enlace_atraves_do_switch_sem_ip_pela_api() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        // Roteador A — switch-assistencia (sem IP) — Roteador B.
        let (a, b, direto) = cenario(&ctx).await;
        let switch = devices::ActiveModel {
            name: Set("switch-assistencia".into()),
            r#type: Set("unmanaged_switch".into()),
            status: Set("unknown".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        // Sem o cabo direto: o único caminho passa pelo switch.
        device_links::Entity::delete_by_id(direto)
            .exec(&ctx.db)
            .await
            .unwrap();
        let now = chrono::Utc::now().into();
        let mut cabos = Vec::new();
        for (de, para) in [(a, switch.id), (switch.id, b)] {
            let cabo = device_links::ActiveModel {
                source_device_id: Set(de),
                target_device_id: Set(para),
                link_type: Set("ethernet".into()),
                discovery_method: Set("manual".into()),
                confidence: Set(100),
                confirmed: Set(true),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await
            .unwrap();
            cabos.push(cabo.id);
        }

        let plano = request
            .post("/api/link-tests/plan")
            .json(
                &json!({ "kind": "link", "sourceDeviceId": a, "targetDeviceId": b,
                           "spec": { "durationS": 1 } }),
            )
            .await;
        plano.assert_status_ok();
        let plano = json_of(&plano.text());
        assert_eq!(plano["blocking"], json!([]), "{plano}");
        let perna = &plano["legs"][0];
        assert_eq!(plano["legs"].as_array().unwrap().len(), 1, "{plano}");
        assert_eq!(
            (
                perna["a"]["deviceId"].as_i64(),
                perna["b"]["deviceId"].as_i64()
            ),
            (Some(a), Some(b))
        );
        assert_eq!(perna["edgeIds"], json!(cabos));
        assert_eq!(perna["edgeId"], Value::Null);
        assert_eq!(perna["viaDeviceIds"], json!([switch.id]));
        assert_eq!(perna["viaNames"], json!(["switch-assistencia"]));
        assert_eq!(perna["runnable"], true);

        // O cabo que encosta no switch não é um enlace testável sozinho.
        let recusado = request
            .post("/api/link-tests/plan")
            .json(&pedido(cabos[0]))
            .await;
        recusado.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            recusado.text().contains("switch-assistencia não tem IP"),
            "{}",
            recusado.text()
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn plano_traz_os_passos_que_faltam_e_eles_se_resolvem_pela_api() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let ap = devices::ActiveModel {
            name: Set("AP-terraço".into()),
            r#type: Set("router".into()),
            status: Set("online".into()),
            ip_address: Set(Some("127.0.0.3".into())),
            operating_system: Set(Some("openwrt".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        let pedido =
            json!({ "kind": "server", "targetDeviceId": ap.id, "spec": { "durationS": 1 } });

        let plano = json_of(
            &request
                .post("/api/link-tests/plan")
                .json(&pedido)
                .await
                .text(),
        );
        assert_eq!(plano["blocking"], json!([]), "{plano}");
        assert_eq!(plano["legs"][0]["runnable"], false);
        let fixes = plano["fixes"].as_array().expect("fixes");
        assert_eq!(fixes.len(), 2, "{plano}");
        assert_eq!(fixes[0]["kind"], "install_plugin");
        assert_eq!(fixes[0]["pluginSlug"], "openwrt-link-test");
        assert_eq!(fixes[0]["deviceId"], ap.id);
        assert_eq!(fixes[1]["kind"], "add_credential");
        assert_eq!(fixes[1]["transport"], "ssh");
        assert_eq!(fixes[1]["message"], "AP-terraço está sem acesso SSH");
        assert_eq!(plano["legs"][0]["fixes"], plano["fixes"]);

        // O formulário do acesso lê o que o equipamento já tem.
        let acesso = json_of(
            &request
                .get(&format!("/api/devices/{}/access", ap.id))
                .await
                .text(),
        );
        assert_eq!(acesso["deviceName"], "AP-terraço");
        assert_eq!(acesso["credentials"], json!([]));
        assert!(acesso["agents"].is_array());

        // "Instalar agora" e "Cadastrar acesso": as rotas de sempre.
        let plugin_id = fixes[0]["pluginId"].as_i64().unwrap();
        request
            .post(&format!(
                "/api/devices/{}/plugins/{plugin_id}/install",
                ap.id
            ))
            .json(&json!({}))
            .await
            .assert_status_ok();
        request
            .put(&format!("/api/devices/{}/credentials", ap.id))
            .json(&json!({ "kind": "ssh", "username": "root", "secret": "x", "storage": "vault" }))
            .await
            .assert_status_ok();
        let plano = json_of(
            &request
                .post("/api/link-tests/plan")
                .json(&pedido)
                .await
                .text(),
        );
        assert_eq!(plano["fixes"], json!([]), "{plano}");
        assert_eq!(plano["legs"][0]["runnable"], true, "{plano}");
    })
    .await;
}

/// Espera a execução do plugin terminar (ela roda em segundo plano).
async fn run_finished(request: &loco_rs::TestServer, id: i64) -> Value {
    for _ in 0..200 {
        let run = json_of(&request.get(&format!("/api/plugin-runs/{id}")).await.text());
        if run["status"] != "running" && run["status"] != "queued" {
            return run;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("a execução {id} não terminou em 10 s");
}

#[tokio::test]
#[serial]
async fn pacote_que_falta_vira_passo_aceito_pelo_administrador() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let (_, b, link_id) = cenario(&ctx).await;
        let mut sem = devices::Entity::find_by_id(b)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap()
            .into_active_model();
        sem.name = Set("Roteador B sem iperf3".into());
        sem.update(&ctx.db).await.unwrap();

        // O plano confere (pelo runtime de verdade) e oferece o passo.
        let plano = json_of(
            &request
                .post("/api/link-tests/plan")
                .json(&pedido(link_id))
                .await
                .text(),
        );
        assert_eq!(plano["blocking"], json!([]), "{plano}");
        assert_eq!(plano["legs"][0]["runnable"], false, "{plano}");
        let fix = &plano["fixes"][0];
        assert_eq!(fix["kind"], "install_package", "{plano}");
        assert_eq!(fix["deviceId"], b);
        assert_eq!(fix["action"], "tp_prepare");
        assert_eq!(fix["engine"], "iperf3");
        assert_eq!(fix["role"], "serve");
        assert_eq!(fix["packages"], json!(["iperf3"]));
        assert_eq!(fix["message"], "Roteador B sem iperf3 não tem o iperf3");

        // Rodar sem aceitar a instalação não passa.
        let mut confirmado = pedido(link_id);
        confirmado["confirm"] = json!(true);
        let recusado = request.post("/api/link-tests").json(&confirmado).await;
        recusado.assert_status(StatusCode::BAD_REQUEST);
        assert!(
            recusado.text().contains("não tem o iperf3"),
            "{}",
            recusado.text()
        );

        // "Instalar": a ação de sempre, com a confirmação explícita da escrita.
        let plugin_id = fix["pluginId"].as_i64().unwrap();
        let rota = format!("/api/devices/{b}/plugins/{plugin_id}/actions/tp_prepare");
        let params = json!({ "engine": fix["engine"], "role": fix["role"] });
        request
            .post(&rota)
            .json(&json!({ "params": params, "confirmWrite": false }))
            .await
            .assert_status(StatusCode::BAD_REQUEST);
        let iniciado = request
            .post(&rota)
            .json(&json!({ "params": params, "confirmWrite": true }))
            .await;
        iniciado.assert_status(StatusCode::ACCEPTED);
        let run_id = json_of(&iniciado.text())["runId"].as_i64().unwrap();
        let run = run_finished(&request, run_id).await;
        assert_eq!(run["status"], "succeeded", "{run}");
        assert_eq!(run["output"]["installed"], json!(["iperf3"]));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn operador_nao_instala_pacote() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let (a, _, _) = cenario(&ctx).await;
        let plugin_id = plugins::Entity::find()
            .all(&ctx.db)
            .await
            .unwrap()
            .into_iter()
            .find(|plugin| plugin.slug == "lt-teste")
            .unwrap()
            .id;
        // Instalar é escrita de plugin: só o administrador (a tela mostra a
        // quem pedir).
        request
            .post(&format!(
                "/api/devices/{a}/plugins/{plugin_id}/actions/tp_prepare"
            ))
            .json(
                &json!({ "params": { "engine": "iperf3", "role": "serve" }, "confirmWrite": true }),
            )
            .await
            .assert_status(StatusCode::FORBIDDEN);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn agendamentos_crud_e_rodar_agora() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;
        let (_, _, link_id) = cenario(&ctx).await;
        let corpo = json!({
            "name": "Torre norte", "kind": "link", "linkId": link_id,
            "sourceDeviceId": null, "targetDeviceId": null,
            "intervalMinutes": 60, "windowStart": "22:00", "windowEnd": "06:00",
            "enabled": true, "spec": { "durationS": 1 }
        });
        let criado = request.post("/api/link-test-schedules").json(&corpo).await;
        criado.assert_status(StatusCode::CREATED);
        let criado = json_of(&criado.text());
        let id = criado["id"].as_i64().unwrap();
        assert_eq!(criado["spec"]["durationS"], 1);
        assert_eq!(criado["windowStart"], "22:00");
        assert!(criado["nextRunAt"].is_string());

        let mut ruim = corpo.clone();
        ruim["intervalMinutes"] = json!(5);
        request
            .post("/api/link-test-schedules")
            .json(&ruim)
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);

        let mut editado = corpo.clone();
        editado["name"] = json!("Torre norte (madrugada)");
        editado["enabled"] = json!(false);
        let editado = json_of(
            &request
                .put(&format!("/api/link-test-schedules/{id}"))
                .json(&editado)
                .await
                .text(),
        );
        assert_eq!(editado["enabled"], false);
        assert!(editado["nextRunAt"].is_null(), "desligado não tem próxima");

        let rodou = request
            .post(&format!("/api/link-test-schedules/{id}/run"))
            .await;
        rodou.assert_status(StatusCode::ACCEPTED);
        let test_id = json_of(&rodou.text())["id"].as_i64().unwrap();
        let teste = finished(&request, test_id).await;
        assert_eq!(teste["origin"], "schedule");
        assert_eq!(teste["scheduleId"], id);
        assert_eq!(teste["status"], "succeeded");

        let lista = json_of(&request.get("/api/link-test-schedules").await.text());
        assert_eq!(lista[0]["lastTestId"], test_id);
        assert_eq!(lista[0]["lastStatus"], "succeeded");
        assert!(lista[0]["lastRunAt"].is_string());

        request
            .delete(&format!("/api/link-test-schedules/{id}"))
            .await
            .assert_status(StatusCode::NO_CONTENT);
        request
            .get(&format!("/api/link-test-schedules/{id}"))
            .await
            .assert_status(StatusCode::NOT_FOUND);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn visualizador_le_mas_nao_roda_nem_agenda() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Viewer).await;
        let (_, _, link_id) = cenario(&ctx).await;
        request.get("/api/link-tests").await.assert_status_ok();
        request
            .get("/api/link-test-schedules")
            .await
            .assert_status_ok();
        let mut confirmado = pedido(link_id);
        confirmado["confirm"] = json!(true);
        request
            .post("/api/link-tests")
            .json(&confirmado)
            .await
            .assert_status(StatusCode::FORBIDDEN);
        request
            .post("/api/link-tests/plan")
            .json(&pedido(link_id))
            .await
            .assert_status(StatusCode::FORBIDDEN);
        request
            .post("/api/link-tests/1/cancel")
            .await
            .assert_status(StatusCode::FORBIDDEN);
        request
            .post("/api/link-test-schedules")
            .json(&json!({ "name": "x", "kind": "link", "linkId": link_id, "intervalMinutes": 60 }))
            .await
            .assert_status(StatusCode::FORBIDDEN);
    })
    .await;
}
