//! Suíte de paridade da Fase 9: bate os endpoints nos dois backends e compara
//! os JSONs normalizados.
//!
//! Roda contra os **dois processos vivos** — não há como comparar contratos sem
//! exercitá-los. É `example` e não teste por isso: um `cargo test` não pode
//! depender de dois servidores no ar.
//!
//! ```sh
//! # Sobe os dois (portas diferentes) e depois:
//! ADONIS_URL=http://localhost:3333 \
//! RUST_URL=http://localhost:3334 \
//! PARITY_EMAIL=admin@monitor.local PARITY_PASSWORD=admin123 \
//!   cargo run --example parity_check
//! ```
//!
//! **O que é normalizado antes de comparar** (e por quê):
//!
//! - `id`, `createdAt`, `updatedAt` e afins mudam entre duas instalações
//!   distintas e não descrevem contrato;
//! - a **ordem das chaves** de objeto é irrelevante em JSON (o `serde_json`
//!   com `preserve_order` desligado já ordena, mas a normalização deixa isso
//!   explícito);
//! - números inteiros vindos como `1` e `1.0` são o mesmo valor para o
//!   frontend, que lê tudo com `Number()`.
//!
//! O que **não** é normalizado: nome de campo, tipo (string vs número),
//! presença/ausência de chave e formato de data. É exatamente aí que uma
//! migração quebra tela, e é isso que a suíte existe para pegar.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

/// Endpoints comparados. Só `GET`: a suíte não escreve em banco de produção.
///
/// A lista cobre o que o frontend carrega ao abrir cada tela — é o conjunto
/// cuja divergência o usuário enxerga primeiro.
const ENDPOINTS: &[&str] = &[
    "/",
    "/api/sites",
    "/api/networks",
    "/api/devices",
    "/api/monitors",
    "/api/probes",
    "/api/alert-rules",
    "/api/alert-rules/catalog",
    "/api/alerts",
    "/api/alerts?page=1&limit=20",
    "/api/dns/servers",
    "/api/dns/performance?hours=24",
    "/api/discovery/runs",
    "/api/discovery/scan-state",
    "/api/topology",
    "/api/zabbix-templates",
    "/api/dashboard/layout",
    "/api/vpn/server",
    "/api/vpn/peers",
    "/api/events?page=1&limit=20",
];

/// Chaves cujo valor é instância-específico e não faz parte do contrato.
const VOLATILE_KEYS: &[&str] = &[
    "id",
    "createdAt",
    "updatedAt",
    "startedAt",
    "finishedAt",
    "resolvedAt",
    "lastRunAt",
    "nextRunAt",
    "lastSeenAt",
    "lastScanAt",
    "nextScanAt",
    "lastSyncedAt",
    "lastHandshakeAt",
    "recordedAt",
    "measuredAt",
    "importedAt",
    "registeredAt",
    "revokedAt",
    "silencedUntil",
    "publicKey",
    "tokenHash",
    "version",
    "content",
    "qrSvg",
    "timestamp",
    "occurredAt",
];

/// Substitui valores voláteis por um marcador de **tipo**.
///
/// Trocar por `null` esconderia uma divergência real: um `id` que virou string
/// continuaria "igual". Guardar o tipo mantém a comparação útil.
fn normalize(value: &Value, key: Option<&str>) -> Value {
    if key.is_some_and(|key| VOLATILE_KEYS.contains(&key)) {
        return Value::String(format!("<{}>", type_name(value)));
    }
    match value {
        Value::Object(map) => {
            // `BTreeMap` ordena as chaves: a ordem em JSON não é contrato.
            let ordered: BTreeMap<&String, &Value> = map.iter().collect();
            Value::Object(
                ordered
                    .into_iter()
                    .map(|(key, value)| (key.clone(), normalize(value, Some(key))))
                    .collect::<Map<String, Value>>(),
            )
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| normalize(item, None)).collect())
        }
        // `1` e `1.0` são o mesmo número para o cliente.
        Value::Number(number) => number
            .as_f64()
            .map_or_else(|| value.clone(), |value| Value::String(format!("{value}"))),
        other => other.clone(),
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Compara duas árvores e devolve os caminhos divergentes.
fn diff(path: &str, left: &Value, right: &Value, findings: &mut Vec<String>) {
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort_unstable();
            keys.dedup();
            for key in keys {
                let child = format!("{path}.{key}");
                match (a.get(key), b.get(key)) {
                    (Some(left), Some(right)) => diff(&child, left, right, findings),
                    (Some(_), None) => findings.push(format!("{child}: só no AdonisJS")),
                    (None, Some(_)) => findings.push(format!("{child}: só no Rust")),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                findings.push(format!(
                    "{path}: tamanho {} (AdonisJS) vs {} (Rust)",
                    a.len(),
                    b.len()
                ));
            }
            for (index, (left, right)) in a.iter().zip(b.iter()).enumerate() {
                diff(&format!("{path}[{index}]"), left, right, findings);
            }
        }
        (left, right) if left != right => {
            findings.push(format!("{path}: {left} (AdonisJS) vs {right} (Rust)"));
        }
        _ => {}
    }
}

struct Backend {
    label: &'static str,
    base_url: String,
    token: Option<String>,
}

impl Backend {
    async fn login(&mut self, client: &reqwest::Client, email: &str, password: &str) {
        let body = match client
            .post(format!("{}/api/auth/login", self.base_url))
            .json(&serde_json::json!({ "email": email, "password": password }))
            .send()
            .await
        {
            Ok(response) => response.json::<Value>().await.unwrap_or(Value::Null),
            Err(_) => Value::Null,
        };
        self.token = body
            .get("token")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        if self.token.is_none() {
            eprintln!(
                "aviso: login falhou em {} — endpoints protegidos vão responder 401",
                self.label
            );
        }
    }

    async fn get(&self, client: &reqwest::Client, path: &str) -> (u16, Value) {
        let mut request = client.get(format!("{}{path}", self.base_url));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response.json::<Value>().await.unwrap_or(Value::Null);
                (status, body)
            }
            Err(error) => {
                eprintln!("{}: {path} falhou — {error}", self.label);
                (0, Value::Null)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("cliente HTTP");

    let mut adonis = Backend {
        label: "AdonisJS",
        base_url: std::env::var("ADONIS_URL")
            .unwrap_or_else(|_| "http://localhost:3333".to_string()),
        token: None,
    };
    let mut rust = Backend {
        label: "Rust",
        base_url: std::env::var("RUST_URL").unwrap_or_else(|_| "http://localhost:3334".to_string()),
        token: None,
    };

    let email = std::env::var("PARITY_EMAIL").unwrap_or_else(|_| "admin@monitor.local".into());
    let password = std::env::var("PARITY_PASSWORD").unwrap_or_else(|_| "admin123".into());
    adonis.login(&client, &email, &password).await;
    rust.login(&client, &email, &password).await;

    println!("Paridade: {} × {}\n", adonis.base_url, rust.base_url);
    let mut total_findings = 0;
    let mut compared = 0;

    for path in ENDPOINTS {
        let (adonis_status, adonis_body) = adonis.get(&client, path).await;
        let (rust_status, rust_body) = rust.get(&client, path).await;

        let mut findings = Vec::new();
        if adonis_status != rust_status {
            findings.push(format!(
                "status: {adonis_status} (AdonisJS) vs {rust_status} (Rust)"
            ));
        }
        diff(
            "$",
            &normalize(&adonis_body, None),
            &normalize(&rust_body, None),
            &mut findings,
        );

        compared += 1;
        if findings.is_empty() {
            println!("  ok        {path}");
        } else {
            total_findings += findings.len();
            println!("  DIVERGE   {path}");
            for finding in findings.iter().take(20) {
                println!("              {finding}");
            }
            if findings.len() > 20 {
                println!(
                    "              … e mais {} divergências",
                    findings.len() - 20
                );
            }
        }
    }

    println!("\n{compared} endpoints comparados, {total_findings} divergências.");
    if total_findings > 0 {
        // Código de saída != 0 para o comando servir de portão em CI.
        std::process::exit(1);
    }
}
