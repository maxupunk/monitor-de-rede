//! Fase 5 do roadmap de alertas inteligentes: robustez do motor.
//!
//! Dois defeitos antigos, os dois invisíveis em teste unitário porque vivem no
//! encontro entre memória de processo e banco:
//!
//! - a histerese de **disparo** zerava a cada restart do scheduler, e agora é
//!   reconstruída a partir de `monitor_results`;
//! - `monitors.retry_count` era gravado e nunca lido, e agora confirma a queda
//!   antes de declarar `down`.

use backend::{
    app::App,
    models::{_entities::alert_events as alert_events_entity, alert_events, monitor_results},
    services::{
        alerts::hysteresis,
        monitoring::{
            contracts::{CheckResult, MonitorStatus},
            result_processor::process_result,
        },
    },
    tasks::scheduler_run::run_cycle,
};
use chrono::{Duration, Utc};
use loco_rs::{testing::prelude::*, TestServer};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

/// Intervalo dos monitores destes testes. Fixo de propósito: a reconstrução
/// mede o intervalo entre observações em múltiplos dele.
const INTERVALO: i64 = 60;

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

/// Uma regra só, com tolerância de 5 min: o catálogo básico casaria com o mesmo
/// `status: down` e abriria episódios que o teste não pediu.
async fn regra_com_tolerancia(request: &TestServer) -> i64 {
    let existentes = json_of(&request.get("/api/alert-rules").await.text());
    for regra in existentes.as_array().expect("lista de regras") {
        let id = regra["id"].as_i64().unwrap();
        request.delete(&format!("/api/alert-rules/{id}")).await;
    }
    let criada = json_of(
        &request
            .post("/api/alert-rules")
            .json(&json!({
                "name": "Queda sustentada por 5 min",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "severity": "critical",
                "durationSeconds": 300
            }))
            .await
            .text(),
    );
    criada["id"].as_i64().expect("regra criada")
}

async fn monitor_quebrado(request: &TestServer, retry_count: i32) -> i64 {
    let monitor = json_of(
        &request
            .post("/api/monitors")
            .json(&json!({
                "name": "TCP fechado",
                "type": "tcp",
                "target": "127.0.0.1",
                "port": 9,
                "intervalSeconds": INTERVALO,
                "retryCount": retry_count
            }))
            .await
            .text(),
    );
    monitor["id"].as_i64().expect("monitor criado")
}

/// Escreve o histórico que um processo anterior teria gravado.
///
/// Do mais velho para o mais novo, como o scheduler grava de verdade.
async fn historico_de_quedas(ctx: &loco_rs::app::AppContext, monitor_id: i64, amostras: usize) {
    for passo in (1..=amostras).rev() {
        let at = Utc::now() - Duration::seconds(INTERVALO * passo as i64);
        monitor_results::ActiveModel {
            monitor_id: Set(monitor_id),
            status: Set("down".into()),
            started_at: Set(at.into()),
            finished_at: Set(at.into()),
            duration_ms: Set(5),
            latency_ms: Set(None),
            message: Set(None),
            data: Set(Some(json!({}))),
            created_at: Set(at.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("observação histórica");
    }
}

async fn eventos_da_regra(ctx: &loco_rs::app::AppContext, rule_id: i64) -> usize {
    alert_events::Entity::find()
        .filter(alert_events_entity::Column::AlertRuleId.eq(rule_id))
        .all(&ctx.db)
        .await
        .expect("eventos da regra")
        .len()
}

#[tokio::test]
#[serial]
async fn a_tolerancia_de_disparo_sobrevive_ao_restart_do_processo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let rule_id = regra_com_tolerancia(&request).await;
        let monitor_id = monitor_quebrado(&request, 0).await;
        let scope_key = format!("monitor:{monitor_id}");
        // O processo "acabou de subir": nenhuma contagem em memória.
        hysteresis::forget(rule_id, &scope_key);

        // Seis observações de 1 em 1 minuto: o alvo está caído há 6 min, mais
        // que a tolerância de 5. Antes da Fase 5 isso não valia nada — a
        // contagem começava do zero e o alerta só sairia 5 min depois.
        historico_de_quedas(&ctx, monitor_id, 6).await;

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Down), None)
            .await
            .expect("processar queda");
        assert_eq!(
            eventos_da_regra(&ctx, rule_id).await,
            1,
            "o histórico provava 6 min de queda contínua: o alerta devia disparar já"
        );
        hysteresis::forget(rule_id, &scope_key);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sem_historico_a_contagem_recomeca_e_a_primeira_queda_nao_dispara() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let rule_id = regra_com_tolerancia(&request).await;
        let monitor_id = monitor_quebrado(&request, 0).await;
        let scope_key = format!("monitor:{monitor_id}");
        hysteresis::forget(rule_id, &scope_key);

        // Matriz de paridade #24: a reconstrução não afrouxa a tolerância —
        // sem passado que a prove, o relógio começa agora.
        for _ in 0..3 {
            process_result(&ctx, monitor_id, &resultado(MonitorStatus::Down), None)
                .await
                .expect("processar queda");
        }
        assert_eq!(
            eventos_da_regra(&ctx, rule_id).await,
            0,
            "três checagens seguidas não somam cinco minutos"
        );
        hysteresis::forget(rule_id, &scope_key);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_queda_e_reconfirmada_antes_de_ser_gravada() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor_id = monitor_quebrado(&request, 2).await;
        run_cycle(&ctx).await.expect("ciclo do scheduler");

        let ultimo = monitor_results::Entity::find()
            .filter(monitor_results::Column::MonitorId.eq(monitor_id))
            .order_by_desc(monitor_results::Column::Id)
            .one(&ctx.db)
            .await
            .expect("consulta")
            .expect("observação gravada");
        assert_eq!(ultimo.status, "down");
        assert_eq!(
            ultimo.data.as_ref().and_then(|data| data.get("attempts")),
            Some(&json!(3)),
            "retryCount 2 = 1 tentativa + 2 reconfirmações"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sem_retry_configurado_a_checagem_continua_sendo_uma_so() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let monitor_id = monitor_quebrado(&request, 0).await;
        run_cycle(&ctx).await.expect("ciclo do scheduler");

        let ultimo = monitor_results::Entity::find()
            .filter(monitor_results::Column::MonitorId.eq(monitor_id))
            .order_by_desc(monitor_results::Column::Id)
            .one(&ctx.db)
            .await
            .expect("consulta")
            .expect("observação gravada");
        assert_eq!(ultimo.status, "down");
        assert!(
            ultimo
                .data
                .as_ref()
                .and_then(|data| data.get("attempts"))
                .is_none(),
            "sem repetição não há o que registrar"
        );
    })
    .await;
}
