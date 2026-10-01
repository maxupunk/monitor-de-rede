//! O pacote `.nmplugin`: o plugin inteiro num JSON só.
//!
//! É o formato de importação, de exportação e de gravação. Leva junto o que
//! permite a outro operador confiar no plugin sem ler o script:
//!
//! * `usage` — como usar cada ação, em markdown. É a documentação **de uso**,
//!   que viaja com o plugin; a de **autoria** (como escrever um) é global e
//!   fica com a IA.
//! * `compatibility` — os equipamentos em que o plugin já passou no teste
//!   funcional, com modelo, firmware e data.
//! * `tests` — unitários (com as respostas do equipamento gravadas em
//!   fixtures) e funcionais (executados no equipamento real).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{
    effect::Effect,
    manifest::{self, PluginManifest},
    runtime,
};
use crate::services::shared::crypto::sha256_hex;

/// Versão do formato do arquivo.
pub const FORMAT_VERSION: u32 = 1;

/// Teto do script. Plugin que precisa de mais que isto está fazendo coisa
/// demais num lugar só.
pub const MAX_SCRIPT_BYTES: usize = 256 * 1024;

/// Uma resposta gravada do equipamento, para o teste unitário.
///
/// Exatamente um de `ssh`, `telnet` ou `http` identifica a chamada; o resto é
/// a resposta. `http` é `"MÉTODO /caminho"`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct Fixture {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ssh: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub telnet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub http: Option<String>,
    /// Trata a chave (`ssh`/`telnet`/`http`) como regex em vez de texto exato.
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    #[serde(default)]
    pub exit: i32,
    #[serde(default = "default_status")]
    pub status: u16,
    #[serde(default)]
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, string>")]
    pub headers: Option<Value>,
}

const fn default_status() -> u16 {
    200
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct UnitTest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    pub action: String,
    #[serde(default)]
    #[ts(type = "Record<string, unknown>")]
    pub params: Value,
    #[serde(default)]
    pub fixtures: Vec<Fixture>,
    /// Subconjunto esperado do resultado (objetos comparados por chave).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub expect: Option<Value>,
    /// Texto que precisa aparecer no resultado serializado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expect_contains: Option<String>,
    /// A ação precisa falhar com uma mensagem que contenha este texto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expect_error: Option<String>,
    /// Firmware do equipamento simulado (`device.info.firmware`). Permite
    /// testar o caminho que depende da versão — ex.: opkg × apk no OpenWrt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub firmware: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct FunctionalTest {
    pub action: String,
    #[serde(default)]
    #[ts(type = "Record<string, unknown>")]
    pub params: Value,
    /// Chaves que o resultado (objeto) precisa ter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expect_keys: Vec<String>,
    /// Mínimo de linhas quando o resultado é uma lista.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expect_min_rows: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expect_contains: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginTests {
    #[serde(default)]
    pub unit: Vec<UnitTest>,
    #[serde(default)]
    pub functional: Vec<FunctionalTest>,
}

/// Um equipamento em que o plugin já foi validado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct CompatEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub firmware: Option<String>,
    /// `passed` ou `failed`.
    pub status: String,
    pub validated_at: String,
    /// Versão do plugin validada.
    pub plugin_version: String,
    /// Execução que comprova (só faz sentido na instalação de origem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub run_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginPackage {
    #[serde(default = "default_format")]
    pub format: u32,
    pub manifest: PluginManifest,
    pub script: String,
    pub usage: String,
    #[serde(default)]
    pub compatibility: Vec<CompatEntry>,
    #[serde(default)]
    pub tests: PluginTests,
}

const fn default_format() -> u32 {
    FORMAT_VERSION
}

impl PluginPackage {
    /// Identidade do **código**: manifesto, script, uso e testes. A lista de
    /// compatibilidade fica de fora — validar em mais um equipamento não muda
    /// o que o plugin faz, e não pode invalidar os testes nem a revisão.
    #[must_use]
    pub fn checksum(&self) -> String {
        let canonical = serde_json::json!({
            "manifest": self.manifest,
            "script": self.script,
            "usage": self.usage,
            "tests": self.tests,
        });
        sha256_hex(&canonical.to_string())
    }
}

/// Todos os problemas do pacote — manifesto, script, uso e testes.
#[must_use]
pub fn validate(package: &PluginPackage) -> Vec<String> {
    let mut problems = manifest::validate(&package.manifest);
    if package.format != FORMAT_VERSION {
        problems.push(format!(
            "formato {} desconhecido; esta versão lê o formato {FORMAT_VERSION}",
            package.format
        ));
    }
    if package.script.len() > MAX_SCRIPT_BYTES {
        problems.push(format!("o script passa de {} KB", MAX_SCRIPT_BYTES / 1024));
    }
    match runtime::entry_points(&package.script) {
        Err(error) => problems.push(format!("o script não compila: {error}")),
        Ok(functions) => {
            for action in &package.manifest.actions {
                if !functions
                    .iter()
                    .any(|(name, arity)| name == &action.id && *arity == 2)
                {
                    problems.push(format!(
                        "o script precisa definir `fn {}(device, params)`",
                        action.id
                    ));
                }
            }
        }
    }
    validate_usage(package, &mut problems);
    validate_tests(package, &mut problems);
    problems
}

fn validate_usage(package: &PluginPackage, problems: &mut Vec<String>) {
    let usage = package.usage.to_lowercase();
    if usage.trim().is_empty() {
        problems.push("o `usage` (como usar, em markdown) é obrigatório".into());
        return;
    }
    for action in &package.manifest.actions {
        if !usage.contains(&action.title.to_lowercase()) && !usage.contains(&action.id) {
            problems.push(format!(
                "o `usage` não explica a ação `{}` ({})",
                action.id, action.title
            ));
        }
    }
}

fn validate_tests(package: &PluginPackage, problems: &mut Vec<String>) {
    let manifest = &package.manifest;
    for test in &package.tests.unit {
        if manifest.action(&test.action).is_none() {
            problems.push(format!(
                "teste unitário para ação inexistente: `{}`",
                test.action
            ));
        }
        for fixture in &test.fixtures {
            let keys = [&fixture.ssh, &fixture.telnet, &fixture.http]
                .iter()
                .filter(|key| key.is_some())
                .count();
            if keys != 1 {
                problems.push(format!(
                    "teste de `{}`: cada fixture precisa de exatamente um de ssh, telnet ou http",
                    test.action
                ));
            }
        }
    }
    for test in &package.tests.functional {
        match manifest.action(&test.action) {
            None => problems.push(format!(
                "teste funcional para ação inexistente: `{}`",
                test.action
            )),
            Some(action) if action.effect == Effect::Write && !action.safe_to_retest => {
                problems.push(format!(
                    "o teste funcional não pode rodar `{}`: é de escrita e não está marcada como `safeToRetest`",
                    action.id
                ));
            }
            Some(_) => {}
        }
    }
    for action in &manifest.actions {
        if !package
            .tests
            .unit
            .iter()
            .any(|test| test.action == action.id)
        {
            problems.push(format!("a ação `{}` não tem teste unitário", action.id));
        }
        if action.effect == Effect::Read
            && !package
                .tests
                .functional
                .iter()
                .any(|test| test.action == action.id)
        {
            problems.push(format!(
                "a ação de leitura `{}` não tem teste funcional",
                action.id
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn package() -> PluginPackage {
        serde_json::from_value(json!({
            "format": 1,
            "manifest": {
                "slug": "exemplo",
                "name": "Exemplo",
                "version": "1.0.0",
                "transports": ["ssh"],
                "actions": [
                    { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" }
                ]
            },
            "script": "fn detect(device, params) { #{ firmware: \"1.0\" } }",
            "usage": "## Detectar\nLê a versão.",
            "tests": {
                "unit": [ { "action": "detect", "expect": { "firmware": "1.0" } } ],
                "functional": [ { "action": "detect", "expectKeys": ["firmware"] } ]
            }
        }))
        .unwrap()
    }

    #[test]
    fn pacote_minimo_e_valido() {
        assert!(
            validate(&package()).is_empty(),
            "{:?}",
            validate(&package())
        );
    }

    #[test]
    fn funcao_faltando_no_script_e_apontada() {
        let mut pacote = package();
        pacote.script = "fn outra(device, params) { 1 }".into();
        assert!(validate(&pacote)
            .iter()
            .any(|p| p.contains("fn detect(device, params)")));
    }

    #[test]
    fn script_que_nao_compila_e_apontado() {
        let mut pacote = package();
        pacote.script = "fn detect(device, params) { ".into();
        assert!(validate(&pacote).iter().any(|p| p.contains("não compila")));
    }

    #[test]
    fn acao_sem_teste_e_uso_sem_secao_sao_apontados() {
        let mut pacote = package();
        pacote.tests = PluginTests::default();
        pacote.usage = "Nada a ver.".into();
        let problems = validate(&pacote);
        assert!(problems.iter().any(|p| p.contains("teste unitário")));
        assert!(problems.iter().any(|p| p.contains("teste funcional")));
        assert!(problems.iter().any(|p| p.contains("não explica")));
    }

    #[test]
    fn checksum_ignora_a_compatibilidade() {
        let mut pacote = package();
        let antes = pacote.checksum();
        pacote.compatibility.push(CompatEntry {
            platform: Some("openwrt".into()),
            vendor: None,
            model: Some("Archer C7".into()),
            firmware: Some("23.05.2".into()),
            status: "passed".into(),
            validated_at: "2026-09-30T00:00:00Z".into(),
            plugin_version: "1.0.0".into(),
            run_id: None,
        });
        assert_eq!(antes, pacote.checksum());
        pacote.script.push(' ');
        assert_ne!(antes, pacote.checksum());
    }
}
