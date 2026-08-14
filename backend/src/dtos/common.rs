//! DTOs comuns a toda a API.
//!
//! O envelope de paginação vive em [`crate::services::shared::pagination`]
//! (§5.4 é a seção normativa) e é reexportado aqui para os controllers
//! importarem tudo de `crate::dtos::common`, como manda a §4.
//!
//! **Destino dos bindings.** O `export_to` aponta para `../../frontend/src/bindings/`
//! — o frontend Vue real, na raiz do repositório. O scaffold apontava para
//! `../frontend/`, que resolve para `backend/frontend/` e não é consumido
//! por ninguém.

pub use crate::services::shared::pagination::{LucidMeta, LucidPage, MaybePaged};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Corpo de erro da API (§5.5).
///
/// É o espelho de `ApiErrorResponse` em `frontend/src/services/apiService.ts`:
/// o cliente lê `message` e, na ausência dela, junta `errors[].message`. Quem
/// produz esta forma em runtime é o `IntoResponse` de
/// [`crate::services::shared::errors::AppError`]; este struct existe para
/// documentar o contrato e gerar o binding TypeScript.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ApiError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiFieldError>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ApiFieldError {
    pub field: Option<String>,
    pub message: String,
}

/// Resposta de `GET /` (§5.6). Fora do prefixo `/api` e sem autenticação —
/// é o health check que o docker-compose usa.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ServiceInfo {
    pub status: String,
    pub service: String,
    pub version: String,
}
