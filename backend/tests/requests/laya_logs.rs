//! Categoria de eventos de log pelo Laya, e o filtro que ela habilita.
//!
//! O que se afirma aqui, contra o Ollaya falso: cada padrão é classificado uma
//! vez (duas linhas de "login failure" com IPs diferentes são uma pergunta
//! só); `GET /api/logs?category=` filtra por ela e a página traz o padrão com
//! o modelo de alerta que o cobre; e a correção do operador vence o palpite.

use backend::{
    app::App,
    dtos::logs::{LogPageResponse, LogTemplateInfo},
    models::logs::{device_logs, log_templates},
    services::{
        ai::settings,
        syslog::{
            categorizer::{backfill, template_of, LayaTemplateClassifier},
            template::hash_hex,
            LogsDb,
        },
    },
};
use chrono::Utc;
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};
use serde_json::json;
use serial_test::serial;

use super::{
    fake_ollaya::{self, by_keyword, with_key},
    prepare_data,
};

fn logs_db(ctx: &AppContext) -> DatabaseConnection {
    LogsDb::from_context(ctx).unwrap().connection().clone()
}

/// Grava a linha como o escritor grava (com o padrão) e o padrão sem categoria.
async fn line(logs: &DatabaseConnection, message: &str) -> i64 {
    let (template, hash) = template_of(message);
    device_logs::ActiveModel {
        source_ip: Set("10.0.0.1".into()),
        received_at: Set(Utc::now().into()),
        severity: Set(Some(4)),
        message: Set(message.into()),
        template_hash: Set(Some(hash)),
        ..Default::default()
    }
    .insert(logs)
    .await
    .unwrap();
    let _ = log_templates::ActiveModel {
        template_hash: Set(hash),
        template: Set(template),
        example: Set(message.into()),
        first_seen_at: Set(Utc::now().into()),
        ..Default::default()
    }
    .insert(logs)
    .await;
    hash
}

#[tokio::test]
#[serial]
async fn classifica_por_padrao_filtra_e_respeita_a_correcao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, ctx| async move {
        with_key();
        let fake = fake_ollaya::start(by_keyword(&[
            ("login failure", "auth_failure"),
            ("link down", "link_change"),
        ]))
        .await;
        let mut laya = fake.settings();
        laya.features.log_events = true;
        settings::save_laya(&ctx.db, laya).await.unwrap();

        let logs = logs_db(&ctx);
        line(&logs, "login failure for user admin from 10.0.0.5").await;
        line(&logs, "login failure for user admin from 10.0.0.9").await;
        let link = line(&logs, "ether3 link down").await;

        let classifier = LayaTemplateClassifier::new(ctx.clone());
        assert_eq!(backfill(&logs, &classifier, None, 10).await, 2);
        assert_eq!(fake.calls(), 2, "uma consulta por padrão, não por linha");

        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (h, v) = prepare_data::auth_header(&session.token);

        let response = request
            .get("/api/logs?category=auth_failure")
            .add_header(h.clone(), v.clone())
            .await;
        response.assert_status_ok();
        let page: LogPageResponse = response.json();
        assert_eq!(page.data.len(), 2);
        assert_eq!(page.templates.len(), 1);
        let template = &page.templates[0];
        assert_eq!(template.category.as_deref(), Some("auth_failure"));
        assert_eq!(
            template.alert_template.as_deref(),
            Some("log_login_failure")
        );
        assert!(!template.confirmed);
        assert_eq!(
            page.data[0].template_hash,
            Some(template.template_hash.clone())
        );

        // O operador corrige: aquele "link down" é de hardware.
        let response = request
            .put(&format!("/api/logs/templates/{}", hash_hex(link)))
            .add_header(h.clone(), v.clone())
            .json(&json!({ "category": "hardware" }))
            .await;
        response.assert_status_ok();
        let info: LogTemplateInfo = response.json();
        assert!(info.confirmed);
        assert_eq!(info.category.as_deref(), Some("hardware"));

        let count = |category: &'static str| {
            let request = &request;
            let (h, v) = (h.clone(), v.clone());
            async move {
                let page: LogPageResponse = request
                    .get(&format!("/api/logs?category={category}"))
                    .add_header(h, v)
                    .await
                    .json();
                page.data.len()
            }
        };
        assert_eq!(count("hardware").await, 1);
        assert_eq!(count("link_change").await, 0, "a correção vence o palpite");

        // A varredura não reclassifica o que o operador decidiu.
        assert_eq!(backfill(&logs, &classifier, None, 10).await, 0);

        let response = request
            .put(&format!("/api/logs/templates/{}", hash_hex(link)))
            .add_header(h, v)
            .json(&json!({ "category": "inventada" }))
            .await;
        assert!(response.status_code().is_client_error());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn frente_desligada_nao_classifica() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        with_key();
        let fake = fake_ollaya::start(by_keyword(&[("login failure", "auth_failure")])).await;
        // O padrão já sai com `log_events: false`.
        settings::save_laya(&ctx.db, fake.settings()).await.unwrap();
        let logs = logs_db(&ctx);
        line(&logs, "login failure for user admin from 10.0.0.5").await;

        let classifier = LayaTemplateClassifier::new(ctx.clone());
        assert_eq!(backfill(&logs, &classifier, None, 10).await, 0);
        assert_eq!(fake.calls(), 0);
    })
    .await;
}
