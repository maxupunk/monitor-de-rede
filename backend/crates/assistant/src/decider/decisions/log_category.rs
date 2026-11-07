//! O que um padrão de log significa: falha de login, link caindo, reboot…
//!
//! Hoje nenhum log tem significado — só 7 regex de alerta, e nenhuma pega
//! "link down". A categoria vem uma vez por **padrão** (ver
//! `syslog::template`), nunca por linha.

use std::collections::BTreeMap;

pub use netmonitor_core::classifier::log_category::{is_category, LogPattern, LOG_CATEGORIES};

use crate::decider::{
    decision::Decision,
    schema::{Answer, Question},
    suggestion::{DeciderSuggestion, FromChoice},
};

pub const CATEGORY: &str = "category";

pub struct LogCategory {
    pub min_confidence: u8,
}

impl Decision for LogCategory {
    type Output = Option<DeciderSuggestion>;

    fn questions(&self) -> BTreeMap<String, Question> {
        let criteria = LOG_CATEGORIES
            .iter()
            .map(|(id, hint)| ((*id).to_string(), (*hint).to_string()))
            .collect();
        BTreeMap::from([(
            CATEGORY.to_string(),
            Question::Choice {
                instructions: "Which category best describes this log message?".to_string(),
                criteria,
            },
        )])
    }

    fn interpret(&self, answers: &BTreeMap<String, Answer>) -> Option<DeciderSuggestion> {
        DeciderSuggestion::from_choice(answers.get(CATEGORY), "", self.min_confidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uma_escolha_com_todas_as_categorias() {
        let questions = LogCategory { min_confidence: 60 }.questions();
        let Question::Choice { criteria, .. } = &questions[CATEGORY] else {
            panic!("categoria é escolha");
        };
        assert_eq!(criteria.len(), LOG_CATEGORIES.len());
        assert!(is_category("link_change") && !is_category("nada"));
    }
}
