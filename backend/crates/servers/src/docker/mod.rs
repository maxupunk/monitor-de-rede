//! Integração com a Docker Engine.
//!
//! Esta é a única fronteira do sistema que conhece `bollard`. Controllers
//! trabalham apenas com DTOs da aplicação e nunca executam o binário `docker`.
//!
//! O código daqui que fala com a Engine ([`source::LocalEngine`] e os módulos
//! que ele usa) roda **no agente** — o remoto e o local, ao lado da central
//! (ADR 014). O processo da API não monta o socket: vê todo host Docker pelo
//! canal do agente ([`hosts`]).

pub mod cli;
pub mod compose;
pub mod engine;
pub mod hosts;
pub mod log_clear;
pub mod log_stream;
pub mod maintenance;
pub mod metrics;
pub mod operations;
pub mod protection;
pub mod realtime;
pub mod remote_engine;
pub mod source;
pub mod stacks;
pub mod update;
pub mod volume_export;

use std::{future::Future, time::Duration};

use bollard::Docker;

use netmonitor_core::shared::errors::{AppError, AppResult};

const OPERATION_TIMEOUT: Duration = Duration::from_secs(12);

pub const DISABLED_REASON: &str =
    "Integração Docker desativada: DOCKER_ENABLED=false no ambiente do agente";
pub const UNAVAILABLE_REASON: &str = "O agente não conseguiu acessar a Docker Engine. A Engine pode estar desligada, o socket pode não estar montado no container do agente ou ele pode não ter permissão";

pub use agent_protocol::docker::DockerError;

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
