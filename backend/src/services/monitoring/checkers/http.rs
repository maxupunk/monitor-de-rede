//! Checker HTTP com timeout e política explícita de códigos aceitos.

use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

use chrono::Utc;
use reqwest::{Client, Method};
use serde::Deserialize;

use crate::services::monitoring::contracts::{CheckMetric, CheckResult, Checker, MonitorStatus};

/// Configuração serializável de uma verificação HTTP/HTTPS.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpConfig {
    pub url: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_codes")]
    pub accepted_status_codes: Vec<u16>,
    #[serde(default = "default_validate_certificate")]
    pub validate_certificate: bool,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

fn default_method() -> String {
    "GET".into()
}
fn default_codes() -> Vec<u16> {
    vec![200, 201, 202, 204, 301, 302]
}
const fn default_validate_certificate() -> bool {
    true
}
const fn default_timeout_ms() -> u64 {
    10_000
}

static DEFAULT_CLIENT: OnceLock<Result<Client, reqwest::Error>> = OnceLock::new();
static DANGEROUS_CLIENT: OnceLock<Result<Client, reqwest::Error>> = OnceLock::new();

fn default_client() -> &'static Result<Client, reqwest::Error> {
    DEFAULT_CLIENT.get_or_init(|| Client::builder().build())
}

fn dangerous_client() -> &'static Result<Client, reqwest::Error> {
    DANGEROUS_CLIENT.get_or_init(|| Client::builder().danger_accept_invalid_certs(true).build())
}

/// Implementação HTTP baseada em `reqwest` com TLS Rustls.
pub struct HttpChecker;

#[async_trait::async_trait]
impl Checker for HttpChecker {
    type Config = HttpConfig;

    async fn execute(&self, config: Self::Config) -> CheckResult {
        let started_at = Utc::now();
        let method = Method::from_bytes(config.method.as_bytes()).unwrap_or(Method::GET);
        let client_source = if config.validate_certificate {
            default_client()
        } else {
            dangerous_client()
        };
        let client = match client_source {
            Ok(client) => client.clone(),
            Err(error) => {
                return failed_result(started_at, &config, method.as_str(), error.to_string())
            }
        };
        let response = client
            .request(method.clone(), &config.url)
            .timeout(Duration::from_millis(config.timeout_ms.max(1)))
            .headers(
                config
                    .headers
                    .iter()
                    .filter_map(|(key, value)| {
                        let name = reqwest::header::HeaderName::try_from(key.as_str()).ok()?;
                        let value = reqwest::header::HeaderValue::try_from(value).ok()?;
                        Some((name, value))
                    })
                    .collect(),
            )
            .send()
            .await;
        let finished_at = Utc::now();
        let duration_ms = (finished_at - started_at).num_milliseconds().max(0);
        match response {
            Ok(response) => {
                let status_code = response.status().as_u16();
                let status_text = response
                    .status()
                    .canonical_reason()
                    .unwrap_or("")
                    .to_string();
                let accepted = config.accepted_status_codes.contains(&status_code);
                CheckResult {
                    success: accepted,
                    status: if accepted {
                        MonitorStatus::Up
                    } else {
                        MonitorStatus::Warning
                    },
                    started_at,
                    finished_at,
                    duration_ms,
                    message: Some(format!(
                        "HTTP {} {} respondeu com código {} em {}ms",
                        method, config.url, status_code, duration_ms
                    )),
                    metrics: vec![
                        CheckMetric {
                            name: "response_time".into(),
                            value: duration_ms as f64,
                            unit: "ms".into(),
                        },
                        CheckMetric {
                            name: "status_code".into(),
                            value: f64::from(status_code),
                            unit: "code".into(),
                        },
                    ],
                    data: serde_json::json!({ "statusCode": status_code, "statusText": status_text }),
                }
            }
            Err(error) => failed_result(started_at, &config, method.as_str(), error.to_string()),
        }
    }
}

fn failed_result(
    started_at: chrono::DateTime<Utc>,
    config: &HttpConfig,
    method: &str,
    error: String,
) -> CheckResult {
    let finished_at = Utc::now();
    let duration_ms = (finished_at - started_at).num_milliseconds().max(0);
    CheckResult {
        success: false,
        status: MonitorStatus::Down,
        started_at,
        finished_at,
        duration_ms,
        message: Some(format!(
            "Falha na requisição HTTP {method} para {}: {error}",
            config.url
        )),
        metrics: vec![CheckMetric {
            name: "response_time".into(),
            value: duration_ms as f64,
            unit: "ms".into(),
        }],
        data: serde_json::json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    #[serial]
    async fn clientes_http_sao_cacheados() {
        let default_a = default_client();
        let default_b = default_client();
        assert!(std::ptr::eq(default_a, default_b));

        let dangerous_a = dangerous_client();
        let dangerous_b = dangerous_client();
        assert!(std::ptr::eq(dangerous_a, dangerous_b));
    }

    #[tokio::test]
    #[serial]
    async fn http_200_eh_marcado_como_up() {
        let url = spawn_local_server("HTTP/1.1 200 OK", "OK").await;
        let config = HttpConfig {
            url,
            method: "GET".into(),
            accepted_status_codes: vec![200],
            validate_certificate: true,
            timeout_ms: 1_000,
            headers: BTreeMap::new(),
        };
        let result = tokio::time::timeout(Duration::from_secs(5), HttpChecker.execute(config))
            .await
            .expect("timeout no teste");
        assert!(result.success);
        assert_eq!(result.status, MonitorStatus::Up);
        let status_metric = result
            .metrics
            .iter()
            .find(|metric| metric.name == "status_code");
        assert_eq!(status_metric.map(|metric| metric.value), Some(200.0));
    }

    #[tokio::test]
    #[serial]
    async fn http_404_eh_marcado_como_warning() {
        let url = spawn_local_server("HTTP/1.1 404 Not Found", "Not Found").await;
        let config = HttpConfig {
            url,
            method: "GET".into(),
            accepted_status_codes: vec![200],
            validate_certificate: true,
            timeout_ms: 1_000,
            headers: BTreeMap::new(),
        };
        let result = tokio::time::timeout(Duration::from_secs(5), HttpChecker.execute(config))
            .await
            .expect("timeout no teste");
        assert!(!result.success);
        assert_eq!(result.status, MonitorStatus::Warning);
        let status_metric = result
            .metrics
            .iter()
            .find(|metric| metric.name == "status_code");
        assert_eq!(status_metric.map(|metric| metric.value), Some(404.0));
    }

    async fn spawn_local_server(status_line: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let status = status_line.to_string();
        let body = body.to_string();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await.unwrap();
            let response = format!(
                "{status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{}:{}", addr.ip(), addr.port())
    }
}
