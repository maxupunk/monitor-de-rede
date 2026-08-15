//! Fase 3 do roadmap de alertas inteligentes: detecção de flapping.
//!
//! O episódio inteiro é conduzido com resultados sintéticos (`process_result`),
//! como em `alert_problem_kind.rs`: o alvo real só aparece no disparo inicial
//! (porta 9 fechada em loopback = down determinístico). É o que torna possível
//! encenar "caiu e voltou N vezes" sem depender de tempo real.

use backend::{
    app::App,
    models::_entities::alert_events as alert_events_entity,
    models::alert_events,
    services::monitoring::{
        contracts::{CheckResult, MonitorStatus},
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

/// Regra com estabilização de 5 min e flapping em 3 recaídas por 15 min, mais
/// um monitor TCP quebrado já com o alerta aberto. Devolve (regra, monitor).
async fn cenario(request: &TestServer, ctx: &loco_rs::app::AppContext) -> (i64, i64) {
    let regra = json_of(
        &request
            .post("/api/alert-rules")
            .json(&json!({
                "name": "Queda com detecção de oscilação",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "severity": "critical",
                "recoveryWindowSeconds": 300,
                "flapThreshold": 3,
                "flapWindowSeconds": 900
            }))
            .await
            .text(),
    );
    assert_eq!(regra["flapThreshold"], 3);
    assert_eq!(regra["flapWindowSeconds"], 900);
    let rule_id = regra["id"].as_i64().unwrap();

    let monitor = json_of(
        &request
            .post("/api/monitors")
            .json(&json!({
                "name": "TCP fechado", "type": "tcp", "target": "127.0.0.1", "port": 9
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

/// Uma oscilação completa: volta ao normal e cai de novo.
async fn oscilar(ctx: &loco_rs::app::AppContext, monitor_id: i64) {
    process_result(ctx, monitor_id, &resultado(MonitorStatus::Up), None)
        .await
        .expect("processar subida");
    process_result(ctx, monitor_id, &resultado(MonitorStatus::Down), None)
        .await
        .expect("processar queda");
}

#[tokio::test]
#[serial]
async fn tres_recaidas_na_janela_declaram_o_alvo_oscilando() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let (rule_id, monitor_id) = cenario(&request, &ctx).await;

        // Duas recaídas: abaixo do limiar, o episódio segue como recaída comum.
        oscilar(&ctx, monitor_id).await;
        oscilar(&ctx, monitor_id).await;
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1, "recaída não abre evento novo");
        assert_eq!(eventos[0].status, "active");
        assert_eq!(
            eventos[0].data.as_ref().unwrap()["recurrenceCount"],
            json!(2)
        );

        // A terceira alcança o limiar: o alvo é declarado oscilante.
        oscilar(&ctx, monitor_id).await;
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1, "a declaração não abre evento novo");
        assert_eq!(eventos[0].status, "flapping");
        let data = eventos[0].data.as_ref().unwrap();
        assert_eq!(data["recurrenceCount"], json!(3));
        assert!(data["flappingSince"].is_string());
        assert_eq!(
            data["problemTimeline"].as_array().unwrap().len(),
            3,
            "cada recaída deixou um carimbo na janela deslizante"
        );

        // Já oscilando, mais recaídas não redeclaram nada nem reabrem evento.
        oscilar(&ctx, monitor_id).await;
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].status, "flapping");
        assert_eq!(
            eventos[0].data.as_ref().unwrap()["recurrenceCount"],
            json!(4)
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_flapping_so_resolve_quando_a_contagem_decai_e_a_estabilidade_volta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let (rule_id, monitor_id) = cenario(&request, &ctx).await;
        for _ in 0..3 {
            oscilar(&ctx, monitor_id).await;
        }
        assert_eq!(eventos_da_regra(&ctx, rule_id).await[0].status, "flapping");

        // Uma checagem ok não basta: os carimbos ainda estão dentro da janela.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar subida");
        let evento = eventos_da_regra(&ctx, rule_id).await.remove(0);
        assert_eq!(
            evento.status, "flapping",
            "estabilidade curta não tira o alvo do estado oscilante"
        );

        // Envelhecendo os carimbos e o último problema para fora das janelas,
        // a próxima checagem ok fecha o episódio.
        let mut data = evento.data.as_ref().unwrap().as_object().unwrap().clone();
        let antigo = (Utc::now() - Duration::seconds(2000)).to_rfc3339();
        data.insert("lastProblemAt".into(), json!(antigo));
        data.insert("problemTimeline".into(), json!([antigo]));
        let mut ativo: alert_events::ActiveModel = evento.into();
        ativo.data = Set(Some(Value::Object(data)));
        ativo.update(&ctx.db).await.expect("carimbos no passado");

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar estabilidade além das janelas");
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1, "o episódio inteiro coube num evento só");
        assert_eq!(eventos[0].status, "resolved");
        assert!(eventos[0].resolved_at.is_some());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sem_limiar_configurado_a_oscilacao_nunca_vira_flapping() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let regra = json_of(
            &request
                .post("/api/alert-rules")
                .json(&json!({
                    "name": "Queda sem detecção",
                    "condition": { "field": "status", "operator": "eq", "value": "down" },
                    "severity": "critical",
                    "recoveryWindowSeconds": 300
                }))
                .await
                .text(),
        );
        assert_eq!(regra["flapThreshold"], 0, "detecção nasce desligada");
        let rule_id = regra["id"].as_i64().unwrap();

        let monitor = json_of(
            &request
                .post("/api/monitors")
                .json(&json!({
                    "name": "TCP fechado", "type": "tcp", "target": "127.0.0.1", "port": 9
                }))
                .await
                .text(),
        );
        let monitor_id = monitor["id"].as_i64().unwrap();
        request
            .post(&format!("/api/monitors/{monitor_id}/run"))
            .await;

        for _ in 0..5 {
            oscilar(&ctx, monitor_id).await;
        }
        let eventos = eventos_da_regra(&ctx, rule_id).await;
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].status, "active", "sem limiar não há declaração");
        let data = eventos[0].data.as_ref().unwrap();
        assert_eq!(data["recurrenceCount"], json!(5));
        assert!(
            data.get("problemTimeline").is_none(),
            "sem detecção configurada a linha do tempo não é gravada"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn limiar_negativo_e_recusado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let mut payload = json!({
            "name": "Regra inválida",
            "condition": { "field": "status", "operator": "eq", "value": "down" },
            "flapThreshold": -1
        });
        assert_eq!(
            request
                .post("/api/alert-rules")
                .json(&payload)
                .await
                .status_code(),
            422
        );

        payload["flapThreshold"] = json!(4);
        payload["flapWindowSeconds"] = json!(-30);
        assert_eq!(
            request
                .post("/api/alert-rules")
                .json(&payload)
                .await
                .status_code(),
            422
        );

        payload["flapWindowSeconds"] = json!(600);
        let criada = json_of(&request.post("/api/alert-rules").json(&payload).await.text());
        assert_eq!(criada["flapThreshold"], 4);
        assert_eq!(criada["flapWindowSeconds"], 600);

        // Campo ausente no PUT mantém o valor atual (o toggle da lista manda
        // só `enabled`).
        let id = criada["id"].as_i64().unwrap();
        let atualizada = json_of(
            &request
                .put(&format!("/api/alert-rules/{id}"))
                .json(&json!({ "enabled": false }))
                .await
                .text(),
        );
        assert_eq!(atualizada["flapThreshold"], 4);
        assert_eq!(atualizada["flapWindowSeconds"], 600);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_indicador_de_instabilidade_conta_quedas_por_alvo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        // A instalação nova provisiona o conjunto básico do catálogo, e
        // `device_offline` casa com o mesmo `status: down` — dois episódios
        // para o mesmo alvo tornariam a contagem ambígua. Aqui interessa
        // exatamente uma regra vigiando o monitor.
        let existentes = json_of(&request.get("/api/alert-rules").await.text());
        for regra in existentes.as_array().expect("lista de regras") {
            let id = regra["id"].as_i64().unwrap();
            request.delete(&format!("/api/alert-rules/{id}")).await;
        }

        let (_, monitor_id) = cenario(&request, &ctx).await;
        for _ in 0..3 {
            oscilar(&ctx, monitor_id).await;
        }

        let scope_key = format!("monitor:{monitor_id}");
        let resumo = json_of(
            &request
                .get(&format!("/api/alerts/instability?scopeKey={scope_key}"))
                .await
                .text(),
        );
        let alvo = &resumo.as_array().expect("lista de alvos")[0];
        assert_eq!(alvo["scopeKey"], json!(scope_key));
        // 1 episódio + 3 recaídas = 4 quedas na janela.
        assert_eq!(alvo["oscillations"], json!(4));
        assert_eq!(alvo["episodes"], json!(1));
        assert_eq!(alvo["flapping"], json!(true));
        assert!(alvo["lastProblemAt"].is_string());

        // Sem filtro, o alvo aparece no ranking geral.
        let geral = json_of(&request.get("/api/alerts/instability").await.text());
        assert!(geral
            .as_array()
            .expect("ranking")
            .iter()
            .any(|item| item["scopeKey"] == json!(scope_key)));
    })
    .await;
}
