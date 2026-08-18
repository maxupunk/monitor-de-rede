//! Erro de domínio e sua tradução para HTTP (§5.5 do roadmap).
//!
//! **Por que não usar `loco_rs::Error` direto nos handlers.** O `IntoResponse`
//! do Loco serializa `{"error": "...", "description": "..."}`. O frontend lê
//! `message` (ou `errors[].message`) em `apiService.handleResponse` — com o
//! corpo do Loco, toda falha viraria o texto genérico
//! `"Erro HTTP 422: Unprocessable Entity"` nos snackbars. Como o contrato HTTP
//! é sagrado (§1.3.1), o corpo de erro é nosso.
//!
//! `AppError` continua conversível nos dois sentidos: `From<loco_rs::Error>`
//! para o `?` funcionar sobre chamadas do framework dentro de um handler, e
//! `From<AppError> for loco_rs::Error` para o serviço ser reaproveitado em
//! tasks e workers, onde quem manda é a assinatura do Loco.

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use loco_rs::controller::ErrorDetail;
use serde::Serialize;

/// Um erro de validação atrelado a um campo do payload.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 422 com mensagem única.
    #[error("{0}")]
    Validation(String),

    /// 422 com erros por campo — o formato `{"errors":[{"field","message"}]}`
    /// que o `apiService` junta com vírgula quando não há `message`.
    #[error("{}", .0.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join(", "))]
    ValidationFields(Vec<FieldError>),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    Unauthorized(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("{msg}")]
    RateLimited { msg: String, retry_after: u64 },

    /// 400 — regra de negócio violada com o dado já validado.
    #[error("{0}")]
    BusinessRule(String),

    /// 500 — nunca vaza detalhe para o cliente; o detalhe vai para o log.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Validation(_) | Self::ValidationFields(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::BusinessRule(_) => StatusCode::BAD_REQUEST,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Atalhos para o caso comum de construir a partir de um `&str`.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }
    pub fn business_rule(msg: impl Into<String>) -> Self {
        Self::BusinessRule(msg.into())
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BusinessRule(msg.into())
    }
    pub fn rate_limited(msg: impl Into<String>, retry_after: u64) -> Self {
        Self::RateLimited {
            msg: msg.into(),
            retry_after,
        }
    }
}

/// Corpo de erro que o frontend entende. `errors` só aparece na variante por
/// campo — `skip_serializing_if` evita mandar `null` e confundir o cliente.
#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<&'a [FieldError]>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();

        // O detalhe do 500 fica no log; o cliente recebe texto genérico.
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error.msg = %self, error.details = ?self, "app_error");
        } else {
            tracing::debug!(error.msg = %self, status = %status, "app_error");
        }

        let (message, errors) = match &self {
            Self::ValidationFields(fields) => (self.to_string(), Some(fields.as_slice())),
            Self::Internal(_) => ("Erro interno do servidor".to_string(), None),
            _ => (self.to_string(), None),
        };

        let mut response = (status, Json(ErrorBody { message, errors })).into_response();

        if let Self::RateLimited { retry_after, .. } = &self {
            if let Ok(value) = retry_after.to_string().parse() {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }

        response
    }
}

/// Extrai a mensagem de um erro sem estourar quando ele não tem uma.
///
/// Equivalente ao padrão de apresentação de erro do sistema anterior; a regra
/// de negócio é a mesma, só mudou a implementação.
#[must_use]
pub fn error_message(err: &dyn std::error::Error) -> String {
    let msg = err.to_string();
    if msg.is_empty() {
        err.source()
            .map_or_else(|| "Erro desconhecido".to_string(), ToString::to_string)
    } else {
        msg
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(err: sea_orm::DbErr) -> Self {
        // `RecordNotFound` chega de `find_by_id(...).one()?.ok_or(...)` mal
        // escrito e de alguns `update`; traduzir aqui evita 500 espúrio.
        match err {
            sea_orm::DbErr::RecordNotFound(msg) => Self::NotFound(msg),
            other => Self::Internal(other.into()),
        }
    }
}

impl From<loco_rs::Error> for AppError {
    fn from(err: loco_rs::Error) -> Self {
        use loco_rs::Error as L;
        match err {
            L::NotFound => Self::NotFound("Recurso não encontrado".to_string()),
            L::Unauthorized(msg) => Self::Unauthorized(msg),
            L::BadRequest(msg) => Self::BusinessRule(msg),
            L::Message(msg) => Self::BusinessRule(msg),
            other => Self::Internal(anyhow::Error::new(other)),
        }
    }
}

impl From<AppError> for loco_rs::Error {
    /// Usado quando o serviço é chamado de uma task/worker, cuja assinatura é
    /// `loco_rs::Result`. `CustomError` preserva o status; o corpo aqui não
    /// importa porque esse caminho não atende requisição HTTP.
    fn from(err: AppError) -> Self {
        let status = err.status();
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            return Self::wrap(err);
        }
        Self::CustomError(status, ErrorDetail::with_reason(err.to_string()))
    }
}

/// Converte os erros do `validator` no formato por campo do §5.5.
impl From<validator::ValidationErrors> for AppError {
    fn from(errors: validator::ValidationErrors) -> Self {
        let fields = errors
            .field_errors()
            .iter()
            .flat_map(|(field, errs)| {
                errs.iter().map(move |e| FieldError {
                    field: (*field).to_string(),
                    // `message` é o texto em português declarado no DTO; o
                    // fallback é o código da regra (`length`, `range`, …).
                    message: e
                        .message
                        .as_ref()
                        .map_or_else(|| e.code.to_string(), ToString::to_string),
                })
            })
            .collect();
        Self::ValidationFields(fields)
    }
}

/// Alias de conveniência para as funções de serviço.
pub type AppResult<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn body_of(err: AppError) -> (StatusCode, Option<String>, serde_json::Value) {
        let response = err.into_response();
        let status = response.status();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .map(|v| v.to_str().unwrap().to_string());
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (
            status,
            retry_after,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    #[tokio::test]
    async fn mapeia_cada_variante_para_o_status_da_tabela_5_5() {
        let cases = vec![
            (AppError::validation("x"), 422),
            (AppError::not_found("x"), 404),
            (AppError::conflict("x"), 409),
            (AppError::unauthorized("x"), 401),
            (AppError::forbidden("x"), 403),
            (AppError::business_rule("x"), 400),
            (AppError::rate_limited("x", 30), 429),
            (AppError::Internal(anyhow::anyhow!("x")), 500),
        ];
        for (err, expected) in cases {
            let (status, _, _) = body_of(err).await;
            assert_eq!(status.as_u16(), expected);
        }
    }

    #[tokio::test]
    async fn corpo_tem_message_que_o_apiservice_le() {
        let (_, _, body) = body_of(AppError::not_found("Dispositivo não encontrado")).await;
        assert_eq!(body["message"], "Dispositivo não encontrado");
        assert!(body.get("errors").is_none());
    }

    #[tokio::test]
    async fn erro_por_campo_expoe_errors_e_message_agregada() {
        let err = AppError::ValidationFields(vec![
            FieldError {
                field: "cidr".into(),
                message: "CIDR inválido".into(),
            },
            FieldError {
                field: "name".into(),
                message: "Nome obrigatório".into(),
            },
        ]);
        let (status, _, body) = body_of(err).await;
        assert_eq!(status.as_u16(), 422);
        assert_eq!(body["errors"][0]["field"], "cidr");
        assert_eq!(body["errors"][1]["message"], "Nome obrigatório");
        // O `apiService` prefere `message` quando ele existe.
        assert_eq!(body["message"], "CIDR inválido, Nome obrigatório");
    }

    #[tokio::test]
    async fn rate_limited_manda_retry_after() {
        let (status, retry_after, body) =
            body_of(AppError::rate_limited("Muitas requisições", 42)).await;
        assert_eq!(status.as_u16(), 429);
        assert_eq!(retry_after.as_deref(), Some("42"));
        assert_eq!(body["message"], "Muitas requisições");
    }

    #[tokio::test]
    async fn internal_nao_vaza_detalhe() {
        let (_, _, body) = body_of(AppError::Internal(anyhow::anyhow!(
            "connection string postgres://user:senha@host"
        )))
        .await;
        assert_eq!(body["message"], "Erro interno do servidor");
    }

    #[test]
    fn db_err_record_not_found_vira_404() {
        let err: AppError = sea_orm::DbErr::RecordNotFound("sumiu".into()).into();
        assert_eq!(err.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn error_message_usa_o_source_quando_a_mensagem_e_vazia() {
        #[derive(Debug)]
        struct Vazio(std::io::Error);
        impl std::fmt::Display for Vazio {
            fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Ok(())
            }
        }
        impl std::error::Error for Vazio {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }
        let inner = std::io::Error::new(std::io::ErrorKind::TimedOut, "tempo esgotado");
        assert_eq!(error_message(&Vazio(inner)), "tempo esgotado");
        assert_eq!(
            error_message(&AppError::validation("CIDR inválido")),
            "CIDR inválido"
        );
    }
}
