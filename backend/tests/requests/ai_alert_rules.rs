//! Regras de alerta pela IA: a origem de um alerta, a listagem com o ruído de
//! cada regra, e criar/excluir só depois da confirmação — pelo mesmo serviço
//! da tela.

use backend::{
    app::App,
    models::_entities::{alert_events, alert_rules, audit_logs, devices},
    services::{
        ai::harness::tools::{ToolArgs, ToolPolicy, ToolRegistry},
        audit::AuditActor,
    },
};
use chrono::{Duration, Utc};
use loco_rs::{prelude::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serde_json::json;
use serial_test::serial;

const COM_ACOES: ToolPolicy = ToolPolicy {
    allow_active: false,
    allow_actions: true,
    confirm_active: false,
    interactive: true,
};

const SO_LEITURA: ToolPolicy = ToolPolicy {
    allow_active: false,
    allow_actions: false,
    confirm_active: false,
    interactive: true,
};

async fn aparelho(ctx: &AppContext) -> devices::Model {
    devices::ActiveModel {
        name: Set("Borda Principal".into()),
        r#type: Set("router".into()),
        ip_address: Set(Some("10.0.0.1".into())),
        is_monitored: Set(true),
        status: Set("up".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn regra(ctx: &AppContext, device_id: i64) -> alert_rules::Model {
    alert_rules::ActiveModel {
        device_id: Set(Some(device_id)),
        name: Set("Latência alta na borda".into()),
        r#type: Set("custom".into()),
        condition: Set(json!({ "field": "latencyMs", "operator": "gt", "value": 150 })),
        severity: Set("warning".into()),
        duration_seconds: Set(60),
        recovery_window_seconds: Set(0),
        flap_threshold: Set(0),
        flap_window_seconds: Set(900),
        notification_cooldown_seconds: Set(0),
        inhibit_when_parent_down: Set(false),
        enabled: Set(true),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn disparo(ctx: &AppContext, rule: &alert_rules::Model, status: &str) -> alert_events::Model {
    let inicio = Utc::now() - Duration::minutes(10);
    alert_events::ActiveModel {
        alert_rule_id: Set(Some(rule.id)),
        device_id: Set(rule.device_id),
        scope_key: Set(Some("interface:999".into())),
        status: Set(status.into()),
        severity: Set(rule.severity.clone()),
        started_at: Set(inicio.into()),
        message: Set(Some("Latência 320 ms".into())),
        data: Set(Some(json!({ "latencyMs": 320 }))),
        created_at: Set(inicio.into()),
        updated_at: Set(inicio.into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

#[tokio::test]
#[serial]
async fn explica_a_origem_do_alerta_pela_regra_e_pelos_fatos() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let rule = regra(&ctx, dev.id).await;
        let evento = disparo(&ctx, &rule, "active").await;
        disparo(&ctx, &rule, "resolved").await;
        let registro = ToolRegistry::new(SO_LEITURA);

        let origem = registro
            .execute(
                &ctx,
                "explain_alert",
                &json!({ "alert_id": evento.id }).to_string(),
            )
            .await
            .unwrap()
            .data;
        assert_eq!(origem["rule"]["id"], rule.id);
        assert_eq!(origem["rule"]["condition"], "latencyMs gt 150");
        assert_eq!(origem["rule"]["scope"], "dispositivo Borda Principal");
        assert_eq!(origem["rule"]["fired_24h"], 2);
        assert_eq!(origem["facts"]["latencyMs"], 320);
        assert_eq!(origem["device"]["name"], "Borda Principal");
        assert_eq!(
            origem["target"],
            json!({ "kind": "interface", "id": 999, "missing": true }),
            "alvo apagado não derruba a explicação"
        );

        let lista = registro
            .execute(
                &ctx,
                "list_alert_rules",
                r#"{"device": "borda", "search": "latência alta"}"#,
            )
            .await
            .unwrap()
            .data;
        assert_eq!(lista["total"], 1);
        assert_eq!(lista["rules"][0]["open_alerts"], 1);
        assert_eq!(lista["rules"][0]["noisy"], false);

        let inexistente = registro
            .execute(&ctx, "explain_alert", r#"{"alert_id": 424242}"#)
            .await
            .unwrap();
        assert!(inexistente.data["error"].is_string());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cria_regra_so_confirmada_e_recusa_campo_inventado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let registro = ToolRegistry::new(COM_ACOES);
        let argumentos = json!({
            "name": "CPU alta na borda",
            "field": "cpuUsagePercent",
            "operator": "gte",
            "value": 90,
            "severity": "warning",
            "device": "borda",
            "duration_seconds": 300
        })
        .to_string();

        assert!(registro.needs_confirmation("create_alert_rule"));
        assert!(
            registro
                .execute(&ctx, "create_alert_rule", &argumentos)
                .await
                .is_err(),
            "o chat não cria regra sozinho"
        );
        let resumo = registro
            .preview(&ctx, "create_alert_rule", &argumentos)
            .await
            .unwrap();
        assert!(resumo.contains("cpuUsagePercent gte 90"), "{resumo}");
        assert!(resumo.contains("Borda Principal"), "{resumo}");
        assert!(resumo.contains("300 s"), "{resumo}");

        let ator = AuditActor {
            user_id: None,
            ip: Some("127.0.0.1".into()),
            user_agent: None,
        };
        let feito = registro
            .execute_confirmed(
                &ctx,
                "create_alert_rule",
                ToolArgs::parse(&argumentos).with_actor(ator),
            )
            .await
            .unwrap();
        let criada = alert_rules::Entity::find_by_id(feito.data["rule_id"].as_i64().unwrap())
            .one(&ctx.db)
            .await
            .unwrap()
            .expect("regra gravada");
        assert_eq!(criada.device_id, Some(dev.id));
        assert_eq!(criada.duration_seconds, 300);
        assert_eq!(criada.condition["field"], "cpuUsagePercent");
        let auditada = audit_logs::Entity::find()
            .filter(audit_logs::Column::ResourceId.eq(criada.id))
            .count(&ctx.db)
            .await
            .unwrap();
        assert_eq!(auditada, 1, "a criação entra na auditoria como pela tela");

        let inventado = registro
            .preview(
                &ctx,
                "create_alert_rule",
                &json!({ "name": "x", "field": "cpuLoad", "operator": "gt", "value": 1 })
                    .to_string(),
            )
            .await
            .unwrap();
        assert!(inventado.starts_with("Ação inválida"), "{inventado}");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn excluir_regra_avisa_que_o_historico_vai_junto() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx).await;
        let rule = regra(&ctx, dev.id).await;
        disparo(&ctx, &rule, "resolved").await;
        disparo(&ctx, &rule, "resolved").await;
        let argumentos = json!({ "rule_id": rule.id }).to_string();

        let sem_acoes = ToolRegistry::new(SO_LEITURA)
            .execute_confirmed(&ctx, "delete_alert_rule", ToolArgs::parse(&argumentos))
            .await;
        assert!(
            sem_acoes.is_err(),
            "ação desligada nas configurações não roda"
        );

        let registro = ToolRegistry::new(COM_ACOES);
        let resumo = registro
            .preview(&ctx, "delete_alert_rule", &argumentos)
            .await
            .unwrap();
        assert!(resumo.contains("Os 2 alertas"), "{resumo}");

        let feito = registro
            .execute_confirmed(&ctx, "delete_alert_rule", ToolArgs::parse(&argumentos))
            .await
            .unwrap();
        assert_eq!(feito.data["deleted_alerts"], 2);
        assert!(alert_rules::Entity::find_by_id(rule.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());

        let de_novo = registro
            .execute_confirmed(&ctx, "delete_alert_rule", ToolArgs::parse(&argumentos))
            .await
            .unwrap();
        assert!(
            de_novo.data["error"].is_string(),
            "regra já excluída vira aviso, não erro 500"
        );
    })
    .await;
}
