use crate::{
    mailers::auth::AuthMailer,
    models::{
        _entities::users,
        users::{LoginParams, RegisterParams},
    },
    services::{
        auth::setup::{map_model_error, SetupParams, SetupService},
        shared::errors::{AppError, AppResult},
    },
    views::auth::{LoginResponse, SetupStatusResponse, UserResponse},
};
use loco_rs::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub static EMAIL_DOMAIN_RE: OnceLock<Regex> = OnceLock::new();

fn get_allow_email_domain_re() -> &'static Regex {
    EMAIL_DOMAIN_RE.get_or_init(|| {
        Regex::new(r"@example\.com$|@gmail\.com$").expect("Failed to compile regex")
    })
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForgotParams {
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResetParams {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MagicLinkParams {
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResendVerificationParams {
    pub email: String,
}

/// Diz se a instalação ainda espera o primeiro usuário.
///
/// Público de propósito — é a pergunta que o frontend faz **antes** de ter
/// qualquer credencial, para escolher entre a tela de login e a de cadastro
/// inicial. Não revela nada além de um booleano que qualquer um descobriria
/// tentando entrar.
#[debug_handler]
async fn setup_status(State(ctx): State<AppContext>) -> AppResult<Response> {
    let needs_setup = SetupService::new(&ctx.db).is_pending().await?;
    Ok(format::json(SetupStatusResponse { needs_setup })?)
}

/// Cadastra o primeiro usuário e já devolve a sessão.
///
/// Devolver o `LoginResponse` (em vez de mandar o operador para a tela de
/// login) fecha o fluxo numa tela só: quem acabou de provar a posse do token de
/// instalação não tem o que reprovar num login logo em seguida.
#[debug_handler]
async fn setup(
    State(ctx): State<AppContext>,
    Json(params): Json<SetupParams>,
) -> AppResult<Response> {
    let user = SetupService::new(&ctx.db).complete(&params).await?;
    Ok(format::json(issue_session(&ctx, &user)?)?)
}

/// Cria um usuário adicional.
///
/// **Exige sessão válida.** Cadastro aberto transformaria o token de instalação
/// em teatro: bastaria pular a tela de setup e chamar este endpoint para entrar
/// num sistema que enxerga a rede inteira. O primeiro usuário nasce em
/// `POST /auth/setup`; daí em diante quem já está dentro cria os demais.
#[debug_handler]
async fn register(
    _auth: auth::JWT,
    State(ctx): State<AppContext>,
    Json(params): Json<RegisterParams>,
) -> AppResult<Response> {
    let user = users::Model::create_with_password(&ctx.db, &params)
        .await
        .map_err(map_model_error)?;

    let user = user
        .into_active_model()
        .set_email_verification_sent(&ctx.db)
        .await
        .map_err(map_model_error)?;

    AuthMailer::send_welcome(&ctx, &user).await?;

    tracing::info!(user_pid = %user.pid, "usuário criado por um operador autenticado");

    Ok(format::json(UserResponse::new(&user))?)
}

/// Verify register user. if the user not verified his email, he can't login to
/// the system.
#[debug_handler]
async fn verify(State(ctx): State<AppContext>, Path(token): Path<String>) -> Result<Response> {
    let Ok(user) = users::Model::find_by_verification_token(&ctx.db, &token).await else {
        return unauthorized("invalid token");
    };

    if user.email_verified_at.is_some() {
        tracing::info!(pid = user.pid.to_string(), "user already verified");
    } else {
        let active_model = user.into_active_model();
        let user = active_model.verified(&ctx.db).await?;
        tracing::info!(pid = user.pid.to_string(), "user verified");
    }

    format::json(())
}

/// In case the user forgot his password  this endpoints generate a forgot token
/// and send email to the user. In case the email not found in our DB, we are
/// returning a valid request for for security reasons (not exposing users DB
/// list).
#[debug_handler]
async fn forgot(
    State(ctx): State<AppContext>,
    Json(params): Json<ForgotParams>,
) -> Result<Response> {
    let Ok(user) = users::Model::find_by_email(&ctx.db, &params.email).await else {
        // we don't want to expose our users email. if the email is invalid we still
        // returning success to the caller
        return format::json(());
    };

    let user = user
        .into_active_model()
        .set_forgot_password_sent(&ctx.db)
        .await?;

    AuthMailer::forgot_password(&ctx, &user).await?;

    format::json(())
}

/// reset user password by the given parameters
#[debug_handler]
async fn reset(State(ctx): State<AppContext>, Json(params): Json<ResetParams>) -> Result<Response> {
    let Ok(user) = users::Model::find_by_reset_token(&ctx.db, &params.token).await else {
        // we don't want to expose our users email. if the email is invalid we still
        // returning success to the caller
        tracing::info!("reset token not found");

        return format::json(());
    };
    user.into_active_model()
        .reset_password(&ctx.db, &params.password)
        .await?;

    format::json(())
}

/// Emite o JWT do usuário e monta a resposta de sessão.
///
/// Existe fora dos handlers porque três caminhos chegam ao mesmo lugar — login,
/// magic link e cadastro inicial. Duplicar a leitura do `jwt` de configuração
/// em cada um é como uma expiração passa a divergir da outra sem ninguém notar.
fn issue_session(ctx: &AppContext, user: &users::Model) -> AppResult<LoginResponse> {
    let jwt = ctx.config.get_jwt_config().map_err(AppError::from)?;
    let token = user
        .generate_jwt(&jwt.secret, jwt.expiration)
        .map_err(map_model_error)?;
    Ok(LoginResponse::new(user, &token))
}

/// Autentica por e-mail e senha.
///
/// Todas as recusas devolvem a **mesma** mensagem: distinguir "e-mail não
/// existe" de "senha errada" entrega ao atacante a lista de quem tem conta, que
/// é metade do trabalho de invadir uma. O motivo real fica no log.
#[debug_handler]
async fn login(
    State(ctx): State<AppContext>,
    Json(params): Json<LoginParams>,
) -> AppResult<Response> {
    const RECUSA: &str = "E-mail ou senha inválidos.";

    let email = params.email.trim().to_lowercase();
    let Ok(user) = users::Model::find_by_email(&ctx.db, &email).await else {
        tracing::debug!(email, "login recusado: e-mail sem cadastro");
        return Err(AppError::unauthorized(RECUSA));
    };

    if !user.verify_password(&params.password) {
        tracing::debug!(email, "login recusado: senha incorreta");
        return Err(AppError::unauthorized(RECUSA));
    }

    // Desativar um operador precisa valer imediatamente. Sem esta checagem a
    // coluna `active` seria decorativa: quem foi desligado continuaria
    // entrando.
    if !user.active {
        tracing::info!(user_pid = %user.pid, "login recusado: usuário desativado");
        return Err(AppError::unauthorized(
            "Este usuário está desativado. Procure um administrador.",
        ));
    }

    Ok(format::json(issue_session(&ctx, &user)?)?)
}

#[debug_handler]
async fn current(auth: auth::JWT, State(ctx): State<AppContext>) -> Result<Response> {
    let user = users::Model::find_by_pid(&ctx.db, &auth.claims.pid).await?;
    format::json(UserResponse::new(&user))
}

/// JWT é stateless: o logout confirma a operação e o cliente descarta o token.
#[debug_handler]
async fn logout(_auth: auth::JWT, State(_ctx): State<AppContext>) -> Result<Response> {
    format::json(serde_json::json!({ "message": "Sessão encerrada com sucesso" }))
}

/// Magic link authentication provides a secure and passwordless way to log in to the application.
///
/// # Flow
/// 1. **Request a Magic Link**:
///    A registered user sends a POST request to `/magic-link` with their email.
///    If the email exists, a short-lived, one-time-use token is generated and sent to the user's email.
///    For security and to avoid exposing whether an email exists, the response always returns 200, even if the email is invalid.
///
/// 2. **Click the Magic Link**:
///    The user clicks the link (/magic-link/{token}), which validates the token and its expiration.
///    If valid, the server generates a JWT and responds with a [`LoginResponse`].
///    If invalid or expired, an unauthorized response is returned.
///
/// This flow enhances security by avoiding traditional passwords and providing a seamless login experience.
async fn magic_link(
    State(ctx): State<AppContext>,
    Json(params): Json<MagicLinkParams>,
) -> Result<Response> {
    let email_regex = get_allow_email_domain_re();
    if !email_regex.is_match(&params.email) {
        tracing::debug!(
            email = params.email,
            "The provided email is invalid or does not match the allowed domains"
        );
        return bad_request("invalid request");
    }

    let Ok(user) = users::Model::find_by_email(&ctx.db, &params.email).await else {
        // we don't want to expose our users email. if the email is invalid we still
        // returning success to the caller
        tracing::debug!(email = params.email, "user not found by email");
        return format::empty_json();
    };

    let user = user.into_active_model().create_magic_link(&ctx.db).await?;
    AuthMailer::send_magic_link(&ctx, &user).await?;

    format::empty_json()
}

/// Verifies a magic link token and authenticates the user.
async fn magic_link_verify(
    Path(token): Path<String>,
    State(ctx): State<AppContext>,
) -> AppResult<Response> {
    let Ok(user) = users::Model::find_by_magic_token(&ctx.db, &token).await else {
        // we don't want to expose our users email. if the email is invalid we still
        // returning success to the caller
        return Err(AppError::unauthorized("Link inválido ou expirado."));
    };

    let user = user
        .into_active_model()
        .clear_magic_link(&ctx.db)
        .await
        .map_err(map_model_error)?;

    Ok(format::json(issue_session(&ctx, &user)?)?)
}

#[debug_handler]
async fn resend_verification_email(
    State(ctx): State<AppContext>,
    Json(params): Json<ResendVerificationParams>,
) -> Result<Response> {
    let Ok(user) = users::Model::find_by_email(&ctx.db, &params.email).await else {
        tracing::info!(
            email = params.email,
            "User not found for resend verification"
        );
        return format::json(());
    };

    if user.email_verified_at.is_some() {
        tracing::info!(
            pid = user.pid.to_string(),
            "User already verified, skipping resend"
        );
        return format::json(());
    }

    let user = user
        .into_active_model()
        .set_email_verification_sent(&ctx.db)
        .await?;

    AuthMailer::send_welcome(&ctx, &user).await?;
    tracing::info!(pid = user.pid.to_string(), "Verification email re-sent");

    format::json(())
}

pub fn routes() -> Routes {
    // Prefixo relativo: o `/api` vem do `AppRoutes::prefix` em `app.rs`, para o
    // grupo de negócio inteiro ter uma origem única (§5.6). O scaffold trazia
    // `/api/auth` embutido, o que duplicaria o prefixo.
    Routes::new()
        .prefix("/auth")
        // Instalação: a única rota que serve para algo antes de haver usuário.
        .add("/setup", get(setup_status).post(setup))
        .add("/register", post(register))
        .add("/verify/{token}", get(verify))
        .add("/login", post(login))
        .add("/forgot", post(forgot))
        .add("/reset", post(reset))
        .add("/current", get(current))
        .add("/me", get(current))
        .add("/logout", post(logout))
        .add("/magic-link", post(magic_link))
        .add("/magic-link/{token}", get(magic_link_verify))
        .add("/resend-verification-mail", post(resend_verification_email))
}
