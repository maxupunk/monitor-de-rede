//! Baseline móvel e alertas por desvio (Fase 3 do roadmap mestre).

use backend::{
    app::App,
    models::{
        _entities::alert_events as alert_events_entity, alert_events, monitor_results_hourly,
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
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use serial_test::serial;

use super::prepare_data;

fn resultado(latency_ms: f64, packet_loss: f64) -> CheckResult {
    let now = Utc::now();
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
