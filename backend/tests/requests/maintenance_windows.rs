//! Janelas de manutenção (Fase 3 do roadmap).
//!
//! Valida o CRUD e, o mais importante, o efeito no diário de notificações:
//! durante uma janela vigente, alertas ainda são criados, mas a linha do
//! `notification_outbox` nasce `suppressed` com motivo `maintenance`.

use backend::{
    app::App,
    models::notification_outbox,
    services::monitoring::{
        contracts::{CheckResult, MonitorStatus},
        result_processor::process_result,
    },
};
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use sea_orm::{EntityTrait, QueryOrder};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

fn resultado(status: MonitorStatus) -> CheckResult {
    let now = Utc::now();
    CheckResult {
        success: status == MonitorStatus::Up,
        status,
        started_at: now,
        finished_at: now,
        duration_ms: 5,
        message: None,
        metrics: vec![],
        data: json!({}),
    }
}

async fn diario(ctx: &loco_rs::app::AppContext) -> Vec<notification_outbox::Model> {
    notification_outbox::Entity::find()
        .order_by_asc(notification_outbox::Column::Id)
        .all(&ctx.db)
        .await
        .expect("diário de notificações")
}

async fn so_a_regra_do_teste(request: &loco_rs::TestServer, payload: Value) -> i64 {
    let existentes = json_of(&request.get("/api/alert-rules").await.text());
    for regra in existentes.as_array().expect("lista de regras") {
        let id = regra["id"].as_i64().unwrap();
        request.delete(&format!("/api/alert-rules/{id}")).await;
    }
    let criada = json_of(&request.post("/api/alert-rules").json(&payload).await.text());
    criada["id"].as_i64().expect("regra criada")
}

fn sem_agrupamento() {
    std::env::set_var("NOTIFICATION_DIGEST_WINDOW_SECONDS", "0");
    std::env::set_var("NOTIFICATION_DIGEST_WAIT_SECONDS", "0");
}

fn agrupamento_padrao() {
    std::env::remove_var("NOTIFICATION_DIGEST_WINDOW_SECONDS");
    std::env::remove_var("NOTIFICATION_DIGEST_WAIT_SECONDS");
}

#[tokio::test]
#[serial]
async fn crud_de_janelas_respeita_contrato_e_validacoes() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let site = request
            .post("/api/sites")
            .json(&json!({"name": "Matriz"}))
            .await;
        assert_eq!(site.status_code(), 201);
        let site = json_of(&site.text());
        let site_id = site["id"].as_i64().unwrap();

        let inicio = Utc::now() + Duration::minutes(5);
        let fim = inicio + Duration::hours(2);

        let criada = request
            .post("/api/maintenance-windows")
            .json(&json!({
                "siteId": site_id,
                "name": "Manutenção no ar condicionado",
                "description": "Desligamento programado",
                "startsAt": inicio.to_rfc3339(),
                "endsAt": fim.to_rfc3339(),
            }))
            .await;
        assert_eq!(criada.status_code(), 201, "{}", criada.text());
        let criada = json_of(&criada.text());
        assert_eq!(criada["name"], "Manutenção no ar condicionado");
        assert_eq!(criada["siteId"], site_id);
        assert_eq!(criada["deviceId"], Value::Null);
        assert!(criada["createdBy"].is_number());

        let lista = request.get("/api/maintenance-windows").await;
        assert_eq!(lista.status_code(), 200);
        let lista: Value = json_of(&lista.text());
        assert_eq!(lista.as_array().unwrap().len(), 1);

        let atualizada = request
            .put(&format!("/api/maintenance-windows/{}", criada["id"]))
            .json(&json!({
                "siteId": site_id,
                "name": "Manutenção no link",
                "description": null,
                "startsAt": inicio.to_rfc3339(),
                "endsAt": fim.to_rfc3339(),
            }))
            .await;
        assert_eq!(atualizada.status_code(), 200);
        let atualizada = json_of(&atualizada.text());
        assert_eq!(atualizada["name"], "Manutenção no link");

        let invalida = request
            .post("/api/maintenance-windows")
            .json(&json!({
                "siteId": site_id,
                "name": "Inválida",
                "startsAt": fim.to_rfc3339(),
                "endsAt": inicio.to_rfc3339(),
            }))
            .await;
        assert_eq!(invalida.status_code(), 422);

        let sem_alvo = request
            .post("/api/maintenance-windows")
            .json(&json!({
                "name": "Sem alvo",
                "startsAt": inicio.to_rfc3339(),
                "endsAt": fim.to_rfc3339(),
            }))
            .await;
        assert_eq!(sem_alvo.status_code(), 422);

        assert_eq!(
            request
                .delete(&format!("/api/maintenance-windows/{}", criada["id"]))
                .await
                .status_code(),
            204
        );

        let lista = request.get("/api/maintenance-windows").await;
        let lista: Value = json_of(&lista.text());
        assert!(lista.as_array().unwrap().is_empty());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn janela_no_dispositivo_suprime_notificacao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        sem_agrupamento();

        let device = request
            .post("/api/devices")
            .json(&json!({"name": "Switch", "type": "switch", "ipAddress": "127.0.0.1"}))
            .await;
        assert_eq!(device.status_code(), 201);
        let device = json_of(&device.text());
        let device_id = device["id"].as_i64().unwrap();

        let monitor = request
            .post("/api/monitors")
            .json(&json!({
                "deviceId": device_id,
                "name": "TCP fechado",
                "type": "tcp",
                "target": "127.0.0.1",
                "port": 9
            }))
            .await;
        assert_eq!(monitor.status_code(), 201);
        let monitor_id = json_of(&monitor.text())["id"].as_i64().unwrap();

        let _rule_id = so_a_regra_do_teste(
            &request,
            json!({
                "name": "Queda",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "severity": "critical",
                "recoveryWindowSeconds": 0,
            }),
        )
        .await;

        let inicio = Utc::now() - Duration::minutes(5);
        let fim = Utc::now() + Duration::hours(2);
        let janela = request
            .post("/api/maintenance-windows")
            .json(&json!({
                "deviceId": device_id,
                "name": "Manutenção no switch",
                "startsAt": inicio.to_rfc3339(),
                "endsAt": fim.to_rfc3339(),
            }))
            .await;
        assert_eq!(janela.status_code(), 201);

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Down), None)
            .await
            .expect("processar queda");

        let linhas = diario(&ctx).await;
        assert_eq!(linhas.len(), 1);
        assert_eq!(linhas[0].kind, "problem");
        assert_eq!(linhas[0].status, "suppressed");
        assert_eq!(linhas[0].suppress_reason.as_deref(), Some("maintenance"));

        agrupamento_padrao();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn janela_no_site_suprime_notificacao_do_dispositivo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        sem_agrupamento();

        let site = request
            .post("/api/sites")
            .json(&json!({"name": "Filial"}))
            .await;
        assert_eq!(site.status_code(), 201);
        let site_id = json_of(&site.text())["id"].as_i64().unwrap();

        let device = request
            .post("/api/devices")
            .json(&json!({
                "siteId": site_id,
                "name": "Roteador",
                "type": "router",
                "ipAddress": "127.0.0.1"
            }))
            .await;
        assert_eq!(device.status_code(), 201);
        let device_id = json_of(&device.text())["id"].as_i64().unwrap();

        let monitor = request
            .post("/api/monitors")
            .json(&json!({
                "deviceId": device_id,
                "name": "TCP fechado",
                "type": "tcp",
                "target": "127.0.0.1",
                "port": 9
            }))
            .await;
        assert_eq!(monitor.status_code(), 201);
        let monitor_id = json_of(&monitor.text())["id"].as_i64().unwrap();

        let _rule_id = so_a_regra_do_teste(
            &request,
            json!({
                "name": "Queda",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "severity": "critical",
                "recoveryWindowSeconds": 0,
            }),
        )
        .await;

        let inicio = Utc::now() - Duration::minutes(5);
        let fim = Utc::now() + Duration::hours(2);
        let janela = request
            .post("/api/maintenance-windows")
            .json(&json!({
                "siteId": site_id,
                "name": "Manutenção na filial",
                "startsAt": inicio.to_rfc3339(),
                "endsAt": fim.to_rfc3339(),
            }))
            .await;
        assert_eq!(janela.status_code(), 201);

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Down), None)
            .await
            .expect("processar queda");

        let linhas = diario(&ctx).await;
        assert_eq!(linhas.len(), 1);
        assert_eq!(linhas[0].status, "suppressed");
        assert_eq!(linhas[0].suppress_reason.as_deref(), Some("maintenance"));

        agrupamento_padrao();
    })
    .await;
}
