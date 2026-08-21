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
async fn administrador_lista_logs_de_auditoria() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let login = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "email": prepare_data::OPERATOR_EMAIL,
                "password": prepare_data::OPERATOR_PASSWORD
            }))
            .await;
        assert_eq!(login.status_code(), 200, "{}", login.text());

        let response = request.get("/api/audit-logs").await;
        assert_eq!(response.status_code(), 200, "{}", response.text());

        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        let data = body["data"].as_array().expect("data array");
        assert!(!data.is_empty(), "esperava ao menos o log de login");
        let actions: Vec<&str> = data.iter().map(|l| l["action"].as_str().unwrap()).collect();
        assert!(
            actions.contains(&"login"),
            "esperava log de login: {actions:?}"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn operador_ou_visualizador_nao_acessa_auditoria() {
    for role in [Role::Operator, Role::Viewer] {
        request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
            session_with_role(&mut request, &ctx, role).await;

            let response = request.get("/api/audit-logs").await;
            assert_eq!(response.status_code(), 403, "{}", response.text());
        })
        .await;
    }
}

#[tokio::test]
#[serial]
async fn filtra_logs_por_acao_e_recurso() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let admin = session_with_role(&mut request, &ctx, Role::Admin).await;

        let site = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Audit Site" }))
            .await;
        assert_eq!(site.status_code(), 201, "{}", site.text());

        let by_action = request.get("/api/audit-logs?action=create").await;
        assert_eq!(by_action.status_code(), 200, "{}", by_action.text());
        let body: serde_json::Value = serde_json::from_str(&by_action.text()).unwrap();
        assert!(body["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| l["action"] == "create"));

        let by_user = request
            .get(&format!("/api/audit-logs?userId={}", admin.id))
            .await;
        assert_eq!(by_user.status_code(), 200, "{}", by_user.text());
        let body: serde_json::Value = serde_json::from_str(&by_user.text()).unwrap();
        assert!(!body["data"].as_array().unwrap().is_empty());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn paginacao_devolve_meta_correta() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        session_with_role(&mut request, &ctx, Role::Admin).await;

        let response = request.get("/api/audit-logs?page=1&limit=5").await;
        assert_eq!(response.status_code(), 200, "{}", response.text());

        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert!(body["meta"]["currentPage"].is_u64());
        assert!(body["meta"]["lastPage"].is_u64());
        assert!(body["meta"]["total"].is_u64());
        assert!(body["meta"]["perPage"].is_u64());
    })
    .await;
}
