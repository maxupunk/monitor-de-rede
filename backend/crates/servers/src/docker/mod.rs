//! Integração com a Docker Engine.
//!
//! Esta é a única fronteira do sistema que conhece `bollard`. Controllers
//! trabalham apenas com DTOs da aplicação e nunca executam o binário `docker`.
//!
//! O código daqui que fala com a Engine ([`source::LocalEngine`] e os módulos
//! que ele usa) roda **no agente** — o remoto e o local, ao lado da central
//! (ADR 014). O processo da API não monta o socket: vê todo host Docker pelo
//! canal do agente ([`hosts`]).

pub mod builds;
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

#[cfg(not(test))]
use std::sync::Mutex;
use std::{future::Future, time::Duration};

use bollard::Docker;

use netmonitor_core::shared::errors::{AppError, AppResult};

/// Um cliente por processo: o `bollard` mantém um pool de conexões preso ao
/// runtime que as abriu. Em teste cada `#[tokio::test]` tem o seu runtime, e
/// um cliente cacheado de um teste encerrado falharia no seguinte — lá não se
/// cacheia.
#[cfg(not(test))]
static CACHED_CLIENT: Mutex<Option<Docker>> = Mutex::new(None);

const OPERATION_TIMEOUT: Duration = Duration::from_secs(12);

/// Iniciar, parar, reiniciar e remover um container. O `stop` espera o
/// `stop_grace_period` do próprio container (10 s por padrão, e o compose
/// costuma aumentar) antes de matar o processo: com o teto de 12 s das
/// consultas, a Engine seguia parando o container enquanto a tela recebia
/// "Engine indisponível".
pub const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(3 * 60);

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
    cached_or_connect()
}

#[cfg(not(test))]
fn cached_or_connect() -> Result<Docker, DockerError> {
    let mut guard = CACHED_CLIENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(docker) = guard.as_ref() {
        return Ok(docker.clone());
    }
    let docker = connect()?;
    *guard = Some(docker.clone());
    Ok(docker)
}

#[cfg(test)]
fn cached_or_connect() -> Result<Docker, DockerError> {
    connect()
}

fn connect() -> Result<Docker, DockerError> {
    Docker::connect_with_local_defaults().map_err(|error| {
        tracing::debug!(%error, "falha ao conectar ao socket da Docker Engine");
        DockerError::Unavailable
    })
}

pub(crate) async fn call<T>(
    future: impl Future<Output = Result<T, bollard::errors::Error>>,
) -> Result<T, DockerError> {
    call_within(OPERATION_TIMEOUT, future).await
}

/// [`call`] com outro teto — o das ações de ciclo de vida.
pub(crate) async fn call_within<T>(
    timeout: Duration,
    future: impl Future<Output = Result<T, bollard::errors::Error>>,
) -> Result<T, DockerError> {
    match tokio::time::timeout(timeout, future).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(map_engine_error(&error)),
        Err(_) => Err(DockerError::Unavailable),
    }
}

fn map_engine_error(error: &bollard::errors::Error) -> DockerError {
    if let bollard::errors::Error::DockerResponseServerError {
        status_code,
        message,
    } = error
    {
        tracing::debug!(status_code, %message, "Docker Engine retornou erro HTTP");
        return match *status_code {
            404 => DockerError::NotFound,
            409 => DockerError::Conflict,
            _ => DockerError::Engine,
        };
    }
    tracing::debug!(%error, "falha de transporte com a Docker Engine");
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
