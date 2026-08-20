use backend::{app::App, models::users, services::auth::setup::SetupService};
use insta::{assert_debug_snapshot, with_settings};
use loco_rs::testing::prelude::*;
use rstest::rstest;
use sea_orm::IntoActiveModel;
use serial_test::serial;

use super::prepare_data;

/// Configura os parâmetros de snapshot do Insta para testes de autenticação.
macro_rules! configure_insta {
    ($($expr:expr),*) => {
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_snapshot_suffix("auth_request");
        let _guard = settings.bind_to_scope();
    };
}

/// Payload do cadastro inicial com o token vigente da instalação.
fn setup_payload(token: &str) -> serde_json::Value {
    serde_json::json!({
        "name": "Operador",
        "email": "admin@netmonitor.local",
        "password": "Senha-bem-forte",
        "token": token,
    })
}

#[tokio::test]
#[serial]
async fn banco_vazio_reporta_instalacao_pendente() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/api/auth/setup").await;
        assert_eq!(response.status_code(), 200);
        response.assert_json(&serde_json::json!({ "needsSetup": true }));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn setup_cria_o_primeiro_usuario_e_ja_devolve_a_sessao() {
    request::<App, _, _>(|request, ctx| async move {
        let token = SetupService::new(&ctx.db).token().await.unwrap();

        let response = request
            .post("/api/auth/setup")
            .json(&setup_payload(&token))
            .await;
        assert_eq!(response.status_code(), 200, "{}", response.text());

        let session: backend::views::auth::LoginResponse =
            serde_json::from_str(&response.text()).unwrap();
        assert_eq!(session.user.email, "admin@netmonitor.local");
        assert!(!session.token.is_empty());

        // A sessão devolvida precisa valer de imediato — é com ela que o
        // frontend entra no dashboard sem passar pela tela de login.
        let (auth_key, auth_value) = prepare_data::auth_header(&session.token);
        let me = request
            .get("/api/auth/me")
            .add_header(auth_key, auth_value)
            .await;
        assert_eq!(me.status_code(), 200);

        // Sem SMTP numa instalação nova, o primeiro usuário já nasce verificado.
        let user = users::Model::find_by_email(&ctx.db, "admin@netmonitor.local")
            .await
            .unwrap();
        assert!(user.email_verified_at.is_some());

        // E a porta se fecha atrás dele.
        let status = request.get("/api/auth/setup").await;
        status.assert_json(&serde_json::json!({ "needsSetup": false }));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn setup_recusa_token_invalido_sem_criar_usuario() {
    request::<App, _, _>(|request, ctx| async move {
        let response = request
            .post("/api/auth/setup")
            .json(&setup_payload("token-errado"))
            .await;

        assert_eq!(response.status_code(), 401);
        response.assert_json(&serde_json::json!({ "message": "Token de instalação inválido." }));
        assert!(SetupService::new(&ctx.db).is_pending().await.unwrap());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn setup_recusa_senha_curta_por_campo() {
    request::<App, _, _>(|request, ctx| async move {
        let token = SetupService::new(&ctx.db).token().await.unwrap();
        let mut payload = setup_payload(&token);
        payload["password"] = serde_json::json!("1234");

        let response = request.post("/api/auth/setup").json(&payload).await;

        assert_eq!(response.status_code(), 422);
        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(body["errors"][0]["field"], "password");
        assert!(SetupService::new(&ctx.db).is_pending().await.unwrap());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn setup_recusa_senha_sem_maiuscula_por_campo() {
    request::<App, _, _>(|request, ctx| async move {
        let token = SetupService::new(&ctx.db).token().await.unwrap();
        let mut payload = setup_payload(&token);
        payload["password"] = serde_json::json!("sem-maiuscula1");

        let response = request.post("/api/auth/setup").json(&payload).await;

        assert_eq!(response.status_code(), 422);
        let body: serde_json::Value = serde_json::from_str(&response.text()).unwrap();
        assert_eq!(body["errors"][0]["field"], "password");
        assert!(SetupService::new(&ctx.db).is_pending().await.unwrap());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn setup_fecha_depois_do_primeiro_usuario() {
    request::<App, _, _>(|request, ctx| async move {
        let token = SetupService::new(&ctx.db).token().await.unwrap();
        prepare_data::create_user(&ctx, "quem chegou antes", "primeiro@loco.com", "Senha1234")
            .await;

        let response = request
            .post("/api/auth/setup")
            .json(&setup_payload(&token))
            .await;

        // 409 mesmo com o token certo: instalação concluída não reabre.
        assert_eq!(response.status_code(), 409);
        assert!(
            users::Model::find_by_email(&ctx.db, "admin@netmonitor.local")
                .await
                .is_err()
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn register_exige_sessao() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request
            .post("/api/auth/register")
            .json(&serde_json::json!({
                "name": "intruso",
                "email": "intruso@loco.com",
                "password": "Senha1234"
            }))
            .await;

        assert_eq!(
            response.status_code(),
            401,
            "cadastro aberto tornaria o token de instalação inútil"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn login_recusa_usuario_desativado() {
    request::<App, _, _>(|request, ctx| async move {
        let user = prepare_data::create_user(&ctx, "desligado", "ex@loco.com", "Senha1234").await;
        let mut active = user.into_active_model();
        active.active = sea_orm::ActiveValue::Set(false);
        sea_orm::ActiveModelTrait::update(active, &ctx.db)
            .await
            .unwrap();

        let response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "email": "ex@loco.com",
                "password": "Senha1234"
            }))
            .await;

        assert_eq!(response.status_code(), 401);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn login_ignora_caixa_do_email() {
    request::<App, _, _>(|request, ctx| async move {
        prepare_data::create_user(&ctx, "maiuscula", "Admin@Casa.com", "Senha1234").await;

        let response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "email": "  ADMIN@casa.com  ",
                "password": "Senha1234"
            }))
            .await;

        assert_eq!(response.status_code(), 200, "{}", response.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_register() {
    configure_insta!();

    request::<App, _, _>(|request, ctx| async move {
        // O cadastro deixou de ser aberto: quem cria usuário é quem já entrou.
        let operator = prepare_data::init_operator(&ctx).await;
        let (auth_key, auth_value) = prepare_data::auth_header(&operator.token);

        let email = "test@loco.com";
        let payload = serde_json::json!({
            "name": "loco",
            "email": email,
            "password": "Senha1234"
        });

        let response = request
            .post("/api/auth/register")
            .add_header(auth_key, auth_value)
            .json(&payload)
            .await;
        assert_eq!(
            response.status_code(),
            200,
            "Register request should succeed"
        );
        let saved_user = users::Model::find_by_email(&ctx.db, email).await;

        with_settings!({
            filters => cleanup_user_model()
        }, {
            assert_debug_snapshot!(saved_user);
        });

        let deliveries = ctx.mailer.unwrap().deliveries();
        assert_eq!(deliveries.count, 1, "Exactly one email should be sent");
    })
    .await;
}

#[rstest]
#[case("login_with_valid_password", "Senha1234")]
#[case("login_with_invalid_password", "invalid-password")]
#[tokio::test]
#[serial]
async fn can_login_with_verify(#[case] test_name: &str, #[case] password: &str) {
    configure_insta!();

    request::<App, _, _>(|request, ctx| async move {
        let email = "test@loco.com";

        let user = prepare_data::create_user(&ctx, "loco", email, "Senha1234").await;
        let user = user
            .into_active_model()
            .set_email_verification_sent(&ctx.db)
            .await
            .expect("Email verification token should be generated");

        let email_verification_token = user
            .email_verification_token
            .expect("Email verification token should be generated");
        request
            .get(&format!("/api/auth/verify/{email_verification_token}"))
            .await;

        //verify user request
        let response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "email": email,
                "password": password
            }))
            .await;

        // Make sure email_verified_at is set
        let user = users::Model::find_by_email(&ctx.db, email)
            .await
            .expect("Failed to find user by email");

        assert!(
            user.email_verified_at.is_some(),
            "Expected the email to be verified, but it was not. User: {:?}",
            user
        );

        with_settings!({
            filters => cleanup_user_model()
        }, {
            assert_debug_snapshot!(test_name, (response.status_code(), response.text()));
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn login_with_un_existing_email() {
    configure_insta!();

    request::<App, _, _>(|request, _ctx| async move {
        let login_response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "email": "un_existing@loco.rs",
                "password":  "1234"
            }))
            .await;

        assert_eq!(
            login_response.status_code(),
            401,
            "Login request should return 401"
        );
        // Mensagem única para e-mail inexistente e senha errada: distinguir os
        // dois entregaria a lista de quem tem conta. O corpo usa `message`
        // porque é o campo que o `apiService` do frontend lê (§5.5).
        login_response.assert_json(&serde_json::json!({"message": "E-mail ou senha inválidos."}));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_login_without_verify() {
    configure_insta!();

    request::<App, _, _>(|request, ctx| async move {
        let email = "test@loco.com";
        let password = "Senha1234";

        // Usuário sem verificação de e-mail nenhuma: o login não a exige.
        prepare_data::create_user(&ctx, "loco", email, password).await;

        let login_response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "email": email,
                "password": password
            }))
            .await;

        assert_eq!(
            login_response.status_code(),
            200,
            "Login request should succeed"
        );

        with_settings!({
            filters => cleanup_user_model()
        }, {
            assert_debug_snapshot!(login_response.text());
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn seed_admin_local_autentica_no_ambiente_de_teste() {
    request::<App, _, _>(|request, ctx| async move {
        seed::<App>(&ctx).await.unwrap();
        let response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "email": "admin@monitor.local",
                "password": "admin123",
            }))
            .await;
        assert_eq!(response.status_code(), 200);
        let logged: backend::views::auth::LoginResponse =
            serde_json::from_str(&response.text()).unwrap();
        assert_eq!(logged.user.email, "admin@monitor.local");
        assert_eq!(logged.user.role, "admin");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn invalid_verification_token() {
    configure_insta!();

    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/api/auth/verify/invalid-token").await;

        assert_eq!(response.status_code(), 401, "Verify request should reject");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_reset_password() {
    configure_insta!();

    request::<App, _, _>(|request, ctx| async move {
        let login_data = prepare_data::init_user_login(&request, &ctx).await;

        let forgot_payload = serde_json::json!({
            "email": login_data.user.email,
        });
        let forget_response = request.post("/api/auth/forgot").json(&forgot_payload).await;
        assert_eq!(
            forget_response.status_code(),
            200,
            "Forget request should succeed"
        );

        let user = users::Model::find_by_email(&ctx.db, &login_data.user.email)
            .await
            .expect("Failed to find user by email");

        assert!(
            user.reset_token.is_some(),
            "Expected reset_token to be set, but it was None. User: {user:?}"
        );
        assert!(
            user.reset_sent_at.is_some(),
            "Expected reset_sent_at to be set, but it was None. User: {user:?}"
        );

        let new_password = "New-password-123";
        let reset_payload = serde_json::json!({
            "token": user.reset_token,
            "password": new_password,
        });

        let reset_response = request.post("/api/auth/reset").json(&reset_payload).await;
        assert_eq!(
            reset_response.status_code(),
            200,
            "Reset password request should succeed"
        );

        let user = users::Model::find_by_email(&ctx.db, &user.email)
            .await
            .unwrap();

        assert!(user.reset_token.is_none());
        assert!(user.reset_sent_at.is_none());

        assert_debug_snapshot!(reset_response.text());

        let login_response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "email": user.email,
                "password": new_password
            }))
            .await;

        assert_eq!(
            login_response.status_code(),
            200,
            "Login request should succeed"
        );

        let deliveries = ctx.mailer.unwrap().deliveries();
        assert_eq!(deliveries.count, 2, "Exactly one email should be sent");
        // with_settings!({
        //     filters => cleanup_email()
        // }, {
        //     assert_debug_snapshot!(deliveries.messages);
        // });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_get_current_user() {
    configure_insta!();

    request::<App, _, _>(|request, ctx| async move {
        let user = prepare_data::init_user_login(&request, &ctx).await;

        let (auth_key, auth_value) = prepare_data::auth_header(&user.token);
        let response = request
            .get("/api/auth/me")
            .add_header(auth_key, auth_value)
            .await;

        assert_eq!(
            response.status_code(),
            200,
            "Current request should succeed"
        );

        with_settings!({
            filters => cleanup_user_model()
        }, {
            assert_debug_snapshot!((response.status_code(), response.text()));
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_auth_with_magic_link() {
    configure_insta!();
    request::<App, _, _>(|request, ctx| async move {
        seed::<App>(&ctx).await.unwrap();

        let payload = serde_json::json!({
            "email": "user1@example.com",
        });
        let response = request.post("/api/auth/magic-link").json(&payload).await;
        assert_eq!(
            response.status_code(),
            200,
            "Magic link request should succeed"
        );

        let deliveries = ctx.mailer.unwrap().deliveries();
        assert_eq!(deliveries.count, 1, "Exactly one email should be sent");

        // let redact_token = format!("[a-zA-Z0-9]{{{}}}", users::MAGIC_LINK_LENGTH);
        // with_settings!({
        //      filters => {
        //          let mut combined_filters = cleanup_email().clone();
        //         combined_filters.extend(vec![(r"(\\r\\n|=\\r\\n)", ""), (redact_token.as_str(), "[REDACT_TOKEN]") ]);
        //         combined_filters
        //     }
        // }, {
        //     assert_debug_snapshot!(deliveries.messages);
        // });

        let user = users::Model::find_by_email(&ctx.db, "user1@example.com")
            .await
            .expect("User should be found");

        let magic_link_token = user
            .magic_link_token
            .expect("Magic link token should be generated");
        let magic_link_response = request
            .get(&format!("/api/auth/magic-link/{magic_link_token}"))
            .await;
        assert_eq!(
            magic_link_response.status_code(),
            200,
            "Magic link authentication should succeed"
        );

        with_settings!({
            filters => cleanup_user_model()
        }, {
            assert_debug_snapshot!(magic_link_response.text());
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_reject_invalid_email() {
    configure_insta!();
    request::<App, _, _>(|request, _ctx| async move {
        let invalid_email = "user1@temp-mail.com";
        let payload = serde_json::json!({
            "email": invalid_email,
        });
        let response = request.post("/api/auth/magic-link").json(&payload).await;
        assert_eq!(
            response.status_code(),
            400,
            "Expected request with invalid email '{invalid_email}' to be blocked, but it was allowed."
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_reject_invalid_magic_link_token() {
    configure_insta!();
    request::<App, _, _>(|request, ctx| async move {
        seed::<App>(&ctx).await.unwrap();

        let magic_link_response = request.get("/api/auth/magic-link/invalid-token").await;
        assert_eq!(
            magic_link_response.status_code(),
            401,
            "Magic link authentication should be rejected"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_resend_verification_email() {
    configure_insta!();

    request::<App, _, _>(|request, ctx| async move {
        let operator = prepare_data::init_operator(&ctx).await;
        let (auth_key, auth_value) = prepare_data::auth_header(&operator.token);

        let email = "test@loco.com";
        let payload = serde_json::json!({
            "name": "loco",
            "email": email,
            "password": "Senha1234"
        });

        let response = request
            .post("/api/auth/register")
            .add_header(auth_key, auth_value)
            .json(&payload)
            .await;
        assert_eq!(
            response.status_code(),
            200,
            "Register request should succeed"
        );

        let resend_payload = serde_json::json!({ "email": email });

        let resend_response = request
            .post("/api/auth/resend-verification-mail")
            .json(&resend_payload)
            .await;

        assert_eq!(
            resend_response.status_code(),
            200,
            "Resend verification email should succeed"
        );

        let deliveries = ctx.mailer.unwrap().deliveries();

        assert_eq!(
            deliveries.count, 2,
            "Two emails should have been sent: welcome and re-verification"
        );

        let user = users::Model::find_by_email(&ctx.db, email)
            .await
            .expect("User should exist");

        with_settings!({
            filters => cleanup_user_model()
        }, {
            assert_debug_snapshot!("resend_verification_user", user);
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cannot_resend_email_if_already_verified() {
    configure_insta!();

    request::<App, _, _>(|request, ctx| async move {
        let operator = prepare_data::init_operator(&ctx).await;
        let (auth_key, auth_value) = prepare_data::auth_header(&operator.token);

        let email = "verified@loco.com";
        let payload = serde_json::json!({
            "name": "verified",
            "email": email,
            "password": "Senha1234"
        });

        request
            .post("/api/auth/register")
            .add_header(auth_key, auth_value)
            .json(&payload)
            .await;

        // Verify user
        let user = users::Model::find_by_email(&ctx.db, email).await.unwrap();
        if let Some(token) = user.email_verification_token.clone() {
            request.get(&format!("/api/auth/verify/{token}")).await;
        }

        // Try resending verification email
        let resend_payload = serde_json::json!({ "email": email });

        let resend_response = request
            .post("/api/auth/resend-verification-mail")
            .json(&resend_payload)
            .await;

        assert_eq!(
            resend_response.status_code(),
            200,
            "Should return 200 even if already verified"
        );

        let deliveries = ctx.mailer.unwrap().deliveries();
        assert_eq!(
            deliveries.count, 1,
            "Only the original welcome email should be sent"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn register_recusa_senha_curta_ou_sem_maiuscula() {
    request::<App, _, _>(|request, ctx| async move {
        let operator = prepare_data::init_operator(&ctx).await;
        let (auth_key, auth_value) = prepare_data::auth_header(&operator.token);

        // Senha curta (< 8 caracteres)
        let curta_res = request
            .post("/api/auth/register")
            .add_header(auth_key.clone(), auth_value.clone())
            .json(&serde_json::json!({
                "name": "Operador 2",
                "email": "op2@loco.com",
                "password": "Short1"
            }))
            .await;
        assert_eq!(curta_res.status_code(), 422);

        // Senha sem maiúscula
        let sem_mai_res = request
            .post("/api/auth/register")
            .add_header(auth_key, auth_value)
            .json(&serde_json::json!({
                "name": "Operador 3",
                "email": "op3@loco.com",
                "password": "senha-sem-maiuscula1"
            }))
            .await;
        assert_eq!(sem_mai_res.status_code(), 422);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn reset_recusa_senha_sem_maiuscula() {
    request::<App, _, _>(|request, ctx| async move {
        let user =
            prepare_data::create_user(&ctx, "reset_user", "reset_test@loco.com", "SenhaForte1")
                .await;
        let user = user
            .into_active_model()
            .set_forgot_password_sent(&ctx.db)
            .await
            .unwrap();

        let token = user.reset_token.unwrap();

        let response = request
            .post("/api/auth/reset")
            .json(&serde_json::json!({
                "token": token,
                "password": "sem-maiuscula123"
            }))
            .await;

        assert_eq!(response.status_code(), 422);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn auth_guard_bloqueia_usuario_desativado_em_rotas_de_negocio() {
    request::<App, _, _>(|request, ctx| async move {
        let user =
            prepare_data::create_user(&ctx, "bloqueado", "bloqueado@loco.com", "SenhaForte1").await;
        let jwt_cfg = ctx.config.get_jwt_config().unwrap();
        let token = user
            .generate_jwt(&jwt_cfg.secret, jwt_cfg.expiration)
            .unwrap();

        let (auth_key, auth_value) = prepare_data::auth_header(&token);

        // Ativo: acessa normalmente
        let res_ativa = request
            .get("/api/devices")
            .add_header(auth_key.clone(), auth_value.clone())
            .await;
        assert_eq!(res_ativa.status_code(), 200);

        // Desativa o usuário no banco
        let mut active = user.into_active_model();
        active.active = sea_orm::ActiveValue::Set(false);
        sea_orm::ActiveModelTrait::update(active, &ctx.db)
            .await
            .unwrap();

        // Imediatamente bloqueado mesmo com token JWT assinado e não expirado
        let res_inativa = request
            .get("/api/devices")
            .add_header(auth_key, auth_value)
            .await;
        assert_eq!(res_inativa.status_code(), 401);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn magic_link_recusa_usuario_desativado() {
    request::<App, _, _>(|request, ctx| async move {
        let user =
            prepare_data::create_user(&ctx, "magic_off", "magic_off@gmail.com", "SenhaForte1")
                .await;
        let user = user
            .into_active_model()
            .create_magic_link(&ctx.db)
            .await
            .unwrap();

        let token = user.magic_link_token.clone().unwrap();

        // Desativa o usuário
        let mut active = user.into_active_model();
        active.active = sea_orm::ActiveValue::Set(false);
        sea_orm::ActiveModelTrait::update(active, &ctx.db)
            .await
            .unwrap();

        // Tentativa de verificação com token de magic link deve ser recusada com 401
        let response = request.get(&format!("/api/auth/magic-link/{token}")).await;
        assert_eq!(response.status_code(), 401);
    })
    .await;
}
