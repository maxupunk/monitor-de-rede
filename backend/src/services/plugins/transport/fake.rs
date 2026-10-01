//! Transporte de teste: responde com as fixtures gravadas.
//!
//! Chamada sem fixture correspondente é **erro**, não resposta vazia. É isso
//! que faz o teste unitário provar alguma coisa: se o script passar a rodar um
//! comando que o teste não previu, o teste quebra em vez de seguir com uma
//! saída inventada.
//!
//! Fixtures repetidas para a mesma chamada respondem **em sequência** (e a
//! última se repete): é assim que um teste mostra o "antes" e o "depois" de
//! uma alteração — a lista de pacotes sem e com o pacote instalado.

use std::{collections::HashMap, sync::Mutex};

use async_trait::async_trait;
use regex::Regex;

use super::{DeviceIoCall, DeviceIoReply, DeviceTransport};
use crate::{
    services::plugins::package::Fixture,
    services::shared::errors::{AppError, AppResult},
};

pub struct FakeTransport {
    fixtures: Vec<Fixture>,
    /// Quantas vezes cada chamada já foi respondida.
    calls: Mutex<HashMap<String, usize>>,
}

impl FakeTransport {
    #[must_use]
    pub fn new(fixtures: Vec<Fixture>) -> Self {
        Self {
            fixtures,
            calls: Mutex::default(),
        }
    }

    /// Quantas vezes `key` já foi chamada antes desta.
    fn next_call(&self, key: &str) -> usize {
        let mut calls = self
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = calls.entry(key.to_owned()).or_insert(0);
        let previous = *count;
        *count += 1;
        previous
    }
}

fn key_matches(pattern: &str, regex: bool, actual: &str) -> bool {
    if regex {
        Regex::new(&format!("^(?:{pattern})$")).is_ok_and(|re| re.is_match(actual))
    } else {
        pattern.trim() == actual.trim()
    }
}

/// A chave com que uma chamada é procurada nas fixtures.
#[must_use]
pub fn call_key(call: &DeviceIoCall) -> String {
    match call {
        DeviceIoCall::SshExec { command, .. } | DeviceIoCall::TelnetExec { command, .. } => {
            command.clone()
        }
        DeviceIoCall::HttpRequest { request, .. } => {
            format!("{} {}", request.method.to_ascii_uppercase(), request.path)
        }
    }
}

#[async_trait]
impl DeviceTransport for FakeTransport {
    async fn execute(&self, call: &DeviceIoCall) -> AppResult<DeviceIoReply> {
        let actual = call_key(call);
        let candidates: Vec<&Fixture> = self
            .fixtures
            .iter()
            .filter(|fixture| match call {
                DeviceIoCall::SshExec { .. } => fixture
                    .ssh
                    .as_deref()
                    .is_some_and(|key| key_matches(key, fixture.regex, &actual)),
                DeviceIoCall::TelnetExec { .. } => fixture
                    .telnet
                    .as_deref()
                    .is_some_and(|key| key_matches(key, fixture.regex, &actual)),
                DeviceIoCall::HttpRequest { .. } => fixture
                    .http
                    .as_deref()
                    .is_some_and(|key| key_matches(key, fixture.regex, &actual)),
            })
            .collect();
        let found = candidates
            .get(
                self.next_call(&actual)
                    .min(candidates.len().saturating_sub(1)),
            )
            .copied();
        let Some(fixture) = found else {
            return Err(AppError::validation(format!(
                "chamada sem fixture no teste: {} `{actual}`",
                call.transport().as_str()
            )));
        };
        Ok(match call {
            DeviceIoCall::HttpRequest { .. } => DeviceIoReply::Http {
                status: fixture.status,
                headers: fixture
                    .headers
                    .as_ref()
                    .and_then(serde_json::Value::as_object)
                    .map(|headers| {
                        headers
                            .iter()
                            .map(|(name, value)| {
                                (
                                    name.to_ascii_lowercase(),
                                    value.as_str().unwrap_or_default().to_owned(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                body: fixture.body.clone(),
            },
            _ => DeviceIoReply::Exec {
                stdout: fixture.stdout.clone(),
                stderr: fixture.stderr.clone(),
                exit_code: Some(fixture.exit),
            },
        })
    }

    fn origin(&self) -> String {
        "teste".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::plugins::transport::{Endpoint, Login};

    fn ssh(command: &str) -> DeviceIoCall {
        DeviceIoCall::SshExec {
            endpoint: Endpoint {
                host: "192.0.2.1".into(),
                port: 22,
            },
            login: Login {
                username: "root".into(),
                password: None,
            },
            command: command.into(),
            timeout_ms: 1_000,
        }
    }

    #[tokio::test]
    async fn fixture_exata_e_por_regex() {
        let fake = FakeTransport::new(vec![
            Fixture {
                ssh: Some("uptime".into()),
                stdout: "up 3 days".into(),
                ..Fixture::default()
            },
            Fixture {
                ssh: Some(r"opkg info \S+".into()),
                regex: true,
                stdout: "Package: x".into(),
                ..Fixture::default()
            },
        ]);
        let DeviceIoReply::Exec { stdout, .. } = fake.execute(&ssh("uptime")).await.unwrap() else {
            panic!()
        };
        assert_eq!(stdout, "up 3 days");
        assert!(fake.execute(&ssh("opkg info luci")).await.is_ok());
    }

    #[tokio::test]
    async fn fixtures_repetidas_respondem_em_sequencia() {
        let fake = FakeTransport::new(vec![
            Fixture {
                ssh: Some("opkg list-installed".into()),
                stdout: "antes".into(),
                ..Fixture::default()
            },
            Fixture {
                ssh: Some("opkg list-installed".into()),
                stdout: "depois".into(),
                ..Fixture::default()
            },
        ]);
        let mut saidas = Vec::new();
        for _ in 0..3 {
            let DeviceIoReply::Exec { stdout, .. } =
                fake.execute(&ssh("opkg list-installed")).await.unwrap()
            else {
                panic!()
            };
            saidas.push(stdout);
        }
        assert_eq!(saidas, ["antes", "depois", "depois"]);
    }

    #[tokio::test]
    async fn chamada_imprevista_quebra_o_teste() {
        let fake = FakeTransport::new(vec![]);
        let error = fake.execute(&ssh("reboot")).await.unwrap_err();
        assert!(error.to_string().contains("sem fixture"));
    }
}
