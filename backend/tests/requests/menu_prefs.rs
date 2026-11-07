//! Menu por usuário (Fase 16): fixar e ocultar, salvo no servidor.

use backend::app::App;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
use serial_test::serial;

use super::prepare_data;

#[tokio::test]
#[serial]
async fn leitor_salva_o_proprio_menu_e_a_sessao_o_le() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_operator(&ctx).await;
        let mut active = session.user.into_active_model();
        active.role = Set("viewer".into());
        active.update(&ctx.db).await.unwrap();
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let salvo = request
            .put("/api/me/menu")
            .json(&serde_json::json!({
                "pinned": ["/topology", "/topology"],
                "hidden": ["/topology", "/logs"]
            }))
            .await;
        assert_eq!(salvo.status_code(), 200, "{}", salvo.text());
        let corpo: serde_json::Value = serde_json::from_str(&salvo.text()).unwrap();
        assert_eq!(
            corpo,
            serde_json::json!({ "pinned": ["/topology"], "hidden": ["/logs"] }),
            "fixado não fica oculto e a repetição sai"
        );

        let invalido = request
            .put("/api/me/menu")
            .json(&serde_json::json!({ "pinned": ["topology"], "hidden": [] }))
            .await;
        assert_eq!(invalido.status_code(), 422, "{}", invalido.text());

        let eu: serde_json::Value =
            serde_json::from_str(&request.get("/api/auth/me").await.text()).unwrap();
        assert_eq!(eu["menuPrefs"]["pinned"], serde_json::json!(["/topology"]));
    })
    .await;
}
