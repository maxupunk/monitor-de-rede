//! Corpo de requisição: o JSON obrigatório com erro legível ([`AppJson`]) e o
//! opcional ([`optional_body`]).

use axum::extract::{FromRequest, Request};

use super::errors::AppError;

/// O `Json` do axum com a rejeição em [`AppError`]: um campo que o DTO não
/// aceita volta como `{"message": "Dados inválidos na requisição: <campo>: <motivo>"}`
/// em vez do `{"error":"Bad Request"}` mudo do `Json` do Loco.
#[derive(Debug, Clone, Copy, Default)]
pub struct AppJson<T>(pub T);

impl<T, S> FromRequest<S> for AppJson<T>
where
    axum::Json<T>: FromRequest<S, Rejection = axum::extract::rejection::JsonRejection>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let axum::Json(value) = axum::Json::<T>::from_request(req, state).await?;
        Ok(Self(value))
    }
}

/// Lê um corpo de requisição **opcional** sem deixá-lo mascarar o erro real.
///
/// O extrator `Json` do axum rejeita com **400** um corpo vazio ou sem
/// `Content-Type: application/json`, e essa rejeição acontece *antes* do
/// handler. Nos endpoints em que todo campo tem default — o heartbeat do probe,
/// o silenciamento de alerta — isso trocaria o 401/404 correto por um
/// "requisição malformada" que não diz nada a quem chamou. Aqui o corpo chega
/// como texto (`body: String`), um payload ausente ou ilegível vira o `Default`
/// do DTO, e a autenticação ou a busca do recurso voltam a ser a primeira coisa
/// a falhar.
///
/// Só vale para DTOs cujos campos são todos opcionais. Onde há campo
/// obrigatório, use `Json<T>` e deixe o 400 acontecer.
pub fn optional_body<T: serde::de::DeserializeOwned + Default>(body: &str) -> T {
    if body.trim().is_empty() {
        return T::default();
    }
    serde_json::from_str(body).unwrap_or_default()
}
