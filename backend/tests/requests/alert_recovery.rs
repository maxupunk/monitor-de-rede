//! Fase 1 do roadmap de alertas inteligentes: histerese de resolução.
//!
//! A janela de recuperação depende de tempo real entre checagens, então a
//! máquina de estados é exercitada à exaustão nos testes unitários
//! (`services/alerts/state_machine.rs`); aqui o que se valida é o fluxo de
//! ponta a ponta: dispara → checagem ok → `recovering` → recaída → estável além
//! da janela (com `lastProblemAt` no passado) → `resolved`.

use backend::{
    app::App,
    models::{
        _entities::{alert_events as alert_events_entity, event_outbox as event_outbox_entity},
        alert_events, event_outbox,
    },
    services::monitoring::{
        contracts::{CheckResult, MonitorStatus},
        result_processor::process_result,
    },
};
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("resposta JSON")
}

/// Resultado sintético: a checagem real de rede só é usada no disparo (porta 9
/// fechada em loopback = down determinístico); a sequência de sobe/desce do
/// episódio é fabricada aqui para o teste não depender de tempo real.
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

#[tokio::test]
#[serial]
async fn janela_de_recuperacao_segura_o_alerta_ate_a_estabilidade_se_confirmar() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // Regra com janela de 5 minutos. O `device_offline` do catálogo (janela
        // 0, provisionado no boot) serve de contraste: ele resolve na primeira
        // checagem ok, como sempre.
        let regra = json_of(
            &request
                .post("/api/alert-rules")
                .json(&json!({
                    "name": "Queda com estabilização",
                    "condition": { "field": "status", "operator": "eq", "value": "down" },
                    "severity": "critical",
                    "recoveryWindowSeconds": 300
                }))
                .await
                .text(),
        );
        assert_eq!(regra["recoveryWindowSeconds"], 300);
        let rule_id = regra["id"].as_i64().unwrap();

        // Porta 9 fechada em loopback: o TCP falha de forma determinística.
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

        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1);
        let evento = &eventos[0];
        assert_eq!(evento.status, "active");
        assert_eq!(
            evento.data.as_ref().unwrap()["recurrenceCount"],
            json!(0),
            "o episódio nasce com o contador zerado"
        );
        assert!(evento.data.as_ref().unwrap()["lastProblemAt"].is_string());

        // Checagem ok: a regra com janela NÃO resolve — entra em recovering.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar subida");
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1, "recuperação não abre evento novo");
        assert_eq!(eventos[0].status, "recovering");
        assert!(eventos[0].resolved_at.is_none());
        // E o episódio trafega na API com os metadados da janela.
        let via_api = json_of(&request.get("/api/alerts").await.text());
        let serializado = via_api
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["alertRuleId"] == rule_id)
            .expect("evento da regra na Central");
        assert_eq!(serializado["status"], "recovering");
        assert!(serializado["data"]["lastProblemAt"].is_string());

        // Recaída: volta a active, o contador sobe e nenhum evento novo nasce.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Down), None)
            .await
            .expect("processar recaída");
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1, "recaída não abre evento novo");
        assert_eq!(eventos[0].status, "active");
        assert_eq!(
            eventos[0].data.as_ref().unwrap()["recurrenceCount"],
            json!(1)
        );

        // A recaída avisou a tela via alert:updated (a notificação, não: ela é
        // reservada ao disparo e à resolução final).
        let atualizacoes = event_outbox::Entity::find()
            .filter(event_outbox_entity::Column::Type.eq("alert:updated"))
            .all(&ctx.db)
            .await
            .expect("outbox");
        assert!(
            atualizacoes.len() >= 2,
            "entrada em recovering + recaída publicam alert:updated"
        );

        // Volta ao normal: recovering de novo.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar nova subida");
        let evento = &eventos_da_regra(&ctx, rule_id).await[0];
        assert_eq!(evento.status, "recovering");

        // Janela já vencida: simula `lastProblemAt` 400 s no passado (a janela
        // é de 300 s) em vez de esperar tempo real.
        let mut data = evento.data.as_ref().unwrap().as_object().unwrap().clone();
        data.insert(
            "lastProblemAt".into(),
            json!((Utc::now() - Duration::seconds(400)).to_rfc3339()),
        );
        let mut ativo: alert_events::ActiveModel = evento.clone().into();
        ativo.data = Set(Some(Value::Object(data)));
        ativo.update(&ctx.db).await.expect("carimbo no passado");

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar estabilidade além da janela");
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].status, "resolved");
        assert!(eventos[0].resolved_at.is_some());
        assert_eq!(
            eventos[0].data.as_ref().unwrap()["recurrenceCount"],
            json!(1)
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn regra_sem_janela_continua_resolvendo_na_primeira_checagem_ok() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let regra = json_of(
            &request
                .post("/api/alert-rules")
                .json(&json!({
                    "name": "Queda sem janela",
                    "condition": { "field": "status", "operator": "eq", "value": "down" }
                }))
                .await
                .text(),
        );
        assert_eq!(
            regra["recoveryWindowSeconds"], 0,
            "default é resolver na hora"
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
        request
            .post(&format!("/api/monitors/{monitor_id}/run"))
            .await;
        assert_eq!(eventos_da_regra(&ctx, rule_id).await[0].status, "active");

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar subida");
        assert_eq!(eventos_da_regra(&ctx, rule_id).await[0].status, "resolved");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn janela_negativa_e_recusada_e_o_campo_sobrevive_ao_put_parcial() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let base = json!({
            "name": "Regra validada",
            "condition": { "field": "status", "operator": "eq", "value": "down" }
        });

        let negativa = request
            .post("/api/alert-rules")
            .json(&json!({
                "name": "Inválida",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "recoveryWindowSeconds": -1
            }))
            .await;
        assert_eq!(negativa.status_code(), 422);

        let mut payload = base.clone();
        payload["recoveryWindowSeconds"] = json!(120);
        let criada = json_of(&request.post("/api/alert-rules").json(&payload).await.text());
        assert_eq!(criada["recoveryWindowSeconds"], 120);
        let id = criada["id"].as_i64().unwrap();

        // PUT parcial manda só `enabled`: a janela não pode zerar.
        let atualizada = json_of(
            &request
                .put(&format!("/api/alert-rules/{id}"))
                .json(&json!({ "enabled": false }))
                .await
                .text(),
        );
        assert_eq!(atualizada["recoveryWindowSeconds"], 120);
        assert_eq!(atualizada["enabled"], false);

        let negativa = request
            .put(&format!("/api/alert-rules/{id}"))
            .json(&json!({ "recoveryWindowSeconds": -5 }))
            .await;
        assert_eq!(negativa.status_code(), 422);

        let zerada = json_of(
            &request
                .put(&format!("/api/alert-rules/{id}"))
                .json(&json!({ "recoveryWindowSeconds": 0 }))
                .await
                .text(),
        );
        assert_eq!(zerada["recoveryWindowSeconds"], 0);
        assert_eq!(zerada["name"], "Regra validada");
    })
    .await;
}
