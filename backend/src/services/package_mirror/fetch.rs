//! Busca na origem. Um cliente HTTP para o processo inteiro (§11: nada de
//! cliente novo por requisição), com prazo para conectar e para cada leitura —
//! um índice grande num link lento continua correndo enquanto chega dado.

use std::{sync::OnceLock, time::Duration};

use crate::services::shared::errors::{AppError, AppResult};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Maior arquivo que o espelho repassa (o índice `packages` do apk tem
/// poucos MB; pacote de firmware inteiro não passa por aqui).
pub const MAX_BYTES: u64 = 128 * 1024 * 1024;

/// Resposta da origem pronta para ser repassada.
pub struct Upstream {
    pub content_type: String,
    pub content_length: Option<u64>,
    pub response: reqwest::Response,
}

fn client() -> AppResult<&'static reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .user_agent(concat!(
            "NetMonitor/",
            env!("CARGO_PKG_VERSION"),
            " package-mirror"
        ))
        .build()
        .map_err(|error| AppError::service_unavailable(format!("cliente HTTP: {error}")))?;
    Ok(CLIENT.get_or_init(|| client))
}

/// Pede `url` à origem.
///
/// # Errors
///
/// `not_found` quando a origem não tem o arquivo; `service_unavailable` quando
/// a central não alcança a origem, ela responde erro ou o arquivo passa de
/// [`MAX_BYTES`].
pub async fn fetch(url: &str) -> AppResult<Upstream> {
    let response = client()?.get(url).send().await.map_err(|error| {
        AppError::service_unavailable(format!(
            "A central não alcançou o repositório do OpenWrt: {error}"
        ))
    })?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::not_found(
            "O repositório do OpenWrt não tem este arquivo.",
        ));
    }
    if !status.is_success() {
        return Err(AppError::service_unavailable(format!(
            "O repositório do OpenWrt respondeu {status}."
        )));
    }
    let content_length = response.content_length();
    if content_length.is_some_and(|length| length > MAX_BYTES) {
        return Err(AppError::service_unavailable(
            "Arquivo grande demais para o espelho de pacotes.",
        ));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    Ok(Upstream {
        content_type,
        content_length,
        response,
    })
}
