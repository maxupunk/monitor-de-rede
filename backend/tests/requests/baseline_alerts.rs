//! Baseline móvel e alertas por desvio (Fase 3 do roadmap mestre).

use backend::{
    app::App,
    models::{
        _entities::alert_events as alert_events_entity, alert_events, device_interfaces, devices,
        metrics, monitor_results_hourly,
    },
    services::{
        alerts::{
            baseline::{self, MIN_BUCKETS_FOR_BASELINE},
            datasets::monitor_result,
            fields,
        },
        monitoring::contracts::{CheckMetric, CheckResult, MonitorStatus},
    },
};
use chrono::{Duration, Utc};
use loco_rs::{testing::prelude::*, TestServer};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use serial_test::serial;

use super::prepare_data;

fn resultado(latency_ms: f64, packet_loss: f64) -> CheckResult {
    resultado_em(latency_ms, packet_loss, Utc::now())
}

fn resultado_em(latency_ms: f64, packet_loss: f64, now: chrono::DateTime<Utc>) -> CheckResult {
    CheckResult {
        success: true,
        status: MonitorStatus::Up,
        started_at: now,
        finished_at: now,
        duration_ms: 10,
        message: None,
        metrics: vec![
            CheckMetric {
                name: "latency".into(),
                value: latency_ms,
                unit: "ms".into(),
            },
            CheckMetric {
                name: "packet_loss".into(),
                value: packet_loss,
                unit: "%".into(),
            },
        ],
        data: json!({}),
    }
}

async fn monitor_externo(request: &TestServer, policy: serde_json::Value) -> (i64, i64) {
    let monitor = request
        .post("/api/monitors")
        .json(&json!({
            "name": "OpenAI externo",
            "type": "http",
            "configuration": {
                "url": "https://chatgpt.com",
                "method": "HEAD",
                "latencyAlertPolicy": policy,
            },
            "intervalSeconds": 60,
        }))
        .await;
    assert_eq!(monitor.status_code(), 201, "{}", monitor.text());
    let monitor: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
    let monitor_id = monitor["id"].as_i64().unwrap();

    let rule = request
        .post("/api/alert-rules")
        .json(&json!({
            "name": "Latência absoluta legada",
            "condition": { "field": "latencyMs", "operator": "gt", "value": 200 },
            "severity": "warning",
            "durationSeconds": 300,
            "monitorId": monitor_id,
        }))
        .await;
    assert_eq!(rule.status_code(), 201, "{}", rule.text());
    let rule: serde_json::Value = serde_json::from_str(&rule.text()).unwrap();
    (monitor_id, rule["id"].as_i64().unwrap())
}

fn limpar_cache() {
    backend::services::alerts::baseline::clear_cache();
}

async fn buckets_de_baseline(ctx: &loco_rs::app::AppContext, monitor_id: i64) {
    let base = Utc::now() - Duration::hours((MIN_BUCKETS_FOR_BASELINE + 1) as i64);
    for hour in 0..MIN_BUCKETS_FOR_BASELINE {
        let bucket = base + Duration::hours(hour as i64);
        monitor_results_hourly::ActiveModel {
            monitor_id: Set(monitor_id),
            bucket: Set(bucket.into()),
            total_checks: Set(60),
            up_checks: Set(57),
            down_checks: Set(3),
            unknown_checks: Set(0),
            avg_latency_ms: Set(Some(20.0)),
            min_latency_ms: Set(Some(18.0)),
            max_latency_ms: Set(Some(22.0)),
            first_started_at: Set(bucket.into()),
            last_finished_at: Set(bucket.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("bucket de baseline");
    }
}

async fn eventos_por_regra(ctx: &loco_rs::app::AppContext, rule_id: i64) -> usize {
    alert_events::Entity::find()
        .filter(alert_events_entity::Column::AlertRuleId.eq(rule_id))
        .all(&ctx.db)
        .await
        .expect("eventos da regra")
        .len()
}

#[tokio::test]
#[serial]
async fn baseline_e_calculada_a_partir_dos_buckets_horarios() {
    limpar_cache();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = request
            .post("/api/monitors")
            .json(&json!({
                "name": "Baseline ping",
                "type": "ping",
                "target": "127.0.0.1",
            }))
            .await;
        assert_eq!(monitor.status_code(), 201);
        let monitor: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
        let monitor_id = monitor["id"].as_i64().unwrap();

        buckets_de_baseline(&ctx, monitor_id).await;

        let baseline = baseline::for_monitor(&ctx.db, monitor_id).await.unwrap();
        assert!(
            baseline
                .latency_baseline_ms
                .is_some_and(|value| (value - 20.0).abs() < 0.001),
            "baseline de latência deveria ser 20 ms"
        );
        assert!(
            baseline
                .uptime_baseline_percent
                .is_some_and(|value| (value - 95.0).abs() < 0.001),
            "baseline de uptime deveria ser 95%"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn regra_de_desvio_de_latencia_dispara_quando_acima_da_baseline() {
    limpar_cache();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = request
            .post("/api/monitors")
            .json(&json!({
                "name": "Baseline ping",
                "type": "ping",
                "target": "127.0.0.1",
            }))
            .await;
        let monitor: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
        let monitor_id = monitor["id"].as_i64().unwrap();

        buckets_de_baseline(&ctx, monitor_id).await;

        let rule = request
            .post("/api/alert-rules")
            .json(&json!({
                "name": "Latência acima da baseline",
                "condition": { "field": "latencyDeviationPercent", "operator": "gt", "value": 50 },
                "severity": "warning",
                "durationSeconds": 0,
                "monitorId": monitor_id,
            }))
            .await;
        assert_eq!(rule.status_code(), 201);
        let rule: serde_json::Value = serde_json::from_str(&rule.text()).unwrap();
        let rule_id = rule["id"].as_i64().unwrap();

        // Latência de 40 ms: 100% acima da baseline de 20 ms.
        let dataset = monitor_result::build(
            "ping",
            &resultado(40.0, 0.0),
            &baseline::for_monitor(&ctx.db, monitor_id).await.unwrap(),
        );
        assert_eq!(dataset[fields::LATENCY_BASELINE_MS], json!(20.0));
        assert!(
            dataset[fields::LATENCY_DEVIATION_PERCENT].as_f64().unwrap() > 50.0,
            "desvio deveria ser maior que 50%"
        );

        // Processa o resultado: a regra deve disparar imediatamente (durationSeconds = 0).
        backend::services::monitoring::result_processor::process_result(
            &ctx,
            monitor_id,
            &resultado(40.0, 0.0),
            None,
        )
        .await
        .unwrap();

        assert_eq!(
            eventos_por_regra(&ctx, rule_id).await,
            1,
            "regra de desvio de latência deveria disparar"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn regra_de_desvio_de_latencia_nao_dispara_abaixo_do_limiar() {
    limpar_cache();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = request
            .post("/api/monitors")
            .json(&json!({
                "name": "Baseline ping estável",
                "type": "ping",
                "target": "127.0.0.1",
            }))
            .await;
        let monitor: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
        let monitor_id = monitor["id"].as_i64().unwrap();

        buckets_de_baseline(&ctx, monitor_id).await;

        let rule = request
            .post("/api/alert-rules")
            .json(&json!({
                "name": "Latência acima da baseline",
                "condition": { "field": "latencyDeviationPercent", "operator": "gt", "value": 50 },
                "severity": "warning",
                "durationSeconds": 0,
                "monitorId": monitor_id,
            }))
            .await;
        let rule: serde_json::Value = serde_json::from_str(&rule.text()).unwrap();
        let rule_id = rule["id"].as_i64().unwrap();

        // Latência de 25 ms: 25% acima da baseline, abaixo do limiar de 50%.
        backend::services::monitoring::result_processor::process_result(
            &ctx,
            monitor_id,
            &resultado(25.0, 0.0),
            None,
        )
        .await
        .unwrap();

        assert_eq!(
            eventos_por_regra(&ctx, rule_id).await,
            0,
            "regra não deveria disparar com desvio de apenas 25%"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn regra_de_anomalia_estatistica_por_z_score_dispara_acima_de_3_sigmas() {
    limpar_cache();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = request
            .post("/api/monitors")
            .json(&json!({
                "name": "Z-Score Anomaly Ping",
                "type": "ping",
                "target": "127.0.0.1",
            }))
            .await;
        let monitor: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
        let monitor_id = monitor["id"].as_i64().unwrap();

        buckets_de_baseline(&ctx, monitor_id).await;

        let rule = request
            .post("/api/alert-rules")
            .json(&json!({
                "name": "Anomalia de latência (3σ)",
                "condition": { "field": "latencyZScore", "operator": "gte", "value": 3.0 },
                "severity": "warning",
                "durationSeconds": 0,
                "monitorId": monitor_id,
            }))
            .await;
        assert_eq!(rule.status_code(), 201);
        let rule: serde_json::Value = serde_json::from_str(&rule.text()).unwrap();
        let rule_id = rule["id"].as_i64().unwrap();

        // Latência de 30 ms com baseline de 20 ms -> Z-Score = (30 - 20) / 0.5 = 20.0 >= 3.0
        backend::services::monitoring::result_processor::process_result(
            &ctx,
            monitor_id,
            &resultado(30.0, 0.0),
            None,
        )
        .await
        .unwrap();

        assert_eq!(
            eventos_por_regra(&ctx, rule_id).await,
            1,
            "regra de anomalia estatística deveria disparar com Z-Score elevado"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn endpoint_baseline_devolve_estatisticas_completas() {
    limpar_cache();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor = request
            .post("/api/monitors")
            .json(&json!({
                "name": "Baseline API Ping",
                "type": "ping",
                "target": "127.0.0.1",
            }))
            .await;
        let monitor: serde_json::Value = serde_json::from_str(&monitor.text()).unwrap();
        let monitor_id = monitor["id"].as_i64().unwrap();

        buckets_de_baseline(&ctx, monitor_id).await;

        let res = request
            .get(&format!("/api/monitors/{monitor_id}/baseline"))
            .await;
        assert_eq!(res.status_code(), 200);
        let payload: serde_json::Value = serde_json::from_str(&res.text()).unwrap();

        assert_eq!(payload["monitorId"], monitor_id);
        assert_eq!(payload["hasSufficientData"], true);
        assert_eq!(payload["baseline"]["latencyBaselineMs"], 20.0);
        assert!(payload["baseline"]["latencyUpperBandMs"].is_number());
        assert_eq!(payload["baseline"]["sampleCount"], MIN_BUCKETS_FOR_BASELINE);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn alvo_externo_com_latencia_regional_alta_nao_dispara_regra_absoluta() {
    limpar_cache();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let (monitor_id, rule_id) = monitor_externo(&request, json!({ "mode": "auto" })).await;
        buckets_de_baseline_com_latencia(&ctx, monitor_id, 220.0).await;
        limpar_cache();

        backend::services::monitoring::result_processor::process_result(
            &ctx,
            monitor_id,
            &resultado(225.0, 0.0),
            None,
        )
        .await
        .unwrap();

        assert_eq!(eventos_por_regra(&ctx, rule_id).await, 0);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn alvo_externo_exige_tres_desvios_consecutivos_antes_do_alerta() {
    limpar_cache();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let (monitor_id, rule_id) = monitor_externo(
            &request,
            json!({
                "mode": "adaptive",
                "deviationPercent": 50,
                "consecutiveChecks": 3,
                "suppressOnSaturation": false,
            }),
        )
        .await;
        buckets_de_baseline_com_latencia(&ctx, monitor_id, 200.0).await;
        limpar_cache();
        let start = Utc::now() - Duration::seconds(10);

        for occurrence in 0..3 {
            backend::services::monitoring::result_processor::process_result(
                &ctx,
                monitor_id,
                &resultado_em(340.0, 0.0, start + Duration::seconds(occurrence)),
                None,
            )
            .await
            .unwrap();
            assert_eq!(
                eventos_por_regra(&ctx, rule_id).await,
                if occurrence == 2 { 1 } else { 0 },
                "o alerta só pode abrir na terceira confirmação"
            );
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn saturacao_da_wan_quebra_a_sequencia_de_alerta_de_latencia() {
    limpar_cache();
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let device = request
            .post("/api/devices")
            .json(&json!({
                "name": "Gateway",
                "type": "router",
                "ipAddress": "192.168.50.1",
                "snmpEnabled": true,
            }))
            .await;
        assert_eq!(device.status_code(), 201, "{}", device.text());
        let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
        let device_id = device["id"].as_i64().unwrap();
        let now = Utc::now();
        let interface = device_interfaces::ActiveModel {
            device_id: Set(device_id),
            snmp_index: Set(Some(1)),
            name: Set("wan1".into()),
            speed: Set(Some(1_000_000_000)),
            admin_status: Set(Some("up".into())),
            oper_status: Set(Some("up".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        let device_model = devices::Entity::find_by_id(device_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let mut active: devices::ActiveModel = device_model.into();
        active.link_interface_id = Set(Some(interface.id));
        active.link_interface_name = Set(Some(interface.name.clone()));
        active.update(&ctx.db).await.unwrap();

        let (monitor_id, rule_id) = monitor_externo(
            &request,
            json!({
                "mode": "adaptive",
                "deviationPercent": 50,
                "consecutiveChecks": 3,
                "sourceDeviceId": device_id,
                "downloadCapacityBps": 100_000_000,
                "uploadCapacityBps": 20_000_000,
                "saturationThresholdPercent": 80,
                "suppressOnSaturation": true,
            }),
        )
        .await;
        buckets_de_baseline_com_latencia(&ctx, monitor_id, 200.0).await;
        limpar_cache();

        for occurrence in 0..3 {
            let observed_at = now + Duration::seconds(occurrence);
            for (name, value) in [("inBps", 90_000_000.0), ("outBps", 2_000_000.0)] {
                metrics::ActiveModel {
                    device_id: Set(device_id),
                    interface_id: Set(Some(interface.id)),
                    monitor_id: Set(None),
                    name: Set(name.into()),
                    value: Set(value),
                    unit: Set("bps".into()),
                    recorded_at: Set(observed_at.into()),
                    ..Default::default()
                }
                .insert(&ctx.db)
                .await
                .unwrap();
            }
            backend::services::monitoring::result_processor::process_result(
                &ctx,
                monitor_id,
                &resultado_em(340.0, 0.0, observed_at),
                None,
            )
            .await
            .unwrap();
        }

        assert_eq!(eventos_por_regra(&ctx, rule_id).await, 0);
        let baseline = request
            .get(&format!("/api/monitors/{monitor_id}/baseline"))
            .await;
        let payload: serde_json::Value = serde_json::from_str(&baseline.text()).unwrap();
        assert_eq!(payload["adaptiveLatency"]["reason"], "link_saturated");
        assert_eq!(payload["adaptiveLatency"]["linkSaturated"], true);
        assert!(payload["adaptiveLatency"]["linkUtilizationPercent"]
            .as_f64()
            .is_some_and(|value| value >= 90.0));
    })
    .await;
}

async fn buckets_de_baseline_com_latencia(
    ctx: &loco_rs::app::AppContext,
    monitor_id: i64,
    latency_ms: f64,
) {
    let base = Utc::now() - Duration::hours((MIN_BUCKETS_FOR_BASELINE + 1) as i64);
    for hour in 0..MIN_BUCKETS_FOR_BASELINE {
        let bucket = base + Duration::hours(hour as i64);
        monitor_results_hourly::ActiveModel {
            monitor_id: Set(monitor_id),
            bucket: Set(bucket.into()),
            total_checks: Set(60),
            up_checks: Set(60),
            down_checks: Set(0),
            unknown_checks: Set(0),
            avg_latency_ms: Set(Some(latency_ms)),
            min_latency_ms: Set(Some(latency_ms)),
            max_latency_ms: Set(Some(latency_ms)),
            first_started_at: Set(bucket.into()),
            last_finished_at: Set(bucket.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
    }
}
