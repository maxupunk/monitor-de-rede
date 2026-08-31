//! Integração com a Docker Engine.
//!
//! Esta é a única fronteira do sistema que conhece `bollard`. Controllers
//! trabalham apenas com DTOs da aplicação e nunca executam o binário `docker`.

pub mod engine;
pub mod metrics;
pub mod volume_export;

use std::{future::Future, time::Duration};

use bollard::Docker;
use loco_rs::app::AppContext;

use crate::services::shared::errors::{AppError, AppResult};

const OPERATION_TIMEOUT: Duration = Duration::from_secs(12);

pub const DISABLED_REASON: &str =
    "Integração Docker desativada: DOCKER_ENABLED=false no ambiente do backend";
pub const UNAVAILABLE_REASON: &str = "O backend não conseguiu acessar a Docker Engine. A Engine pode estar desligada, o socket pode não estar montado ou o usuário da aplicação pode não ter permissão";

#[derive(Debug, thiserror::Error)]
pub enum DockerError {
    #[error("Docker está desabilitado nesta instalação")]
    Disabled,
    #[error("Docker Engine indisponível")]
    Unavailable,
    #[error("Recurso Docker não encontrado")]
    NotFound,
    #[error("A operação conflita com o estado atual do recurso Docker")]
    Conflict,
    #[error("{0}")]
    Validation(String),
    #[error("Falha ao comunicar com a Docker Engine")]
    Engine,
}

impl From<DockerError> for AppError {
    fn from(error: DockerError) -> Self {
        match error {
            DockerError::Disabled | DockerError::Unavailable => {
                Self::service_unavailable(error.to_string())
            }
            DockerError::NotFound => Self::not_found(error.to_string()),
            DockerError::Conflict => Self::conflict(error.to_string()),
            DockerError::Validation(message) => Self::validation(message),
            DockerError::Engine => Self::Internal(anyhow::anyhow!(error)),
        }
    }
}

#[must_use]
pub fn enabled() -> bool {
    !std::env::var("DOCKER_ENABLED").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "false" | "0" | "no" | "off"
        )
    })
}

pub fn client() -> Result<Docker, DockerError> {
    if !enabled() {
        return Err(DockerError::Disabled);
    }
    Docker::connect_with_local_defaults().map_err(|_| DockerError::Unavailable)
}

pub fn install(ctx: &AppContext) {
    metrics::install(ctx);
}

pub(crate) async fn call<T>(
    future: impl Future<Output = Result<T, bollard::errors::Error>>,
) -> Result<T, DockerError> {
    match tokio::time::timeout(OPERATION_TIMEOUT, future).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(map_engine_error(&error)),
        Err(_) => Err(DockerError::Unavailable),
    }
}

fn map_engine_error(error: &bollard::errors::Error) -> DockerError {
    if let bollard::errors::Error::DockerResponseServerError { status_code, .. } = error {
        return match *status_code {
            404 => DockerError::NotFound,
            409 => DockerError::Conflict,
            _ => DockerError::Engine,
        };
    }
    // Falhas de transporte (socket/pipe inexistente, conexão recusada ou
    // encerrada) significam Engine indisponível. Erros HTTP da Engine foram
    // tratados acima e continuam distinguindo conflito, ausência e 5xx.
    DockerError::Unavailable
}

pub fn validate_identifier(value: &str, label: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 255 || value.chars().any(char::is_control) {
        return Err(AppError::validation(format!("{label} inválido")));
    }
    Ok(value.to_string())
}
