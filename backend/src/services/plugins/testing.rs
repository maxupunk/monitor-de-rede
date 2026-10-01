//! Os testes do plugin: unitários (sem equipamento) e a checagem dos
//! funcionais (no equipamento, executados por `runs`).
//!
//! O unitário roda o script de verdade, no mesmo sandbox da produção, contra
//! o [`FakeTransport`]: o que muda é só quem responde. Por isso um teste verde
//! prova que o script faz o que diz com aquelas respostas — e um comando novo
//! sem fixture quebra o teste em vez de passar despercebido.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::{
    gate::AllowAll,
    manifest::TransportKind,
    package::{self, FunctionalTest, PluginPackage, UnitTest},
    params,
    runtime::{
        self, AccessProfile, Credentials, DeviceInfo, ExecutionContext, Limits, TranscriptEntry,
    },
    transport::{fake::FakeTransport, Login},
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TestCaseResult {
    pub name: String,
    pub action: String,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub output: Option<Value>,
    pub transcript: Vec<TranscriptEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TestReport {
    pub passed: bool,
    pub total: u32,
    pub failed: u32,
    /// Problemas de validação do pacote. Qualquer um reprova.
    pub problems: Vec<String>,
    pub cases: Vec<TestCaseResult>,
}

fn test_credentials() -> Credentials {
    let profile = |port| {
        Some(AccessProfile {
            login: Login {
                username: "usuario-de-teste".into(),
                password: None,
            },
            port,
            https: false,
        })
    };
    Credentials {
        ssh: profile(22),
        http: profile(80),
        telnet: profile(23),
    }
}

/// Roda todos os testes unitários do pacote.
pub async fn run_unit_tests(package: &PluginPackage) -> TestReport {
    let problems = package::validate(package);
    let mut cases = Vec::with_capacity(package.tests.unit.len());
    for (index, test) in package.tests.unit.iter().enumerate() {
        cases.push(run_unit_test(package, test, index).await);
    }
    let failed =
        u32::try_from(cases.iter().filter(|case| !case.passed).count()).unwrap_or(u32::MAX);
    TestReport {
        passed: problems.is_empty() && failed == 0 && !cases.is_empty(),
        total: u32::try_from(cases.len()).unwrap_or(u32::MAX),
        failed,
        problems,
        cases,
    }
}

async fn run_unit_test(package: &PluginPackage, test: &UnitTest, index: usize) -> TestCaseResult {
    let name = test
        .name
        .clone()
        .unwrap_or_else(|| format!("#{} {}", index + 1, test.action));
    let failed =
        |message: String, output: Option<Value>, transcript: Vec<TranscriptEntry>| TestCaseResult {
            name: name.clone(),
            action: test.action.clone(),
            passed: false,
            message: Some(message),
            output,
            transcript,
        };
    let Some(action) = package.manifest.action(&test.action) else {
        return failed(format!("ação inexistente: {}", test.action), None, vec![]);
    };
    let params = match params::validate(action.params.as_ref(), &test.params) {
        Ok(params) => params,
        // Parâmetro recusado antes de chegar ao equipamento é um desfecho
        // legítimo de teste: é assim que se prova a defesa contra injeção.
        Err(errors) => {
            let message = errors.join("; ");
            return match &test.expect_error {
                Some(expected) if message.contains(expected.as_str()) => TestCaseResult {
                    name,
                    action: test.action.clone(),
                    passed: true,
                    message: None,
                    output: None,
                    transcript: vec![],
                },
                _ => failed(message, None, vec![]),
            };
        }
    };
    let context = ExecutionContext {
        device: DeviceInfo {
            id: 0,
            name: "equipamento de teste".into(),
            ip: "192.0.2.1".into(),
            platform: package
                .manifest
                .matcher
                .platforms
                .first()
                .cloned()
                .unwrap_or_else(|| "other".into()),
            firmware: test.firmware.clone(),
            ..DeviceInfo::default()
        },
        transports: package.manifest.transports.clone(),
        action_effect: action.effect,
        reason: None,
        credentials: test_credentials(),
        transport: Arc::new(FakeTransport::new(test.fixtures.clone())),
        gate: Arc::new(AllowAll),
        cancel: CancellationToken::new(),
        observer: None,
        limits: Limits {
            deadline: Duration::from_secs(20),
            ..Limits::default()
        },
    };
    let outcome = runtime::execute(&package.script, &action.id, params, context).await;
    let transcript = outcome.transcript;
    match (&test.expect_error, outcome.output) {
        (Some(expected), Err(error)) if error.contains(expected.as_str()) => TestCaseResult {
            name,
            action: test.action.clone(),
            passed: true,
            message: None,
            output: None,
            transcript,
        },
        (Some(expected), Err(error)) => failed(
            format!("falhou com \"{error}\", e o esperado era um erro contendo \"{expected}\""),
            None,
            transcript,
        ),
        (Some(expected), Ok(output)) => failed(
            format!("era esperado um erro contendo \"{expected}\", e a ação terminou bem"),
            Some(output),
            transcript,
        ),
        (None, Err(error)) => failed(error, None, transcript),
        (None, Ok(output)) => {
            if let Some(expected) = &test.expect {
                if !contains(expected, &output) {
                    return failed(
                        format!("resultado diferente do esperado: esperado {expected}"),
                        Some(output),
                        transcript,
                    );
                }
            }
            if let Some(text) = &test.expect_contains {
                if !render(&output).contains(text.as_str()) {
                    return failed(
                        format!("o resultado não contém \"{text}\""),
                        Some(output),
                        transcript,
                    );
                }
            }
            TestCaseResult {
                name,
                action: test.action.clone(),
                passed: true,
                message: None,
                output: Some(output),
                transcript,
            }
        }
    }
}

fn render(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// `expected` está contido em `actual`: objetos por chave (recursivo), listas
/// elemento a elemento e o resto por igualdade (números como `f64`).
#[must_use]
pub fn contains(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => expected
            .iter()
            .all(|(key, value)| actual.get(key).is_some_and(|found| contains(value, found))),
        (Value::Array(expected), Value::Array(actual)) => {
            expected.len() == actual.len()
                && expected.iter().zip(actual).all(|(e, a)| contains(e, a))
        }
        (Value::Number(expected), Value::Number(actual)) => expected.as_f64() == actual.as_f64(),
        _ => expected == actual,
    }
}

/// Confere o resultado de um teste funcional.
///
/// # Errors
///
/// A expectativa que não se cumpriu, em português.
pub fn check_functional(test: &FunctionalTest, output: &Value) -> Result<(), String> {
    for key in &test.expect_keys {
        let present = output
            .get(key)
            .is_some_and(|value| !value.is_null() && value != &Value::String(String::new()));
        if !present {
            return Err(format!("o resultado não trouxe `{key}`"));
        }
    }
    if let Some(minimum) = test.expect_min_rows {
        let rows = output.as_array().map_or(0, Vec::len);
        if rows < minimum as usize {
            return Err(format!(
                "eram esperadas ao menos {minimum} linhas, vieram {rows}"
            ));
        }
    }
    if let Some(text) = &test.expect_contains {
        if !render(output).contains(text.as_str()) {
            return Err(format!("o resultado não contém \"{text}\""));
        }
    }
    Ok(())
}

/// Os transportes que a suíte funcional vai usar — os do manifesto.
#[must_use]
pub fn functional_transports(package: &PluginPackage) -> Vec<TransportKind> {
    package.manifest.transports.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn package(expect: Value) -> PluginPackage {
        serde_json::from_value(json!({
            "manifest": {
                "slug": "exemplo", "name": "Exemplo", "version": "1.0.0",
                "transports": ["ssh"],
                "actions": [ { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" } ]
            },
            "script": "fn detect(device, params) { let v = device.run(\"cat /etc/version\"); #{ firmware: trimmed(v) } }",
            "usage": "## Detectar",
            "tests": {
                "unit": [ {
                    "action": "detect",
                    "fixtures": [ { "ssh": "cat /etc/version", "stdout": "1.2.3\n" } ],
                    "expect": expect
                } ],
                "functional": [ { "action": "detect", "expectKeys": ["firmware"] } ]
            }
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn teste_que_bate_passa() {
        let report = run_unit_tests(&package(json!({ "firmware": "1.2.3" }))).await;
        assert!(report.passed, "{report:?}");
        assert_eq!(report.total, 1);
    }

    #[tokio::test]
    async fn teste_que_nao_bate_reprova() {
        let report = run_unit_tests(&package(json!({ "firmware": "9.9.9" }))).await;
        assert!(!report.passed);
        assert_eq!(report.failed, 1);
        assert!(report.cases[0]
            .message
            .as_deref()
            .unwrap()
            .contains("diferente"));
    }

    #[test]
    fn contido_compara_por_subconjunto() {
        assert!(contains(&json!({"a": 1}), &json!({"a": 1.0, "b": 2})));
        assert!(!contains(&json!({"a": [1, 2]}), &json!({"a": [1]})));
        assert!(contains(&json!("x"), &json!("x")));
    }

    #[test]
    fn funcional_confere_chaves_linhas_e_texto() {
        let test = FunctionalTest {
            action: "detect".into(),
            params: Value::Null,
            expect_keys: vec!["firmware".into()],
            expect_min_rows: None,
            expect_contains: None,
        };
        assert!(check_functional(&test, &json!({ "firmware": "1.0" })).is_ok());
        assert!(check_functional(&test, &json!({ "firmware": "" })).is_err());
        let rows = FunctionalTest {
            expect_keys: vec![],
            expect_min_rows: Some(2),
            ..test
        };
        assert!(check_functional(&rows, &json!([1])).is_err());
        assert!(check_functional(&rows, &json!([1, 2])).is_ok());
    }
}
