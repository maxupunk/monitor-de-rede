//! Plugins de frota pela API: aplicativo no menu, configuração guardada com
//! segredo mascarado, membros (instalação), ação em lote com consolidado e o
//! ajuste sugerido aplicado à configuração de cada equipamento.

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

async fn admin(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(Role::Admin.as_str().to_string());
    active.update(&ctx.db).await.unwrap();
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

/// Um "AP" com página de status em 127.0.0.1.
async fn web_ap(clients: u32) -> u16 {
    let app = Router::new().route(
        "/",
        get(move || async move { format!("clientes={clients}") }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    port
}

async fn device(ctx: &loco_rs::app::AppContext, name: &str) -> devices::Model {
    devices::ActiveModel {
        name: Set(name.into()),
        r#type: Set("ap".into()),
        status: Set("online".into()),
        ip_address: Set(Some("127.0.0.1".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

/// Um plugin de frota mínimo: lê a página do AP, mostra a etiqueta da frota
/// e soma os clientes de todos no consolidado, sugerindo um ajuste.
fn package() -> Value {
    json!({
        "format": 1,
        "manifest": {
            "slug": "frota-teste", "name": "Frota de teste", "version": "1.0.0",
            "transports": ["http"],
            "surfaces": ["device", "fleet"],
            "settings": {
                "fleet": { "type": "object", "properties": {
                    "label": { "type": "string", "default": "rede" },
                    "token": { "type": "string", "secret": true } } },
                "device": { "type": "object", "properties": {
                    "slot": { "type": "string", "default": "" } } }
            },
            "actions": [
                { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" },
                { "id": "status", "title": "Estado", "effect": "read", "output": "kv" }
            ],
            "fleet": {
                "title": "Frota de teste", "icon": "mdi-lan",
                "statusAction": "status",
                "actions": [ { "id": "total", "title": "Somar clientes", "action": "status",
                               "reduce": "somar" } ]
            }
        },
        "script": "fn detect(device, params) { #{ firmware: \"1.0\" } }
fn status(device, params) {
    let body = device.get(\"/\").body;
    #{ label: device.settings.fleet.label, token: device.settings.fleet.token,
       clients: parse_int(regex_capture(body, \"clientes=(\\\\d+)\")) }
}
fn somar(results, settings) {
    let total = 0;
    let patch = #{};
    for r in results {
        total += r.output.clients;
        patch[r.deviceId.to_string()] = #{ slot: \"ok-\" + r.output.clients };
    }
    #{ total: total, settings_patch: patch }
}",
        "usage": "## Detectar\nVersão.\n## Estado\nClientes do AP.",
        "tests": {
            "unit": [
                { "action": "detect", "expect": { "firmware": "1.0" } },
                { "action": "status",
                  "settings": { "fleet": { "token": "segredo-1" } },
                  "fixtures": [ { "http": "GET /", "body": "clientes=3" } ],
                  "expect": { "label": "rede", "clients": 3, "token": "********" } },
                { "action": "total",
                  "input": [ { "deviceId": 1, "status": "succeeded", "output": { "clients": 2 } } ],
                  "expect": { "total": 2 } }
            ],
            "functional": [
                { "action": "detect", "expectKeys": ["firmware"] },
                { "action": "status", "expectKeys": ["clients"] }
            ]
        }
    })
}

/// Dois APs com página em 127.0.0.1 e credencial HTTP cadastrada.
async fn two_aps(
    request: &loco_rs::TestServer,
    ctx: &loco_rs::app::AppContext,
) -> (devices::Model, devices::Model) {
    let sala = device(ctx, "AP Sala").await;
    let deposito = device(ctx, "AP Depósito").await;
    let (porta_sala, porta_deposito) = (web_ap(2).await, web_ap(5).await);
    for (ap, porta) in [(&sala, porta_sala), (&deposito, porta_deposito)] {
        request
            .put(&format!("/api/devices/{}/credentials", ap.id))
            .json(
                &json!({ "kind": "http", "username": "admin", "storage": "vault",
                           "secret": "s3nh4", "port": porta }),
            )
            .await
            .assert_status_success();
    }
    (sala, deposito)
}

/// Cria, testa e ativa um pacote; devolve o id.
async fn active_plugin(request: &loco_rs::TestServer, package: Value) -> i64 {
    let criado = json_of(
        &request
            .post("/api/plugins")
            .json(&json!({ "package": package }))
            .await
            .text(),
    );
    let id = criado["summary"]["id"].as_i64().unwrap();
    let teste = json_of(
        &request
            .post(&format!("/api/plugins/{id}/test"))
            .await
            .text(),
    );
    assert_eq!(teste["passed"], true, "{teste}");
    request
        .post(&format!("/api/plugins/{id}/promote"))
        .await
        .assert_status_success();
    id
}

async fn finished_batch(request: &loco_rs::TestServer, batch_id: i64) -> Value {
    for _ in 0..200 {
        let batch = json_of(
            &request
                .get(&format!("/api/plugin-batches/{batch_id}"))
                .await
                .text(),
        );
        if batch["finishedAt"].is_string() {
            return batch;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("o lote {batch_id} não terminou");
}

#[tokio::test]
#[serial]
async fn frota_configura_executa_em_lote_e_aplica_o_ajuste_sugerido() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        admin(&mut request, &ctx).await;
        let (sala, deposito) = two_aps(&request, &ctx).await;
        // Plugin criado, testado e ativado.
        let id = active_plugin(&request, package()).await;

        // Aparece nos Aplicativos, ainda sem membros.
        let apps = json_of(&request.get("/api/plugins/apps").await.text());
        let app = apps
            .as_array()
            .unwrap()
            .iter()
            .find(|app| app["id"] == id)
            .expect("aplicativo listado");
        assert_eq!(app["members"], 0);
        assert_eq!(app["title"], "Frota de teste");

        // Sem membros, a ação não tem quem atender.
        request
            .post(&format!("/api/plugins/{id}/fleet/actions/status"))
            .await
            .assert_status_bad_request();

        // Membros: instalar nos dois.
        for ap in [&sala, &deposito] {
            request
                .post(&format!("/api/devices/{}/plugins/{id}/install", ap.id))
                .await
                .assert_status_success();
        }

        // Configuração da frota com segredo: gravada, mascarada na volta.
        let salvo = json_of(
            &request
                .put(&format!("/api/plugins/{id}/settings"))
                .json(&json!({ "value": { "label": "Loja", "token": "tok-123456" } }))
                .await
                .text(),
        );
        assert_eq!(salvo["token"], "********");
        let pagina = request.get(&format!("/api/plugins/{id}/fleet")).await;
        assert!(!pagina.text().contains("tok-123456"));
        let pagina = json_of(&pagina.text());
        assert_eq!(pagina["members"].as_array().unwrap().len(), 2);
        assert_eq!(pagina["members"][0]["credentialsReady"], true);
        assert_eq!(pagina["settings"]["label"], "Loja");

        // Ação em lote com consolidado.
        let iniciado = request
            .post(&format!("/api/plugins/{id}/fleet/actions/total"))
            .json(&json!({}))
            .await;
        iniciado.assert_status(axum::http::StatusCode::ACCEPTED);
        let batch_id = json_of(&iniciado.text())["batchId"].as_i64().unwrap();
        let lote = finished_batch(&request, batch_id).await;
        assert_eq!(lote["status"], "succeeded", "{lote}");
        assert_eq!(lote["result"]["total"], 7, "{lote}");
        assert_eq!(lote["hasPatch"], true);
        let saidas: Vec<&Value> = lote["devices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| &item["output"])
            .collect();
        assert!(saidas.iter().all(|saida| saida["label"] == "Loja"));
        assert!(
            !lote.to_string().contains("tok-123456"),
            "o segredo da frota não sai no lote"
        );

        // O ajuste sugerido vai para a configuração de cada equipamento.
        let aplicado = json_of(
            &request
                .post(&format!("/api/plugin-batches/{batch_id}/apply-patch"))
                .await
                .text(),
        );
        assert_eq!(aplicado["hasPatch"], false);
        let pagina = json_of(
            &request
                .get(&format!("/api/plugins/{id}/fleet"))
                .await
                .text(),
        );
        let slots: Vec<&str> = pagina["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| member["settings"]["slot"].as_str().unwrap())
            .collect();
        assert!(
            slots.contains(&"ok-2") && slots.contains(&"ok-5"),
            "{slots:?}"
        );

        // Uma só aplicação: o segundo clique é recusado.
        request
            .post(&format!("/api/plugin-batches/{batch_id}/apply-patch"))
            .await
            .assert_status_bad_request();
    })
    .await;
}

/// Ação com parâmetro secreto e pré-visualização: lê a página e devolve a
/// etiqueta e o PIN que recebeu.
fn package_with_params() -> Value {
    json!({
        "format": 1,
        "manifest": {
            "slug": "frota-parametros", "name": "Frota com parâmetros", "version": "1.0.0",
            "transports": ["http"],
            "surfaces": ["fleet"],
            "actions": [
                { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" },
                { "id": "marcar", "title": "Marcar", "effect": "read", "output": "kv",
                  "params": { "type": "object", "properties": {
                      "tag": { "type": "string", "pattern": "[A-Z]{0,4}", "default": "" },
                      "pin": { "type": "string", "secret": true, "pattern": "[a-z0-9-]{0,32}" } } } }
            ],
            "fleet": {
                "title": "Frota com parâmetros",
                "actions": [ { "id": "marcar_todos", "title": "Marcar", "action": "marcar",
                               "preview": "marcar" } ]
            }
        },
        "script": "fn detect(device, params) { #{ firmware: \"1.0\" } }
fn marcar(device, params) {
    let body = device.get(\"/\").body;
    #{ tag: params.tag, pin: params.pin, page: body }
}",
        "usage": "## Detectar\nVersão.\n## Marcar\nEtiqueta.",
        "tests": {
            "unit": [
                { "action": "detect", "expect": { "firmware": "1.0" } },
                { "action": "marcar", "params": { "tag": "A", "pin": "pin-123" },
                  "fixtures": [ { "http": "GET /", "body": "ok" } ],
                  "expect": { "tag": "A", "pin": "********" } }
            ],
            "functional": [
                { "action": "detect", "expectKeys": ["firmware"] },
                { "action": "marcar", "params": { "tag": "T" }, "expectKeys": ["tag"] }
            ]
        }
    })
}

#[tokio::test]
#[serial]
async fn lote_com_parametro_por_equipamento_e_segredo_mascarado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        admin(&mut request, &ctx).await;
        let (sala, deposito) = two_aps(&request, &ctx).await;
        let id = active_plugin(&request, package_with_params()).await;
        for ap in [&sala, &deposito] {
            request
                .post(&format!("/api/devices/{}/plugins/{id}/install", ap.id))
                .await
                .assert_status_success();
        }

        // Comum (o PIN) + o de cada um (a etiqueta).
        let iniciado = request
            .post(&format!("/api/plugins/{id}/fleet/actions/marcar_todos"))
            .json(&json!({
                "params": { "pin": "pin-secreto-9" },
                "deviceParams": { sala.id.to_string(): { "tag": "A" },
                                  deposito.id.to_string(): { "tag": "B" } }
            }))
            .await;
        iniciado.assert_status(axum::http::StatusCode::ACCEPTED);
        let lote = finished_batch(
            &request,
            json_of(&iniciado.text())["batchId"].as_i64().unwrap(),
        )
        .await;
        assert_eq!(lote["status"], "succeeded", "{lote}");
        let de = |device_id: i64| {
            lote["devices"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["deviceId"] == device_id)
                .unwrap()
                .clone()
        };
        assert_eq!(de(sala.id)["output"]["tag"], "A");
        assert_eq!(de(deposito.id)["output"]["tag"], "B");
        assert_eq!(de(sala.id)["output"]["pin"], "********");
        assert!(!lote.to_string().contains("pin-secreto-9"));
        // O histórico da execução também guarda a máscara.
        let run_id = de(sala.id)["runId"].as_i64().unwrap();
        let run = request
            .get(&format!("/api/plugin-runs/{run_id}"))
            .await
            .text();
        assert!(!run.contains("pin-secreto-9"), "{run}");

        // A pré-visualização de uma ação de frota roda como lote também.
        request
            .post(&format!("/api/plugins/{id}/fleet/actions/marcar"))
            .json(&json!({ "params": { "tag": "P" } }))
            .await
            .assert_status(axum::http::StatusCode::ACCEPTED);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn wifi_nasce_desligado_e_liga_ao_cadastrar_um_openwrt() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        admin(&mut request, &ctx).await;
        let slugs = |apps: &Value| -> Vec<String> {
            apps.as_array()
                .unwrap()
                .iter()
                .map(|app| app["slug"].as_str().unwrap().to_owned())
                .collect()
        };
        let antes = json_of(&request.get("/api/plugins/apps").await.text());
        assert!(!slugs(&antes).contains(&"openwrt-wifi".to_owned()));

        request
            .post("/api/devices")
            .json(
                &json!({ "name": "AP Recepção", "ipAddress": "127.0.0.1", "type": "router",
                           "operatingSystem": "openwrt" }),
            )
            .await
            .assert_status_success();

        let depois = json_of(&request.get("/api/plugins/apps").await.text());
        let wifi = depois
            .as_array()
            .unwrap()
            .iter()
            .find(|app| app["slug"] == "openwrt-wifi")
            .expect("Rede Wi-Fi ligada pelo cadastro");
        assert_eq!(wifi["title"], "Rede Wi-Fi");
        let id = wifi["id"].as_i64().unwrap();
        let pagina = json_of(
            &request
                .get(&format!("/api/plugins/{id}/fleet"))
                .await
                .text(),
        );
        // O roteador é a fonte da verdade: nada de configuração guardada.
        assert_eq!(pagina["settings"], Value::Null);
        assert_eq!(pagina["plugin"]["fleet"]["statusAction"], "status");
        assert_eq!(
            pagina["plugin"]["fleet"]["list"]["edit"]["action"],
            "network"
        );

        // Equipamento de outro sistema aparece como candidato, incompatível e
        // com o porquê — nunca some sem explicação.
        request
            .post("/api/devices")
            .json(
                &json!({ "name": "Borda MikroTik", "ipAddress": "127.0.0.2", "type": "router",
                           "operatingSystem": "routeros" }),
            )
            .await
            .assert_status_success();
        let pagina = json_of(
            &request
                .get(&format!("/api/plugins/{id}/fleet"))
                .await
                .text(),
        );
        let candidatos = pagina["candidates"].as_array().unwrap();
        let mikrotik = candidatos
            .iter()
            .find(|item| item["name"] == "Borda MikroTik")
            .expect("incompatível listado");
        assert_eq!(mikrotik["compat"], "incompatible");
        assert!(mikrotik["reasons"][0].as_str().unwrap().contains("openwrt"));
        assert!(candidatos.iter().any(|item| item["name"] == "AP Recepção"));

        // "Verificar sistema" devolve a página refeita; o sistema declarado no
        // cadastro continua decidindo (a sonda não o sobrescreve).
        let verificada = request
            .post(&format!("/api/plugins/{id}/fleet/identify"))
            .json(&json!({ "deviceIds": [mikrotik["deviceId"]] }))
            .await;
        verificada.assert_status_success();
        let verificada = json_of(&verificada.text());
        let mikrotik = verificada["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == "Borda MikroTik")
            .expect("continua listado");
        assert_eq!(mikrotik["compat"], "incompatible");
    })
    .await;
}
