//! Checker HTTP com timeout e política explícita de códigos aceitos.

use std::{collections::BTreeMap, time::Duration};

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

/// Implementação HTTP baseada em `reqwest` com TLS Rustls.
pub struct HttpChecker;

#[async_trait::async_trait]
impl Checker for HttpChecker {
    type Config = HttpConfig;

    async fn execute(&self, config: Self::Config) -> CheckResult {
        let started_at = Utc::now();
        let method = Method::from_bytes(config.method.as_bytes()).unwrap_or(Method::GET);
        let client = match Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms.max(1)))
            .danger_accept_invalid_certs(!config.validate_certificate)
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                return failed_result(started_at, &config, method.as_str(), error.to_string())
            }
        };
        let response = client
            .request(method.clone(), &config.url)
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
