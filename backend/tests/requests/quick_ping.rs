//! Diagnóstico rápido por ping (`POST /api/diagnostics/quick-ping`, ADR 015 §9).
//!
//! Só 127.0.0.1 — nenhum teste sai da máquina. Onde o sistema recusa o socket
//! ICMP sem privilégio (sysctl fechado, Windows), a resposta tem que ser o 503
//! que diz o que fazer, nunca um 500.

use backend::{app::App, models::devices, services::users::Role};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

async fn session_with_role(request: &mut loco_rs::TestServer, ctx: &AppContext, role: Role) {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.as_str().to_string());
    active.update(&ctx.db).await.expect("papel");
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

async fn device(ctx: &AppContext, name: &str, ip: Option<&str>) -> devices::Model {
    devices::ActiveModel {
        name: Set(name.into()),
        r#type: Set("router".into()),
        status: Set("unknown".into()),
        ip_address: Set(ip.map(Into::into)),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

fn body(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

#[tokio::test]
#[serial]
async fn valida_o_alvo_antes_de_pingar() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let without_ip = device(&ctx, "switch-sem-ip", None).await;

        let cases = [
            (json!({}), 422),
            (json!({ "ip": "não-é-ip" }), 422),
            // Endereço público avulso: a central não vira canhão de ping.
            (json!({ "ip": "8.8.8.8" }), 422),
            (json!({ "deviceId": without_ip.id }), 422),
            (json!({ "deviceId": 999_999 }), 404),
        ];
        for (payload, status) in cases {
            let response = request
                .post("/api/diagnostics/quick-ping")
                .json(&payload)
                .await;
            assert_eq!(
                response.status_code(),
                status,
                "{payload}: {}",
                response.text()
            );
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn somente_leitura_nao_dispara() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Viewer).await;
        let response = request
            .post("/api/diagnostics/quick-ping")
            .json(&json!({ "ip": "127.0.0.1" }))
            .await;
        assert_eq!(response.status_code(), 403, "{}", response.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn loopback_devolve_relatorio_ou_503_explicado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let target = device(&ctx, "loopback", Some("127.0.0.1")).await;

        let response = request
            .post("/api/diagnostics/quick-ping")
            .json(&json!({ "deviceId": target.id }))
            .await;
        let report = body(&response.text());
        match response.status_code().as_u16() {
            200 => {
                assert_eq!(report["target"], "127.0.0.1");
                assert!(report["sent"].as_u64().unwrap() > 0);
                assert!(report["capacityNote"].is_string());
                assert!(report["verdict"].as_str().is_some_and(|v| !v.is_empty()));
                // Loopback é rápido demais para o método: nunca sai número.
                if report["samples"].as_u64().unwrap() > 0 {
                    assert!(report["capacity"].is_null(), "{report}");
                    assert!(report["small"]["minMs"].is_number());
                }
            }
            503 => assert!(
                report["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("Ping indisponível")),
                "{report}"
            ),
            other => panic!("status inesperado {other}: {report}"),
        }
    })
    .await;
}
