//! A IA pergunta antes de prosseguir.
//!
//! Quando não está claro de qual equipamento vem a informação, olhar o
//! aparelho errado custa mais (em tokens e em confiança) do que perguntar. A
//! pergunta aparece no chat com as opções clicáveis e a rodada termina ali:
//! nada de gastar outra chamada ao provedor esperando uma resposta que só o
//! usuário tem.

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};

use super::{log_digest::truncate_chars, AiToolHandler, ToolArgs, ToolKind, ToolOutput};
use crate::services::shared::errors::AppResult;

const MAX_OPTIONS: usize = 6;
const MAX_QUESTION_CHARS: usize = 300;
const MAX_OPTION_CHARS: usize = 80;

/// Pergunta e opções já limpas: sem repetição, curtas e no máximo [`MAX_OPTIONS`].
fn question_payload(question: &str, options: Vec<String>) -> Value {
    let mut unique: Vec<String> = Vec::new();
    for option in options {
        let option = truncate_chars(&option, MAX_OPTION_CHARS);
        if !unique.iter().any(|seen| seen.eq_ignore_ascii_case(&option)) {
            unique.push(option);
        }
    }
    unique.truncate(MAX_OPTIONS);
    json!({
        "status": "awaiting_user_answer",
        "question": truncate_chars(question, MAX_QUESTION_CHARS),
        "options": unique,
    })
}

pub struct AskUser;

#[async_trait]
impl AiToolHandler for AskUser {
    fn name(&self) -> &'static str {
        "ask_user"
    }

    fn description(&self) -> &'static str {
        "Pergunta ao usuário antes de prosseguir, com opções clicáveis. Use quando não estiver claro de qual dispositivo ou recurso vem a informação, \
em vez de adivinhar. A rodada termina aqui: não escreva mais nada depois de chamar."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "A pergunta, curta" },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Até 6 respostas prováveis (ex: nomes de dispositivos de list_devices). O usuário também pode responder livremente."
                }
            },
            "required": ["question"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Interactive
    }

    async fn execute(&self, _ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let Some(question) = args.text("question") else {
            return Ok(ToolOutput::not_found("Informe a pergunta em 'question'"));
        };
        Ok(ToolOutput::awaiting_user(question_payload(
            &question,
            args.texts("options"),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcoes_repetidas_somem_e_o_teto_vale() {
        let opcoes = ["Borda", "borda", "MPPT", "Core", "A", "B", "C", "D"]
            .map(String::from)
            .to_vec();
        let pergunta = question_payload("Qual equipamento?", opcoes);
        assert_eq!(pergunta["status"], "awaiting_user_answer");
        assert_eq!(
            pergunta["options"],
            json!(["Borda", "MPPT", "Core", "A", "B", "C"])
        );
    }
}
