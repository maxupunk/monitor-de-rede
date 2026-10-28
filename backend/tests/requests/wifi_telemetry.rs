//! Sinais e clientes Wi-Fi pela API: a capacidade do equipamento, a leitura
//! em segundo plano publicada pelo SSE (`wifi:snapshot`), o ping até uma
//! estação e o vigia dos aparelhos que deveriam estar sempre conectados — com
//! um plugin de teste que fala com um "AP" em 127.0.0.1.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{routing::get, Router};
use backend::{
    app::App,
    models::{_entities::alert_events, alert_rules, devices, wifi_watched_clients},
    services::{
        events::EventBus,
        users::Role,
        wifi::{self, watchlist::WATCHED_EVENT, SNAPSHOT_EVENT},
    },
};
use chrono::Utc;
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

async fn session(request: &mut loco_rs::TestServer, ctx: &AppContext, role: Role) {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.as_str().to_string());
    active.update(&ctx.db).await.unwrap();
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

async fn device(ctx: &AppContext, name: &str) -> devices::Model {
    devices::ActiveModel {
        name: Set(name.into()),
        r#type: Set("access_point".into()),
        status: Set("online".into()),
        ip_address: Set(Some("127.0.0.1".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

/// As estações que o "AP" entrega; o teste tira e devolve aparelhos.
type Stations = Arc<Mutex<Vec<Value>>>;

fn notebook() -> Value {
    json!({ "mac": "AA:BB:CC:DD:EE:01", "hostname": "notebook", "ip": "192.168.1.150",
            "ssid": "Loja", "band": "5g", "channel": 36, "signal_dbm": -58, "noise_dbm": -92,
            "tx_packets": 900, "tx_retries": 100, "inactive_ms": 40 })
}

/// Um "AP" que entrega as estações no contrato dos plugins Wi-Fi.
async fn fake_ap() -> (u16, Stations) {
    let stations: Stations = Arc::new(Mutex::new(vec![
        notebook(),
        json!({ "mac": "3a:11:22:33:44:55", "band": "2g", "signal_dbm": -83, "inactive_ms": 20 }),
        json!({ "mac": "sem-mac", "signal_dbm": -40 }),
    ]));
    let served = stations.clone();
    let app = Router::new().route(
        "/clients",
        get(move || {
            let clients = served.lock().unwrap().clone();
            async move { json!({ "summary": "clientes", "clients": clients }).to_string() }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (port, stations)
}

/// Plugin mínimo com o bloco `wifi`: lista pela página do AP e "pinga".
fn package() -> Value {
    json!({
        "format": 1,
        "manifest": {
            "slug": "ap-teste", "name": "AP de teste", "version": "1.0.0",
            "transports": ["http"],
            "actions": [
                { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" },
                { "id": "clients", "title": "Clientes", "effect": "read", "output": "report" },
                { "id": "ping", "title": "Latência", "effect": "read", "output": "kv",
                  "params": { "type": "object", "properties": {
                      "target": { "type": "string", "pattern": "[0-9a-fA-F.:]{2,45}" } },
                      "required": ["target"] } }
            ],
            "wifi": { "clients": "clients", "ping": "ping" }
        },
        "script": "fn detect(device, params) { #{ firmware: \"1.0\" } }
fn clients(device, params) { json_parse(device.get(\"/clients\").body) }
fn ping(device, params) { #{ summary: params.target, sent: 3, received: 2, avg_ms: 4.5 } }",
        "usage": "## Detectar\nVersão.\n## Clientes\nEstações.\n## Latência\nPing.",
        "tests": {
            "unit": [
                { "action": "detect", "expect": { "firmware": "1.0" } },
                { "action": "clients",
                  "fixtures": [ { "http": "GET /clients", "body": "{\"clients\": []}" } ],
                  "expect": { "clients": [] } },
                { "action": "ping", "params": { "target": "10.0.0.2" },
                  "expect": { "sent": 3, "received": 2 } }
            ],
            "functional": [
                { "action": "detect", "expectKeys": ["firmware"] },
                { "action": "clients", "expectKeys": ["clients"] }
            ]
        }
    })
}

/// Um AP com o plugin de teste ativo, instalado e com acesso HTTP.
async fn ap_with_wifi(
    request: &loco_rs::TestServer,
    ctx: &AppContext,
) -> (devices::Model, Stations) {
    let ap = device(ctx, "AP Sala").await;
    let created = json_of(
        &request
            .post("/api/plugins")
            .json(&json!({ "package": package() }))
            .await
            .text(),
    );
    let id = created["summary"]["id"].as_i64().unwrap();
    assert_eq!(created["summary"]["wifi"], true, "a frota leva ao painel");
    let test = json_of(
        &request
            .post(&format!("/api/plugins/{id}/test"))
            .await
            .text(),
    );
    assert_eq!(test["passed"], true, "{test}");
    request
        .post(&format!("/api/plugins/{id}/promote"))
        .await
        .assert_status_success();
    let (port, stations) = fake_ap().await;
    request
        .put(&format!("/api/devices/{}/credentials", ap.id))
        .json(
            &json!({ "kind": "http", "username": "admin", "storage": "vault",
                       "secret": "s3nh4", "port": port }),
        )
        .await
        .assert_status_success();
    request
        .post(&format!("/api/devices/{}/plugins/{id}/install", ap.id))
        .await
        .assert_status_success();
    (ap, stations)
}

/// O snapshot do equipamento no barramento, quando a leitura terminar.
async fn finished_snapshot(ctx: &AppContext, device_id: i64) -> Value {
    let bus = EventBus::from_context(ctx).unwrap();
    for _ in 0..200 {
        let found = bus
            .current_snapshots()
            .into_iter()
            .filter(|event| event.event_type == SNAPSHOT_EVENT)
            .map(|event| event.payload)
            .find(|payload| payload["deviceId"] == device_id && payload["status"] != "reading");
        if let Some(snapshot) = found {
            return snapshot;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("a leitura do equipamento {device_id} não terminou");
}

#[tokio::test]
#[serial]
async fn le_classifica_e_publica_as_estacoes_pelo_sse() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Admin).await;
        let (ap, _) = ap_with_wifi(&request, &ctx).await;

        let caps = json_of(
            &request
                .get(&format!("/api/devices/{}/capabilities", ap.id))
                .await
                .text(),
        );
        assert_eq!(caps["wifi"], true, "plugin com bloco `wifi` dá a aba");

        let started = request
            .post("/api/wifi/refresh")
            .json(&json!({ "deviceId": ap.id }))
            .await;
        assert_eq!(started.status_code(), 202);
        assert_eq!(json_of(&started.text())["deviceIds"], json!([ap.id]));

        let snapshot = finished_snapshot(&ctx, ap.id).await;
        assert_eq!(snapshot["status"], "ok", "{snapshot}");
        assert_eq!(snapshot["deviceName"], "AP Sala");
        assert_eq!(snapshot["canPing"], true);
        assert!(snapshot["readAt"].is_string());
        let stations = snapshot["stations"].as_array().unwrap();
        assert_eq!(stations.len(), 2, "MAC inválido não vira estação");
        // Da pior para a melhor.
        let fraca = &stations[0];
        assert_eq!(fraca["mac"], "3a:11:22:33:44:55");
        assert_eq!(fraca["health"], "critical");
        assert_eq!(fraca["signalGrade"], "poor");
        assert_eq!(fraca["randomizedMac"], true, "MAC privado de celular");
        assert_eq!(fraca["snrDb"], Value::Null, "sem ruído não há SNR");
        let boa = &stations[1];
        assert_eq!(boa["mac"], "aa:bb:cc:dd:ee:01");
        assert_eq!(boa["band"], "5g");
        assert_eq!(boa["snrDb"], 34);
        assert_eq!(boa["txRetriesPct"], 10.0);
        assert_eq!(boa["health"], "healthy");

        let ping = request
            .post("/api/wifi/ping")
            .json(&json!({ "deviceId": ap.id, "target": "192.168.1.150" }))
            .await;
        assert_eq!(ping.status_code(), 200, "{}", ping.text());
        let ping = json_of(&ping.text());
        assert_eq!(ping["avgMs"], 4.5);
        assert_eq!(ping["lossPct"], 33.3);
        assert_eq!(ping["grade"], "critical", "perda é crítica");

        // Ler todos inclui o AP.
        let all = request.post("/api/wifi/refresh").await;
        assert_eq!(all.status_code(), 202);
        assert_eq!(json_of(&all.text())["deviceIds"], json!([ap.id]));
        finished_snapshot(&ctx, ap.id).await;
    })
    .await;
}

#[tokio::test]
#[serial]
async fn equipamento_sem_plugin_wifi_explica_o_que_falta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Operator).await;
        let ap = device(&ctx, "AP sem plugin").await;

        let caps = json_of(
            &request
                .get(&format!("/api/devices/{}/capabilities", ap.id))
                .await
                .text(),
        );
        assert_eq!(caps["wifi"], false, "tipo `access_point` não basta");

        let all = request.post("/api/wifi/refresh").await;
        assert_eq!(all.status_code(), 202);
        assert_eq!(json_of(&all.text())["deviceIds"], json!([]));

        let one = request
            .post("/api/wifi/refresh")
            .json(&json!({ "deviceId": ap.id }))
            .await;
        assert_eq!(one.status_code(), 400);
        assert!(one.text().contains("não tem um plugin de Wi-Fi ativo"));

        let missing = request
            .post("/api/wifi/refresh")
            .json(&json!({ "deviceId": 999_999 }))
            .await;
        assert_eq!(missing.status_code(), 404);

        let ping = request
            .post("/api/wifi/ping")
            .json(&json!({ "deviceId": ap.id, "target": "192.168.1.150" }))
            .await;
        assert_eq!(ping.status_code(), 400);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn ping_valida_o_ip_antes_de_tocar_o_equipamento() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Operator).await;
        let ap = device(&ctx, "AP").await;
        for target in ["nao-eh-ip", "192.168.1.1; reboot", ""] {
            let response = request
                .post("/api/wifi/ping")
                .json(&json!({ "deviceId": ap.id, "target": target }))
                .await;
            assert_eq!(response.status_code(), 422, "{target}");
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn visualizador_nao_dispara_leitura_nem_ping() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Viewer).await;
        let ap = device(&ctx, "AP").await;
        let refresh = request.post("/api/wifi/refresh").await;
        assert_eq!(refresh.status_code(), 403);
        let ping = request
            .post("/api/wifi/ping")
            .json(&json!({ "deviceId": ap.id, "target": "192.168.1.150" }))
            .await;
        assert_eq!(ping.status_code(), 403);
    })
    .await;
}

/// A lista dos vigiados como a tela a recebe pelo SSE.
fn watched_snapshot(ctx: &AppContext) -> Value {
    EventBus::from_context(ctx)
        .unwrap()
        .current_snapshots()
        .into_iter()
        .find(|event| event.event_type == WATCHED_EVENT)
        .map(|event| event.payload)
        .expect("a lista dos vigiados está no barramento")
}

async fn events_of(ctx: &AppContext, watched_id: i64) -> Vec<alert_events::Model> {
    alert_events::Entity::find()
        .filter(alert_events::Column::ScopeKey.eq(format!("wifi_client:{watched_id}")))
        .all(&ctx.db)
        .await
        .unwrap()
}

#[tokio::test]
#[serial]
async fn aparelho_vigiado_que_some_vira_alerta_e_volta_resolvendo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Admin).await;
        let (ap, stations) = ap_with_wifi(&request, &ctx).await;

        // Cadastro: MAC normalizado, nasce aguardando a primeira leitura.
        let created = request
            .post("/api/wifi/watched")
            .json(&json!({ "mac": "AA-BB-CC-DD-EE-01", "name": "  Câmera da portaria " }))
            .await;
        assert_eq!(created.status_code(), 201, "{}", created.text());
        let created = json_of(&created.text());
        let id = created["id"].as_i64().unwrap();
        assert_eq!(created["mac"], "aa:bb:cc:dd:ee:01");
        assert_eq!(created["name"], "Câmera da portaria");
        assert_eq!(created["state"], "pending");
        assert_eq!(watched_snapshot(&ctx)["clients"][0]["id"], id);

        let repetido = request
            .post("/api/wifi/watched")
            .json(&json!({ "mac": "aa:bb:cc:dd:ee:01", "name": "Outra" }))
            .await;
        assert_eq!(repetido.status_code(), 409);
        let invalido = request
            .post("/api/wifi/watched")
            .json(&json!({ "mac": "nao-eh-mac", "name": "X" }))
            .await;
        assert_eq!(invalido.status_code(), 422);

        // O primeiro vigiado instala as regras de Wi-Fi.
        let regra = alert_rules::Entity::find()
            .filter(alert_rules::Column::TemplateKey.eq("wifi_watched_offline"))
            .one(&ctx.db)
            .await
            .unwrap();
        assert!(regra.is_some(), "regra de queda instalada");

        // Conectado: sem alerta.
        assert_eq!(wifi::watch::run(&ctx).await.unwrap(), 1);
        let cliente = watched_snapshot(&ctx)["clients"][0].clone();
        assert_eq!(cliente["state"], "connected", "{cliente}");
        assert_eq!(cliente["deviceId"], ap.id);
        assert_eq!(cliente["deviceName"], "AP Sala");
        assert!(events_of(&ctx, id).await.is_empty());

        // Some do AP (o celular de MAC privado também some — e não é vigiado).
        stations.lock().unwrap().clear();
        let mut visto: wifi_watched_clients::ActiveModel =
            wifi_watched_clients::Entity::find_by_id(id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .into();
        visto.last_seen_at = Set(Some((Utc::now() - chrono::Duration::minutes(15)).into()));
        visto.update(&ctx.db).await.unwrap();

        assert_eq!(wifi::watch::run(&ctx).await.unwrap(), 1);
        assert_eq!(watched_snapshot(&ctx)["clients"][0]["state"], "offline");
        let eventos = events_of(&ctx, id).await;
        assert_eq!(eventos.len(), 1, "um alerta, só do vigiado");
        assert_eq!(eventos[0].status, "active");
        assert_eq!(eventos[0].severity, "critical");
        let mensagem = eventos[0].message.clone().unwrap_or_default();
        assert!(mensagem.contains("fora do Wi-Fi"), "{mensagem}");
        let total = alert_events::Entity::find().all(&ctx.db).await.unwrap();
        assert_eq!(total.len(), 1, "aparelho não vigiado nunca alerta");

        // Volta: o alerta entra em recuperação.
        stations.lock().unwrap().push(notebook());
        wifi::watch::run(&ctx).await.unwrap();
        assert_eq!(watched_snapshot(&ctx)["clients"][0]["state"], "connected");
        let status = events_of(&ctx, id).await[0].status.clone();
        assert!(
            status == "recovering" || status == "resolved",
            "status após a volta: {status}"
        );

        // Renomear e deixar de vigiar fecham o ciclo.
        let renomeado = request
            .patch(&format!("/api/wifi/watched/{id}"))
            .json(&json!({ "name": "Câmera do portão" }))
            .await;
        assert_eq!(renomeado.status_code(), 200);
        assert_eq!(json_of(&renomeado.text())["name"], "Câmera do portão");
        let removido = request.delete(&format!("/api/wifi/watched/{id}")).await;
        assert_eq!(removido.status_code(), 204);
        assert_eq!(events_of(&ctx, id).await[0].status, "resolved");
        assert_eq!(watched_snapshot(&ctx)["clients"], json!([]));
        assert_eq!(
            wifi::watch::run(&ctx).await.unwrap(),
            0,
            "sem vigiado, sem leitura"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn visualizador_nao_vigia_aparelho() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session(&mut request, &ctx, Role::Viewer).await;
        let response = request
            .post("/api/wifi/watched")
            .json(&json!({ "mac": "aa:bb:cc:dd:ee:01", "name": "Câmera" }))
            .await;
        assert_eq!(response.status_code(), 403);
        let response = request.delete("/api/wifi/watched/1").await;
        assert_eq!(response.status_code(), 403);
    })
    .await;
}
