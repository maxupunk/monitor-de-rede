//! `GET /` — identificação do serviço (§5.6).
//!
//! Fica **fora** do prefixo `/api` e fora da autenticação: é o mesmo endpoint
//! que o `docker-compose` usa como health check e que o AdonisJS servia em
//! `start/routes.ts`. O corpo é literal, incluindo a versão `1.0.0` — não é a
//! versão do crate; é o número que o backend atual publica, e mudá-lo alteraria
//! um payload observável.

use loco_rs::prelude::*;

use crate::dtos::common::ServiceInfo;

pub const SERVICE_NAME: &str = "Network Monitor API";
pub const API_VERSION: &str = "1.0.0";

#[debug_handler]
pub async fn index() -> Result<Response> {
    format::json(ServiceInfo {
        status: "online".to_string(),
        service: SERVICE_NAME.to_string(),
        version: API_VERSION.to_string(),
    })
}

pub fn routes() -> Routes {
    Routes::new().add("/", get(index))
}
