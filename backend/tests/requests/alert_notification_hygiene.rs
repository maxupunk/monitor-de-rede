//! Fase 4 do roadmap de alertas inteligentes: higiene de notificações.
//!
//! O que se valida aqui é o **diário**: toda decisão de notificar vira linha em
//! `notification_outbox`, com o desfecho e o motivo. É o que torna testável uma
//! feature cujo efeito visível é uma mensagem que **não** chega.
//!
//! Como nas Fases 2 e 3, o episódio é conduzido com resultados sintéticos
//! (`process_result`); o alvo real só aparece no disparo inicial (porta 9
//! fechada em loopback = down determinístico).

use backend::{
    app::App,
    models::{
        _entities::alert_events as alert_events_entity, alert_events, devices, notification_outbox,
        sites,
    },
    services::{
        maintenance::data_pruner,
        monitoring::{
            contracts::{CheckResult, MonitorStatus},
            result_processor::process_result,
        },
        notifications::outbox,
    },
};
use chrono::{Duration, Utc};
use loco_rs::{testing::prelude::*, TestServer};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
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

/// O diário inteiro, do mais velho para o mais novo.
async fn diario(ctx: &loco_rs::app::AppContext) -> Vec<notification_outbox::Model> {
    notification_outbox::Entity::find()
        .order_by_asc(notification_outbox::Column::Id)
        .all(&ctx.db)
        .await
        .expect("diário de notificações")
}

/// Desliga o agrupamento: o que estes testes medem é cooldown e inibição, e a
/// espera do digest só embaralharia a leitura.
fn sem_agrupamento() {
    std::env::set_var("NOTIFICATION_DIGEST_WINDOW_SECONDS", "0");
    std::env::set_var("NOTIFICATION_DIGEST_WAIT_SECONDS", "0");
}

fn agrupamento_padrao() {
    std::env::remove_var("NOTIFICATION_DIGEST_WINDOW_SECONDS");
    std::env::remove_var("NOTIFICATION_DIGEST_WAIT_SECONDS");
}

/// A instalação nova provisiona o conjunto básico do catálogo, e várias dessas
/// regras casam com `status: down` — o diário ficaria com linhas de regras que
/// o teste não configurou. Aqui interessa exatamente uma regra vigiando o alvo.
async fn so_a_regra_do_teste(request: &TestServer, payload: Value) -> i64 {
    let existentes = json_of(&request.get("/api/alert-rules").await.text());
    for regra in existentes.as_array().expect("lista de regras") {
        let id = regra["id"].as_i64().unwrap();
        request.delete(&format!("/api/alert-rules/{id}")).await;
    }
    let criada = json_of(&request.post("/api/alert-rules").json(&payload).await.text());
    criada["id"].as_i64().expect("regra criada")
}

/// Monitor TCP para uma porta fechada em loopback: `down` determinístico.
async fn monitor_quebrado(request: &TestServer, nome: &str, device_id: Option<i64>) -> i64 {
    let mut payload = json!({
        "name": nome, "type": "tcp", "target": "127.0.0.1", "port": 9
    });
    if let Some(device_id) = device_id {
        payload["deviceId"] = json!(device_id);
    }
    let monitor = json_of(&request.post("/api/monitors").json(&payload).await.text());
    monitor["id"].as_i64().expect("monitor criado")
}

/// Antecipa a liberação das linhas represadas, para o despacho não depender de
/// esperar a janela de verdade.
async fn liberar_agora(ctx: &loco_rs::app::AppContext) {
    for row in diario(ctx).await {
        if row.status == "pending" {
            let mut active: notification_outbox::ActiveModel = row.into();
            active.deliver_after = Set((Utc::now() - Duration::seconds(1)).into());
            active.update(&ctx.db).await.expect("liberar linha");
        }
    }
}

#[tokio::test]
#[serial]
async fn o_cooldown_cala_o_reabrir_e_a_resolucao_orfa_junto() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        sem_agrupamento();

        // Sem janela de estabilização: cada queda abre um episódio e cada
        // subida o fecha — é exatamente o cenário que o cooldown existe para
        // conter, porque a Fase 1 sozinha não o cobre.
        let _rule_id = so_a_regra_do_teste(
            &request,
            json!({
                "name": "Queda com cooldown",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "severity": "critical",
                "recoveryWindowSeconds": 0,
                "notificationCooldownSeconds": 900
            }),
        )
        .await;
        let monitor_id = monitor_quebrado(&request, "TCP fechado", None).await;

        // 1º episódio: o disparo é enfileirado e entregue.
        request
            .post(&format!("/api/monitors/{monitor_id}/run"))
            .await;
        let linhas = diario(&ctx).await;
        assert_eq!(linhas.len(), 1, "o disparo virou uma linha do diário");
        assert_eq!(linhas[0].kind, "problem");
        assert_eq!(linhas[0].status, "pending");

        let stats = outbox::dispatch_pending(&ctx).await.expect("despacho");
        assert_eq!(stats.delivered, 1);
        assert_eq!(diario(&ctx).await[0].status, "sent");

        // A resolução do episódio anunciado sai normalmente.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar subida");
        outbox::dispatch_pending(&ctx).await.expect("despacho");
        let linhas = diario(&ctx).await;
        assert_eq!(linhas.len(), 2);
        assert_eq!(linhas[1].kind, "resolved");
        assert_eq!(linhas[1].status, "sent");

        // 2º episódio dentro do cooldown: o disparo é engolido...
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Down), None)
            .await
            .expect("processar queda");
        let linhas = diario(&ctx).await;
        assert_eq!(linhas.len(), 3);
        assert_eq!(linhas[2].kind, "problem");
        assert_eq!(linhas[2].status, "suppressed");
        assert_eq!(linhas[2].suppress_reason.as_deref(), Some("cooldown"));

        // ...e a resolução dele também, porque ninguém soube que caiu. Um ✅
        // sem 🚨 correspondente seria ruído puro.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar subida");
        let linhas = diario(&ctx).await;
        assert_eq!(linhas.len(), 4);
        assert_eq!(linhas[3].kind, "resolved");
        assert_eq!(linhas[3].status, "suppressed");
        assert_eq!(linhas[3].suppress_reason.as_deref(), Some("unannounced"));

        // O saldo do critério de aceite: quatro transições, duas mensagens.
        let stats = outbox::dispatch_pending(&ctx).await.expect("despacho");
        assert_eq!(stats.total(), 0, "nada mais tinha a sair");
        agrupamento_padrao();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn alertas_correlatos_saem_numa_mensagem_so() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        // Espera zerada e janela viva: o agrupamento acontece por correlação,
        // não por atraso — dois alvos do mesmo site que caem juntos.
        std::env::set_var("NOTIFICATION_DIGEST_WAIT_SECONDS", "0");
        std::env::set_var("NOTIFICATION_DIGEST_WINDOW_SECONDS", "300");

        let _rule_id = so_a_regra_do_teste(
            &request,
            json!({
                "name": "Queda agrupável",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "severity": "warning"
            }),
        )
        .await;

        let site = sites::ActiveModel {
            name: Set("Matriz".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("site");
        let mut monitores = Vec::new();
        for indice in 1..=2 {
            let device = devices::ActiveModel {
                site_id: Set(Some(site.id)),
                name: Set(format!("Equipamento {indice}")),
                r#type: Set("switch".into()),
                status: Set("online".into()),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await
            .expect("dispositivo");
            monitores
                .push(monitor_quebrado(&request, &format!("TCP {indice}"), Some(device.id)).await);
        }

        for monitor_id in &monitores {
            process_result(&ctx, *monitor_id, &resultado(MonitorStatus::Down), None)
                .await
                .expect("processar queda");
        }
        let linhas = diario(&ctx).await;
        assert_eq!(linhas.len(), 2, "cada alvo pediu a própria notificação");
        assert!(
            linhas
                .iter()
                .all(|row| row.group_key == format!("site:{}", site.id)),
            "os dois alvos correlacionam pelo site"
        );

        let stats = outbox::dispatch_pending(&ctx).await.expect("despacho");
        assert_eq!(stats.delivered, 1, "uma mensagem só chegou ao canal");
        assert_eq!(stats.consolidated, 2, "as duas linhas couberam nela");
        assert!(
            diario(&ctx).await.iter().all(|row| row.status == "sent"),
            "as duas linhas foram entregues pela mensagem consolidada"
        );
        agrupamento_padrao();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_alerta_do_pai_suprime_o_do_filho_ate_o_pai_voltar() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        sem_agrupamento();

        let _rule_id = so_a_regra_do_teste(
            &request,
            json!({
                "name": "Queda com inibição",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "severity": "critical",
                "inhibitWhenParentDown": true
            }),
        )
        .await;

        let pai = devices::ActiveModel {
            name: Set("Roteador de borda".into()),
            r#type: Set("router".into()),
            status: Set("offline".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("pai");
        let filho = devices::ActiveModel {
            parent_id: Set(Some(pai.id)),
            name: Set("Servidor atrás do roteador".into()),
            r#type: Set("server".into()),
            status: Set("offline".into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("filho");

        // O pai já está em alerta: é ele que explica a queda do filho.
        let alerta_do_pai = alert_events::ActiveModel {
            device_id: Set(Some(pai.id)),
            scope_key: Set(Some(format!("device:{}", pai.id))),
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(Utc::now().into()),
            message: Set(Some("Host inacessível".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("alerta do pai");

        let monitor_id = monitor_quebrado(&request, "TCP do filho", Some(filho.id)).await;
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Down), None)
            .await
            .expect("processar queda");

        let linha = diario(&ctx).await.remove(0);
        assert!(linha.inhibitable, "o filho tem pai declarado");
        assert!(
            linha.deliver_after.with_timezone(&Utc) > Utc::now(),
            "a linha inibível espera a carência antes de ser julgada"
        );

        liberar_agora(&ctx).await;
        let stats = outbox::dispatch_pending(&ctx).await.expect("despacho");
        assert_eq!(stats.suppressed, 1);
        assert_eq!(stats.delivered, 0, "o operador só ouve falar do pai");
        let linha = diario(&ctx).await.remove(0);
        assert_eq!(linha.status, "suppressed");
        assert_eq!(linha.suppress_reason.as_deref(), Some("inhibited"));

        // Com o pai normalizado, a queda do filho passa a ser notícia própria.
        let mut fechado: alert_events::ActiveModel = alerta_do_pai.into();
        fechado.status = Set("resolved".into());
        fechado.resolved_at = Set(Some(Utc::now().into()));
        fechado.update(&ctx.db).await.expect("fechar alerta do pai");

        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar subida");
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Down), None)
            .await
            .expect("processar nova queda");
        liberar_agora(&ctx).await;
        let stats = outbox::dispatch_pending(&ctx).await.expect("despacho");
        assert!(stats.delivered >= 1, "sem pai em alerta, o filho fala");
        assert!(
            diario(&ctx)
                .await
                .iter()
                .any(|row| row.kind == "problem" && row.status == "sent"),
            "a segunda queda do filho chegou ao canal"
        );
        agrupamento_padrao();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn alerta_silenciado_nao_notifica_nem_a_propria_resolucao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        sem_agrupamento();

        let rule_id = so_a_regra_do_teste(
            &request,
            json!({
                "name": "Queda silenciável",
                "condition": { "field": "status", "operator": "eq", "value": "down" },
                "severity": "warning"
            }),
        )
        .await;
        let monitor_id = monitor_quebrado(&request, "TCP fechado", None).await;

        request
            .post(&format!("/api/monitors/{monitor_id}/run"))
            .await;
        outbox::dispatch_pending(&ctx).await.expect("despacho");

        let evento = alert_events::Entity::find()
            .filter(alert_events_entity::Column::AlertRuleId.eq(rule_id))
            .one(&ctx.db)
            .await
            .expect("consulta")
            .expect("evento aberto");
        assert_eq!(
            request
                .post(&format!("/api/alerts/{}/silence", evento.id))
                .json(&json!({ "minutes": 60 }))
                .await
                .status_code(),
            200
        );

        // O ✅ de um alerta silenciado era ruído que furava o pedido do
        // operador; agora ele fica registrado como suprimido, não entregue.
        process_result(&ctx, monitor_id, &resultado(MonitorStatus::Up), None)
            .await
            .expect("processar subida");
        let resolucao = diario(&ctx)
            .await
            .into_iter()
            .find(|row| row.kind == "resolved")
            .expect("linha da resolução");
        assert_eq!(resolucao.status, "suppressed");
        assert_eq!(resolucao.suppress_reason.as_deref(), Some("silenced"));
        agrupamento_padrao();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_purga_apaga_episodio_fechado_e_preserva_o_que_esta_aberto() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let antigo = Utc::now() - Duration::days(200);

        let fechado = alert_events::ActiveModel {
            status: Set("resolved".into()),
            severity: Set("warning".into()),
            started_at: Set(antigo.into()),
            resolved_at: Set(Some(antigo.into())),
            message: Set(Some("Episódio encerrado".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("evento fechado");
        let aberto = alert_events::ActiveModel {
            status: Set("active".into()),
            severity: Set("critical".into()),
            started_at: Set(antigo.into()),
            message: Set(Some("Ainda caído".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("evento aberto");

        let entregue = notification_outbox::ActiveModel {
            group_key: Set("global".into()),
            kind: Set("problem".into()),
            title: Set("Antiga".into()),
            body: Set("Corpo".into()),
            severity: Set("warning".into()),
            metadata: Set(json!({})),
            status: Set("sent".into()),
            inhibitable: Set(false),
            deliver_after: Set(antigo.into()),
            sent_at: Set(Some(antigo.into())),
            created_at: Set(antigo.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("notificação entregue");
        let pendente = notification_outbox::ActiveModel {
            group_key: Set("global".into()),
            kind: Set("problem".into()),
            title: Set("Por entregar".into()),
            body: Set("Corpo".into()),
            severity: Set("warning".into()),
            metadata: Set(json!({})),
            status: Set("pending".into()),
            inhibitable: Set(false),
            deliver_after: Set(antigo.into()),
            created_at: Set(antigo.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("notificação pendente");

        let stats = data_pruner::prune_all(&ctx.db).await.expect("purga");
        assert_eq!(stats.alert_events_deleted, 1);
        assert_eq!(stats.notifications_deleted, 1);

        assert!(
            alert_events::Entity::find_by_id(fechado.id)
                .one(&ctx.db)
                .await
                .expect("consulta")
                .is_none(),
            "o episódio fechado e antigo devia ter sido purgado"
        );
        assert!(
            alert_events::Entity::find_by_id(aberto.id)
                .one(&ctx.db)
                .await
                .expect("consulta")
                .is_some(),
            "alerta aberto nunca é purgado, por mais antigo que seja"
        );
        assert!(notification_outbox::Entity::find_by_id(entregue.id)
            .one(&ctx.db)
            .await
            .expect("consulta")
            .is_none());
        assert!(
            notification_outbox::Entity::find_by_id(pendente.id)
                .one(&ctx.db)
                .await
                .expect("consulta")
                .is_some(),
            "linha pendente é notificação por entregar, não histórico"
        );
    })
    .await;
}
