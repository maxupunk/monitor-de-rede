//! Cliente HTTP de envio de notificações Web Push para serviços remotos (RFC 8030).

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_ENCODING, CONTENT_TYPE};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, warn};

use super::crypto::{build_vapid_header, encrypt_payload, SubscriptionKeys, VapidKeyPair};
use crate::services::shared::errors::{AppError, AppResult};

/// Desfecho do envio de uma notificação Web Push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    /// Entregue com sucesso ao Push Service do navegador (201 Created / 200 OK / 202 Accepted).
    Success,
    /// A subscrição expirou ou foi cancelada pelo operador (404 Not Found ou 410 Gone).
    Expired,
}

/// Envia uma notificação Web Push encriptada com VAPID para o endpoint do navegador.
///
/// # Errors
///
/// Retorna erro em caso de falha de cifragem, cabeçalho VAPID inválido ou erro de transporte HTTP.
pub async fn send_push(
    endpoint: &str,
    subscription: &SubscriptionKeys,
    vapid: &VapidKeyPair,
    payload: &Value,
) -> AppResult<PushOutcome> {
    let payload_bytes = serde_json::to_vec(payload)
        .map_err(|e| AppError::bad_request(format!("Erro ao serializar payload JSON: {e}")))?;

    let encrypted_body = encrypt_payload(subscription, &payload_bytes)?;

    let now = chrono::Utc::now().timestamp();
    let expiration = now + 12 * 3600; // 12 horas de validade no token JWT VAPID
    let auth_header = build_vapid_header(vapid, endpoint, expiration)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(CONTENT_ENCODING, HeaderValue::from_static("aes128gcm"));
    headers.insert("TTL", HeaderValue::from_static("86400"));
    headers.insert("Urgency", HeaderValue::from_static("high"));
    headers.insert(
        reqwest::header::AUTHORIZATION,
        HeaderValue::from_str(&auth_header)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Header VAPID inválido: {e}")))?,
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Erro ao criar cliente HTTP: {e}")))?;

    let res = client
        .post(endpoint)
        .headers(headers)
        .body(encrypted_body)
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Erro de rede ao enviar Web Push: {e}")))?;

    let status = res.status();
    if status.is_success() {
        debug!(endpoint, %status, "Web Push entregue ao Push Service com sucesso");
        Ok(PushOutcome::Success)
    } else if status == reqwest::StatusCode::GONE || status == reqwest::StatusCode::NOT_FOUND {
        debug!(endpoint, %status, "Subscrição Web Push expirada ou cancelada");
        Ok(PushOutcome::Expired)
    } else {
        let err_body = res.text().await.unwrap_or_default();
        warn!(endpoint, %status, err_body = %err_body, "Push Service rejeitou a requisição");
        Err(AppError::Internal(anyhow::anyhow!(
            "Push Service retornou status {}: {}",
            status,
            err_body
        )))
    }
}
