//! Testes das rotas de Notificações Web Push (PWA).

use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

#[tokio::test]
#[serial]
async fn a_chave_publica_vapid_e_gerada_e_devolvida_com_sucesso() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request.get("/api/push/vapid-public-key").await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let json: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        let public_key = json["publicKey"].as_str().expect("publicKey em string");
        assert!(!public_key.is_empty());

        // Segunda chamada devolve a mesma chave (persistida em system_settings)
        let resposta2 = request.get("/api/push/vapid-public-key").await;
        let json2: serde_json::Value = serde_json::from_str(&resposta2.text()).unwrap();
        assert_eq!(json2["publicKey"], public_key);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_status_do_push_reporta_dados_e_contagem() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let resposta = request.get("/api/push/status").await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let json: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        assert_eq!(json["configured"], true);
        assert_eq!(json["totalSubscriptions"], 0);
        assert_eq!(json["userSubscriptions"], 0);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn registrar_e_remover_subscricao_webpush() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let endpoint = "https://fcm.googleapis.com/fcm/send/fake-test-device-token-123";
        let sub_payload = serde_json::json!({
            "endpoint": endpoint,
            "keys": {
                "p256dh": "BNcRdreALRFXTkOOUHK1EtK2wtaz5Ry4YfYCA_0QTpQtUbVlUls0VJXg7A8u-Ts1XbjhazAkj7I99e8QcYP7DkM",
                "auth": "tBHItJI5svbpez7KI4CCXg"
            },
            "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"
        });

        let post_res = request.post("/api/push/subscriptions").json(&sub_payload).await;
        assert_eq!(post_res.status_code(), 200, "{}", post_res.text());
        let post_json: serde_json::Value = serde_json::from_str(&post_res.text()).unwrap();
        assert_eq!(post_json["success"], true);

        // Verifica status com 1 subscrição
        let status_res = request.get("/api/push/status").await;
        let status_json: serde_json::Value = serde_json::from_str(&status_res.text()).unwrap();
        assert_eq!(status_json["totalSubscriptions"], 1);
        assert_eq!(status_json["userSubscriptions"], 1);

        // Remove subscrição
        let del_payload = serde_json::json!({ "endpoint": endpoint });
        let del_res = request.delete("/api/push/subscriptions").json(&del_payload).await;
        assert_eq!(del_res.status_code(), 200, "{}", del_res.text());

        // Status volta a zero
        let status_depois = request.get("/api/push/status").await;
        let depois_json: serde_json::Value = serde_json::from_str(&status_depois.text()).unwrap();
        assert_eq!(depois_json["totalSubscriptions"], 0);
    })
    .await;
}
