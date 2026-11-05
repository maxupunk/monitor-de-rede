//! Padrão de log casado, traduzido para o vocabulário das regras.
//!
//! Só publica fatos: "o padrão X casou N vezes nos últimos M minutos neste
//! dispositivo". A decisão de alertar — e com qual severidade — continua sendo
//! das regras cadastradas, como em todo dataset.

use serde_json::json;

use crate::services::alerts::{contracts::AlertDataset, fields};

/// O que a janela deslizante do matcher observou para um par (regra, alvo).
#[derive(Debug, Clone)]
pub struct LogPatternFacts<'a> {
    /// Chave do padrão, igual à do template do catálogo (`log_login_failure`).
    pub pattern_key: &'a str,
    /// Ocorrências dentro da janela.
    pub match_count: i64,
    /// Largura da janela, em segundos.
    pub window_seconds: i64,
    /// A severidade mais grave observada entre os casamentos.
    pub severity: Option<i16>,
    /// A última mensagem que casou — é ela que vai para o texto do alerta.
    pub last_message: &'a str,
}

/// Monta os fatos do padrão.
#[must_use]
pub fn build(facts: &LogPatternFacts<'_>) -> AlertDataset {
    let mut dataset = AlertDataset::new();
    dataset.insert(fields::LOG_PATTERN_KEY.into(), json!(facts.pattern_key));
    dataset.insert(fields::LOG_MATCH_COUNT.into(), json!(facts.match_count));
    dataset.insert(
        fields::LOG_WINDOW_SECONDS.into(),
        json!(facts.window_seconds),
    );
    if let Some(severity) = facts.severity {
        dataset.insert(fields::LOG_SEVERITY.into(), json!(severity));
    }
    dataset.insert(fields::LOG_MESSAGE.into(), json!(facts.last_message));
    dataset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_fatos_saem_no_vocabulario_das_regras() {
        let dataset = build(&LogPatternFacts {
            pattern_key: "log_login_failure",
            match_count: 4,
            window_seconds: 300,
            severity: Some(3),
            last_message: "login failure for user admin",
        });

        assert_eq!(dataset[fields::LOG_PATTERN_KEY], json!("log_login_failure"));
        assert_eq!(dataset[fields::LOG_MATCH_COUNT], json!(4));
        assert_eq!(dataset[fields::LOG_WINDOW_SECONDS], json!(300));
        assert_eq!(dataset[fields::LOG_SEVERITY], json!(3));
    }

    #[test]
    fn sem_severidade_o_campo_some_em_vez_de_virar_nulo() {
        // O avaliador trata campo ausente e campo nulo igual (nunca dispara),
        // mas publicar `null` sugeriria que a medida existe e veio vazia.
        let dataset = build(&LogPatternFacts {
            pattern_key: "log_generico",
            match_count: 1,
            window_seconds: 60,
            severity: None,
            last_message: "linha sem severidade",
        });
        assert!(!dataset.contains_key(fields::LOG_SEVERITY));
    }
}
