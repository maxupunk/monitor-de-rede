//! O `.env` de um alvo: ler as chaves e comparar duas versões.
//!
//! O arquivo é guardado **como o operador colou** — comentários, ordem e
//! linhas em branco incluídos —, porque é ele que o agente grava. Esta
//! leitura só serve para mostrar o que mudou e para listar as chaves no
//! histórico sem decifrar valores.

use std::collections::BTreeMap;

use serde::Serialize;
use ts_rs::TS;

use netmonitor_core::shared::errors::{AppError, AppResult};

/// Teto do arquivo: um `.env` maior que isso é engano (colaram um dump).
pub const MAX_ENV_BYTES: usize = 64 * 1024;

/// As atribuições `CHAVE=valor`, na ordem do arquivo; a última vence.
#[must_use]
pub fn parse(content: &str) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        entries.insert(key.to_string(), unquote(value.trim()).to_string());
    }
    entries
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return &value[1..value.len() - 1];
        }
    }
    value
}

/// As chaves, em ordem alfabética.
#[must_use]
pub fn keys(content: &str) -> Vec<String> {
    parse(content).into_keys().collect()
}

/// Confere o tamanho e as linhas que não são nem comentário nem `CHAVE=valor`.
///
/// # Errors
///
/// `Validation` com a primeira linha inválida, para o operador achar.
pub fn validate(content: &str) -> AppResult<()> {
    if content.len() > MAX_ENV_BYTES {
        return Err(AppError::validation(format!(
            "O .env passa de {} KiB",
            MAX_ENV_BYTES / 1024
        )));
    }
    for (number, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let valid = line.split_once('=').is_some_and(|(key, _)| {
            let key = key.trim();
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
        if !valid {
            return Err(AppError::validation(format!(
                "Linha {} do .env não é CHAVE=valor: \"{}\"",
                number + 1,
                line.chars().take(40).collect::<String>()
            )));
        }
    }
    Ok(())
}

/// O que mudou de uma versão para a outra. Só nomes: o diff vai à tela e ao
/// log de auditoria, e valor de `.env` é senha.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployEnvDiff"
)]
pub struct EnvDiff {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
}

impl EnvDiff {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }

    /// "2 adicionadas, 1 alterada" — para a auditoria e a nota da versão.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        let mut push = |count: usize, one: &str, many: &str| {
            if count > 0 {
                parts.push(format!("{count} {}", if count == 1 { one } else { many }));
            }
        };
        push(self.added.len(), "adicionada", "adicionadas");
        push(self.changed.len(), "alterada", "alteradas");
        push(self.removed.len(), "removida", "removidas");
        if parts.is_empty() {
            "sem mudanças nas variáveis".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Compara `before` com `after`.
#[must_use]
pub fn diff(before: &str, after: &str) -> EnvDiff {
    let before = parse(before);
    let after = parse(after);
    let mut output = EnvDiff::default();
    for (key, value) in &after {
        match before.get(key) {
            None => output.added.push(key.clone()),
            Some(old) if old != value => output.changed.push(key.clone()),
            Some(_) => {}
        }
    }
    output.removed = before
        .keys()
        .filter(|key| !after.contains_key(*key))
        .cloned()
        .collect();
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENV: &str = "# banco\nDB_HOST=postgres\nexport DB_PASS=\"s3nh4\"\n\nAPP_KEY='abc'\n";

    #[test]
    fn le_atribuicoes_e_ignora_comentarios() {
        let entries = parse(ENV);
        assert_eq!(entries.get("DB_HOST").map(String::as_str), Some("postgres"));
        assert_eq!(entries.get("DB_PASS").map(String::as_str), Some("s3nh4"));
        assert_eq!(entries.get("APP_KEY").map(String::as_str), Some("abc"));
        assert_eq!(keys(ENV), vec!["APP_KEY", "DB_HOST", "DB_PASS"]);
    }

    #[test]
    fn diff_so_com_nomes() {
        let after = "DB_HOST=postgres\nDB_PASS=outra\nNOVA=1\n";
        let diff = diff(ENV, after);
        assert_eq!(diff.added, vec!["NOVA"]);
        assert_eq!(diff.changed, vec!["DB_PASS"]);
        assert_eq!(diff.removed, vec!["APP_KEY"]);
        assert_eq!(diff.summary(), "1 adicionada, 1 alterada, 1 removida");
        assert!(!format!("{diff:?}").contains("outra"));
    }

    #[test]
    fn mudar_so_comentario_nao_e_mudanca() {
        assert!(diff(ENV, &format!("# novo comentário\n{ENV}")).is_empty());
    }

    #[test]
    fn linha_invalida_aponta_o_numero() {
        let error = validate("A=1\nisto não é env\n").expect_err("inválido");
        assert!(error.to_string().contains("Linha 2"));
        assert!(validate(ENV).is_ok());
        assert!(validate(&"A=1\n".repeat(MAX_ENV_BYTES)).is_err());
    }
}
