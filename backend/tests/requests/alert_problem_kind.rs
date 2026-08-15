//! Fase 2 do roadmap de alertas inteligentes: classificação do problema
//! (`data.problemKind`) e `warning` contando como problema para a janela de
//! recuperação.
//!
//! Como em `alert_recovery.rs`, a checagem real de rede só aparece no disparo
//! (porta 9 fechada em loopback = down determinístico); o resto do episódio é
//! fabricado com resultados sintéticos para o teste não depender de tempo real.

use backend::{
    app::App,
    models::_entities::alert_events as alert_events_entity,
    models::alert_events,
    services::monitoring::{
        contracts::{CheckMetric, CheckResult, MonitorStatus},
        result_processor::process_result,
    },
};
use chrono::{Duration, Utc};
use loco_rs::{testing::prelude::*, TestServer};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

fn resultado(status: MonitorStatus) -> CheckResult {
    resultado_com_metricas(status, vec![])
}

fn resultado_com_metricas(status: MonitorStatus, metrics: Vec<CheckMetric>) -> CheckResult {
    let now = Utc::now();
    CheckResult {
        success: status == MonitorStatus::Up,
        status,
        started_at: now,
        finished_at: now,
        duration_ms: 5,
        message: None,
        metrics,
        data: json!({}),
    }
}

/// Os eventos da regra informada, do mais novo para o mais velho.
async fn eventos_da_regra(
    ctx: &loco_rs::app::AppContext,
    rule_id: i64,
) -> Vec<alert_events::Model> {
    alert_events::Entity::find()
        .filter(alert_events_entity::Column::AlertRuleId.eq(rule_id))
        .all(&ctx.db)
        .await
        .expect("eventos da regra")
}

/// Cria regra + monitor TCP quebrado e dispara o alerta. Devolve (regra, monitor).
async fn cenario_com_alerta_aberto(
    request: &TestServer,
    ctx: &loco_rs::app::AppContext,
    recovery_window_seconds: i32,
) -> (i64, i64) {
    let regra = json_of(
        &request
            .post("/api/alert-rules")
            .json(&json!({
                "name": "Queda com estabilização",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "severity": "critical",
                "recoveryWindowSeconds": recovery_window_seconds
            }))
            .await
            .text(),
    );
    let rule_id = regra["id"].as_i64().unwrap();

    let monitor = json_of(
        &request
            .post("/api/monitors")
            .json(&json!({
                "name": "TCP fechado", "type": "tcp",
                "target": "127.0.0.1", "port": 9
            }))
            .await
            .text(),
    );
    let monitor_id = monitor["id"].as_i64().unwrap();
    assert_eq!(
        request
            .post(&format!("/api/monitors/{monitor_id}/run"))
            .await
            .status_code(),
        200
    );
    assert_eq!(eventos_da_regra(ctx, rule_id).await[0].status, "active");
    (rule_id, monitor_id)
}

#[tokio::test]
#[serial]
async fn warning_durante_a_estabilizacao_conta_como_recaida() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let (rule_id, monitor_id) = cenario_com_alerta_aberto(&request, &ctx, 300).await;

        // Checagem ok: entra em recovering.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar subida");
        assert_eq!(
            eventos_da_regra(&ctx, rule_id).await[0].status,
            "recovering"
        );

        // Warning (perda parcial, DNS parcial): não bate a condição da regra,
        // mas conta como problema — recaída sem notificação e sem evento novo.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Warning), None)
            .await
            .expect("processar warning");
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1, "warning não abre evento novo");
        assert_eq!(
            eventos[0].status, "active",
            "warning em recovering é recaída"
        );
        assert_eq!(
            eventos[0].data.as_ref().unwrap()["recurrenceCount"],
            json!(1)
        );

        // De volta ao normal: recovering outra vez e, com a janela vencida,
        // resolve — o episódio inteiro coube num evento só.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar nova subida");
        let evento = eventos_da_regra(&ctx, rule_id).await.remove(0);
        assert_eq!(evento.status, "recovering");

        let mut data = evento.data.as_ref().unwrap().as_object().unwrap().clone();
        data.insert(
            "lastProblemAt".into(),
            json!((Utc::now() - Duration::seconds(400)).to_rfc3339()),
        );
        let mut ativo: alert_events::ActiveModel = evento.into();
        ativo.data = Set(Some(Value::Object(data)));
        ativo.update(&ctx.db).await.expect("carimbo no passado");

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar estabilidade além da janela");
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].status, "resolved");
        assert_eq!(
            eventos[0].data.as_ref().unwrap()["recurrenceCount"],
            json!(1)
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn warning_com_a_janela_vencida_bloqueia_a_resolucao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let (rule_id, monitor_id) = cenario_com_alerta_aberto(&request, &ctx, 300).await;

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar subida");
        let evento = eventos_da_regra(&ctx, rule_id).await.remove(0);
        assert_eq!(evento.status, "recovering");

        // A janela já venceu (último problema há 400 s, janela de 300 s): uma
        // checagem ok resolveria. Em vez dela chega um warning — o link segue
        // doente, então não resolve: é recaída e o relógio reinicia.
        let mut data = evento.data.as_ref().unwrap().as_object().unwrap().clone();
        data.insert(
            "lastProblemAt".into(),
            json!((Utc::now() - Duration::seconds(400)).to_rfc3339()),
        );
        let mut ativo: alert_events::ActiveModel = evento.into();
        ativo.data = Set(Some(Value::Object(data)));
        ativo.update(&ctx.db).await.expect("carimbo no passado");

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Warning), None)
            .await
            .expect("processar warning com janela vencida");
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1);
        assert_ne!(
            eventos[0].status, "resolved",
            "warning bloqueia a resolução mesmo com a janela vencida"
        );
        assert_eq!(eventos[0].status, "active");
        assert!(eventos[0].resolved_at.is_none());
        assert_eq!(
            eventos[0].data.as_ref().unwrap()["recurrenceCount"],
            json!(1)
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn problem_kind_e_gravado_no_disparo_e_exposto_na_serializacao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // Regra de perda de pacotes sem tolerância: um warning com 50% de
        // perda dispara na hora.
        let regra = json_of(
            &request
                .post("/api/alert-rules")
                .json(&json!({
                    "name": "Perda de pacotes",
                    "condition": { "field": "packetLoss", "operator": "gt", "value": 10 },
                    "severity": "warning"
                }))
                .await
                .text(),
        );
        let rule_id = regra["id"].as_i64().unwrap();

        let monitor = json_of(
            &request
                .post("/api/monitors")
                .json(&json!({
                    "name": "Ping sintético", "type": "ping", "target": "127.0.0.1"
                }))
                .await
                .text(),
        );
        let monitor_id = monitor["id"].as_i64().unwrap();

        let aviso = resultado_com_metricas(
            MonitorStatus::Warning,
            vec![CheckMetric {
                name: "packet_loss".into(),
                value: 50.0,
                unit: "%".into(),
            }],
        );
        process_result(&ctx, monitor_id, &aviso, None)
            .await
            .expect("processar perda parcial");

        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1, "a regra de perda disparou");
        assert_eq!(eventos[0].status, "active");
        assert_eq!(
            eventos[0].data.as_ref().unwrap()["problemKind"],
            json!("packet_loss"),
            "a condição sobre packetLoss classifica o episódio"
        );

        // E o tipo trafega na serialização consumida pela Central de Alertas.
        let via_api = json_of(&request.get("/api/alerts").await.text());
        let serializado = via_api
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["alertRuleId"] == rule_id)
            .expect("evento da regra na Central");
        assert_eq!(serializado["data"]["problemKind"], json!("packet_loss"));
        assert_eq!(serializado["status"], "active");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn warning_sem_regra_batendo_nao_dispara_alerta_novo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // Regra exigente: 50% de perda não bate "maior que 90%".
        let regra = json_of(
            &request
                .post("/api/alert-rules")
                .json(&json!({
                    "name": "Perda severa",
                    "condition": { "field": "packetLoss", "operator": "gt", "value": 90 },
                    "severity": "critical"
                }))
                .await
                .text(),
        );
        let rule_id = regra["id"].as_i64().unwrap();

        let monitor = json_of(
            &request
                .post("/api/monitors")
                .json(&json!({
                    "name": "Ping sintético", "type": "ping", "target": "127.0.0.1"
                }))
                .await
                .text(),
        );
        let monitor_id = monitor["id"].as_i64().unwrap();

        let aviso = resultado_com_metricas(
            MonitorStatus::Warning,
            vec![CheckMetric {
                name: "packet_loss".into(),
                value: 50.0,
                unit: "%".into(),
            }],
        );
        process_result(&ctx, monitor_id, &aviso, None)
            .await
            .expect("processar warning sem regra batendo");

        assert!(
            eventos_da_regra(&ctx, rule_id).await.is_empty(),
            "warning respeita o evaluator: condição que não bateu não dispara"
        );
    })
    .await;
}
