//! Sinais de uma conversa: o que deu certo e o que falhou, lido das mensagens
//! como a tela as guardou.
//!
//! Não há avaliação humana ainda — a classificação é automática e só olha o
//! que é inequívoco: resposta que terminou em erro e ferramenta que falhou.
//! Ação recusada pelo usuário é contada à parte: pode ser sugestão errada da
//! IA ou só mudança de ideia, e quem revisa decide.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

/// Classificação automática da conversa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// Nenhuma resposta com erro e nenhuma ferramenta falhou.
    NoFailures,
    /// Ao menos uma resposta com erro ou uma ferramenta que falhou.
    WithFailures,
}

/// Onde a conversa falhou, para quem revisa ir direto ao ponto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    /// Posição da mensagem na conversa (a partir de 0).
    pub message_index: usize,
    /// `response` (a resposta toda) ou `tool` (uma ferramenta).
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub detail: String,
}

/// Contagens de uma conversa.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Signals {
    pub questions: usize,
    pub answers: usize,
    pub tool_calls: usize,
    pub tools_succeeded: usize,
    pub tools_failed: usize,
    /// Ações que pediram confirmação e o usuário recusou.
    pub actions_declined: usize,
    /// Ações que ficaram esperando confirmação (a conversa parou ali).
    pub actions_pending: usize,
    pub answers_with_error: usize,
    pub notices: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Modelos que responderam, em ordem alfabética.
    pub models: BTreeSet<String>,
    pub failures: Vec<Failure>,
}

impl Signals {
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if self.failures.is_empty() {
            Outcome::NoFailures
        } else {
            Outcome::WithFailures
        }
    }
}

/// Texto não vazio do campo, sem espaços nas pontas.
pub(super) fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// Lista do campo, ou vazia.
pub(super) fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn tokens(usage: &Value, key: &str) -> u64 {
    usage.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn count_tool(signals: &mut Signals, index: usize, tool: &Value) {
    signals.tool_calls += 1;
    let name = text(tool, "name").unwrap_or("?");
    match text(tool, "status").unwrap_or_default() {
        "done" => signals.tools_succeeded += 1,
        "error" => {
            signals.tools_failed += 1;
            let detail = tool
                .get("result")
                .and_then(|result| text(result, "error"))
                .unwrap_or("Falhou sem mensagem de erro");
            signals.failures.push(Failure {
                message_index: index,
                kind: "tool",
                tool: Some(name.to_owned()),
                detail: detail.to_owned(),
            });
        }
        "cancelled" => signals.actions_declined += 1,
        "awaiting" => signals.actions_pending += 1,
        _ => {}
    }
}

/// Lê os sinais das mensagens guardadas. Campo ausente ou de outro tipo
/// conta como vazio: a exportação nunca para por uma conversa antiga.
#[must_use]
pub fn analyze(messages: &[Value]) -> Signals {
    let mut signals = Signals::default();
    for (index, message) in messages.iter().enumerate() {
        match text(message, "role") {
            Some("user") => signals.questions += 1,
            Some("assistant") => signals.answers += 1,
            _ => continue,
        }
        for tool in list(message, "toolCalls") {
            count_tool(&mut signals, index, tool);
        }
        if let Some(error) = text(message, "error") {
            signals.answers_with_error += 1;
            signals.failures.push(Failure {
                message_index: index,
                kind: "response",
                tool: None,
                detail: error.to_owned(),
            });
        }
        if text(message, "notice").is_some() {
            signals.notices += 1;
        }
        if let Some(usage) = message.get("usage").filter(|usage| usage.is_object()) {
            signals.prompt_tokens += tokens(usage, "promptTokens");
            signals.completion_tokens += tokens(usage, "completionTokens");
            if let Some(model) = text(usage, "model") {
                signals.models.insert(model.to_owned());
            }
        }
    }
    signals
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn conversa_limpa_nao_tem_falhas() {
        let messages = [
            json!({ "role": "user", "content": "como está o gateway?" }),
            json!({
                "role": "assistant",
                "content": "Online, 2 ms.",
                "toolCalls": [{ "name": "ping", "status": "done", "result": { "ok": true } }],
                "usage": { "promptTokens": 120, "completionTokens": 30, "model": "qwen3" },
            }),
        ];
        let signals = analyze(&messages);
        assert_eq!(signals.questions, 1);
        assert_eq!(signals.answers, 1);
        assert_eq!(signals.tools_succeeded, 1);
        assert_eq!(signals.prompt_tokens, 120);
        assert_eq!(signals.completion_tokens, 30);
        assert_eq!(signals.models.iter().collect::<Vec<_>>(), ["qwen3"]);
        assert_eq!(signals.outcome(), Outcome::NoFailures);
    }

    #[test]
    fn erro_de_resposta_e_de_ferramenta_viram_falhas_com_posicao() {
        let messages = [
            json!({ "role": "user", "content": "reinicie o AP" }),
            json!({
                "role": "assistant",
                "content": "",
                "toolCalls": [
                    { "name": "device_reboot", "status": "error", "result": { "error": "timeout" } },
                    { "name": "silence_alert", "status": "cancelled" },
                    { "name": "run_plugin", "status": "awaiting" },
                ],
            }),
            json!({ "role": "assistant", "content": "", "error": "Provedor fora do ar" }),
        ];
        let signals = analyze(&messages);
        assert_eq!(signals.tools_failed, 1);
        assert_eq!(signals.actions_declined, 1);
        assert_eq!(signals.actions_pending, 1);
        assert_eq!(signals.answers_with_error, 1);
        assert_eq!(signals.outcome(), Outcome::WithFailures);
        assert_eq!(
            signals.failures,
            vec![
                Failure {
                    message_index: 1,
                    kind: "tool",
                    tool: Some("device_reboot".into()),
                    detail: "timeout".into(),
                },
                Failure {
                    message_index: 2,
                    kind: "response",
                    tool: None,
                    detail: "Provedor fora do ar".into(),
                },
            ]
        );
    }

    #[test]
    fn mensagem_malformada_nao_quebra_a_leitura() {
        let messages = [
            json!("texto solto"),
            json!({ "role": "assistant", "toolCalls": "não é lista", "usage": 3 }),
        ];
        let signals = analyze(&messages);
        assert_eq!(signals.answers, 1);
        assert_eq!(signals.tool_calls, 0);
        assert_eq!(signals.outcome(), Outcome::NoFailures);
    }
}
