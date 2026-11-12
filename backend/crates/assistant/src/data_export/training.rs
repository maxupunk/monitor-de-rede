//! Conversa no formato de treino: o `messages` de chat com `tool_calls`, o
//! mesmo que OpenAI, Unsloth, Axolotl e afins leem em JSONL.
//!
//! Sai só o que o modelo veria: pergunta, chamada de ferramenta, resultado e
//! resposta. Gráfico, uso de tokens, marcações e avisos são da tela e ficam
//! no arquivo de análise. O prompt de sistema não é guardado com a conversa
//! (é montado a cada pergunta), então não aparece aqui.

use serde_json::{json, Value};

use super::signals::{list, text};

/// O que o modelo recebe de volta de uma ferramenta que não rodou.
fn tool_content(tool: &Value) -> String {
    let status = text(tool, "status").unwrap_or_default();
    let result = tool.get("result").filter(|result| !result.is_null());
    match (status, result) {
        ("cancelled", _) => json!({ "status": "declined_by_user" }).to_string(),
        ("awaiting", _) => json!({ "status": "awaiting_confirmation" }).to_string(),
        (_, Some(result)) => result.to_string(),
        _ => json!({ "status": "no_result" }).to_string(),
    }
}

fn arguments(tool: &Value) -> String {
    tool.get("arguments")
        .filter(|arguments| arguments.is_object())
        .map_or_else(|| "{}".to_owned(), Value::to_string)
}

fn push_assistant(out: &mut Vec<Value>, message: &Value) {
    let tools = list(message, "toolCalls");
    if !tools.is_empty() {
        let calls: Vec<Value> = tools
            .iter()
            .enumerate()
            .map(|(position, tool)| {
                json!({
                    "id": call_id(tool, position),
                    "type": "function",
                    "function": {
                        "name": text(tool, "name").unwrap_or("unknown"),
                        "arguments": arguments(tool),
                    },
                })
            })
            .collect();
        out.push(json!({ "role": "assistant", "content": "", "tool_calls": calls }));
        for (position, tool) in tools.iter().enumerate() {
            out.push(json!({
                "role": "tool",
                "tool_call_id": call_id(tool, position),
                "content": tool_content(tool),
            }));
        }
    }
    if let Some(content) = text(message, "content") {
        out.push(json!({ "role": "assistant", "content": content }));
    }
}

fn call_id(tool: &Value, position: usize) -> String {
    text(tool, "id").map_or_else(|| format!("call_{position}"), str::to_owned)
}

/// As mensagens da conversa no formato de treino. Mensagem sem papel
/// conhecido ou vazia é pulada; conversa sem nenhuma resposta vira `None` —
/// ela não ensina nada.
#[must_use]
pub fn to_chat_example(messages: &[Value]) -> Option<Value> {
    let mut out = Vec::with_capacity(messages.len());
    for message in messages {
        match text(message, "role") {
            Some("user") => {
                if let Some(content) = text(message, "content") {
                    out.push(json!({ "role": "user", "content": content }));
                }
            }
            Some("assistant") => push_assistant(&mut out, message),
            _ => {}
        }
    }
    let answered = out
        .iter()
        .any(|message| message["role"] == "assistant" || message["role"] == "tool");
    let asked = out.iter().any(|message| message["role"] == "user");
    (asked && answered).then(|| json!({ "messages": out }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ferramentas_viram_tool_calls_e_resultados() {
        let messages = [
            json!({ "role": "user", "content": "ping no gateway", "mentions": [] }),
            json!({
                "role": "assistant",
                "content": "Respondeu em 2 ms.",
                "toolCalls": [{
                    "id": "c1",
                    "name": "ping",
                    "arguments": { "host": "10.0.0.1" },
                    "result": { "latencyMs": 2 },
                    "status": "done",
                    "chart": { "kind": "line" },
                }],
                "usage": { "promptTokens": 10 },
            }),
        ];
        assert_eq!(
            to_chat_example(&messages).unwrap(),
            json!({ "messages": [
                { "role": "user", "content": "ping no gateway" },
                { "role": "assistant", "content": "", "tool_calls": [{
                    "id": "c1",
                    "type": "function",
                    "function": { "name": "ping", "arguments": "{\"host\":\"10.0.0.1\"}" },
                }]},
                { "role": "tool", "tool_call_id": "c1", "content": "{\"latencyMs\":2}" },
                { "role": "assistant", "content": "Respondeu em 2 ms." },
            ]})
        );
    }

    #[test]
    fn acao_recusada_ou_pendente_diz_o_motivo_ao_modelo() {
        let messages = [
            json!({ "role": "user", "content": "silencie" }),
            json!({ "role": "assistant", "toolCalls": [
                { "name": "silence", "status": "cancelled" },
                { "id": "x", "name": "reboot", "status": "awaiting" },
            ]}),
        ];
        let example = to_chat_example(&messages).unwrap();
        let tools: Vec<&Value> = example["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|message| message["role"] == "tool")
            .collect();
        assert_eq!(tools[0]["tool_call_id"], "call_0");
        assert_eq!(tools[0]["content"], "{\"status\":\"declined_by_user\"}");
        assert_eq!(tools[1]["tool_call_id"], "x");
        assert_eq!(
            tools[1]["content"],
            "{\"status\":\"awaiting_confirmation\"}"
        );
    }

    #[test]
    fn conversa_sem_resposta_nao_vira_exemplo() {
        let messages = [
            json!({ "role": "user", "content": "oi" }),
            json!({ "role": "assistant", "content": "", "error": "Provedor fora do ar" }),
        ];
        assert!(to_chat_example(&messages).is_none());
        assert!(to_chat_example(&[]).is_none());
    }
}
