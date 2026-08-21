use axum::http::{HeaderName, HeaderValue};
use backend::{
    mailers::auth::AuthMailer,
    models::{users, users::RegisterParams},
    views::auth::LoginResponse,
};
use loco_rs::{app::AppContext, TestServer};
use sea_orm::IntoActiveModel;

pub const USER_EMAIL: &str = "test@loco.com";
pub const USER_PASSWORD: &str = "User1234";

pub const OPERATOR_EMAIL: &str = "operator@loco.com";
pub const OPERATOR_PASSWORD: &str = "Operator-1234";

pub struct LoggedInUser {
    pub user: users::Model,
    pub token: String,
}

/// Cria um usuário direto pelo modelo — sem passar pelo HTTP.
///
/// `POST /api/auth/register` exige sessão desde que o cadastro deixou de ser
/// aberto (o primeiro usuário nasce em `/auth/setup`, com token de instalação).
/// Um teste que precisa de um usuário para *poder* chamar a API não tem como
/// tirá-lo da própria API sem cair nesse círculo, então ele entra por baixo.
pub async fn create_user(
    ctx: &AppContext,
    name: &str,
    email: &str,
    password: &str,
) -> users::Model {
    users::Model::create_with_password(
        &ctx.db,
        &RegisterParams {
            name: name.to_string(),
            email: email.to_string(),
            password: password.to_string(),
        },
    )
    .await
    .expect("criar usuário de teste")
}

fn generate_token(ctx: &AppContext, user: &users::Model) -> String {
    let jwt = ctx.config.get_jwt_config().expect("config de jwt");
    user.generate_jwt(&jwt.secret, jwt.expiration)
        .expect("gerar jwt")
}

/// Operador já autenticado, para os testes que exercitam rotas protegidas.
///
/// Não dispara e-mail: os testes que contam entregas do mailer precisam que
/// só o fluxo sob teste apareça na contagem.
pub async fn init_operator(ctx: &AppContext) -> LoggedInUser {
    let user = create_user(ctx, "operator", OPERATOR_EMAIL, OPERATOR_PASSWORD).await;
    let token = generate_token(ctx, &user);
    LoggedInUser { user, token }
}

/// Usuário de teste no mesmo estado em que o `register` o deixaria: token de
/// verificação emitido e e-mail de boas-vindas entregue.
pub async fn init_user_login(request: &TestServer, ctx: &AppContext) -> LoggedInUser {
    let user = create_user(ctx, "loco", USER_EMAIL, USER_PASSWORD).await;

    let user = user
        .into_active_model()
        .set_email_verification_sent(&ctx.db)
        .await
        .expect("emitir token de verificação");
    AuthMailer::send_welcome(ctx, &user)
        .await
        .expect("enviar e-mail de boas-vindas");

    let response = request
        .post("/api/auth/login")
        .json(&serde_json::json!({
            "email": USER_EMAIL,
            "password": USER_PASSWORD
        }))
        .await;

    let login_response: LoginResponse = serde_json::from_str(&response.text()).unwrap();

    LoggedInUser {
        user: users::Model::find_by_email(&ctx.db, USER_EMAIL)
            .await
            .unwrap(),
        token: login_response.token,
    }
}

pub fn auth_header(token: &str) -> (HeaderName, HeaderValue) {
    let auth_header_value = HeaderValue::from_str(&format!("Bearer {}", &token)).unwrap();

    (HeaderName::from_static("authorization"), auth_header_value)
}
