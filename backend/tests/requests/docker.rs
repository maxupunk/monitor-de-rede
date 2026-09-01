//! Contrato HTTP do gerenciador Docker, inclusive quando a Engine não existe.

use backend::{app::App, models::users, services::users::Role};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serial_test::serial;

use super::prepare_data;

async fn session_with_role(
    request: &mut loco_rs::TestServer,
    ctx: &loco_rs::app::AppContext,
    role: Role,
) -> users::Model {
    let session = prepare_data::init_operator(ctx).await;
    let mut active = session.user.into_active_model();
    active.role = Set(role.as_str().to_string());
    let user = active.update(&ctx.db).await.unwrap();
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
    user
}

#[tokio::test]
#[serial]
async fn status_exige_sessao_e_mantem_contrato_sem_engine() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        assert_eq!(request.get("/api/docker/status").await.status_code(), 401);

        session_with_role(&mut request, &ctx, Role::Viewer).await;
        let response = request.get("/api/docker/status").await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert!(body["available"].is_boolean());
        assert!(body.get("reason").is_some());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn listagens_distinguem_lista_vazia_de_engine_indisponivel() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Viewer).await;
        for path in [
            "/api/docker/containers",
            "/api/docker/volumes",
            "/api/docker/networks",
            "/api/docker/images",
        ] {
            let response = request.get(path).await;
            assert_eq!(response.status_code(), 200, "{path}: {}", response.text());
            let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
            assert!(body["available"].is_boolean(), "{path}: {body}");
            assert!(body["data"].is_array(), "{path}: {body}");
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn metricas_degradam_sem_quebrar_o_dashboard() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Viewer).await;
        let response = request.get("/api/docker/metrics").await;
        assert_eq!(response.status_code(), 200, "{}", response.text());
        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert!(body["dockerAvailable"].is_boolean());
        assert!(body["containers"].is_array());
        assert!(body["collectedAt"].is_string());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn somente_administrador_controla_a_engine() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Operator).await;
        let response = request
            .post("/api/docker/networks")
            .json(&serde_json::json!({ "name": "rede-teste" }))
            .await;
        assert_eq!(response.status_code(), 403, "{}", response.text());

        let export = request.get("/api/docker/volumes/dados/export").await;
        assert_eq!(export.status_code(), 403, "{}", export.text());

        let clear_logs = request.delete("/api/docker/containers/exemplo/logs").await;
        assert_eq!(clear_logs.status_code(), 403, "{}", clear_logs.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn entradas_invalidas_falham_antes_de_tocar_na_engine() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let network = request
            .post("/api/docker/networks")
            .json(&serde_json::json!({ "name": "", "driver": "bridge" }))
            .await;
        assert_eq!(network.status_code(), 422, "{}", network.text());

        let logs = request
            .get("/api/docker/containers/exemplo/logs?tail=10001")
            .await;
        assert_eq!(logs.status_code(), 422, "{}", logs.text());
    })
    .await;
}
