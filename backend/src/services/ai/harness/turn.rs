//! Quanto a pergunta precisa, decidido antes de gastar tokens.
//!
//! Toda chamada ao provedor leva o system prompt, o histórico e os contratos
//! das ferramentas oferecidas. Um "oi" com o catálogo inteiro custou 22 mil
//! tokens (três consultas inúteis e quatro rodadas). As regras aqui cortam
//! isso sem tirar nada de uma pergunta de verdade:
//!
//! - [`is_small_talk`]: saudação e agradecimento respondem sem ferramenta.
//! - [`preselect_groups`]: o que a pergunta obviamente pede já vai carregado;
//!   o resto a IA carrega com `load_tools` se precisar.
//! - [`compact_for_model`]: resultado gigante não entra inteiro na conversa.

use std::collections::HashSet;

use serde_json::Value;

use super::tools::{ToolGroup, ToolGroups};
use crate::dtos::ai::{AiMention, AiMentionKind, ChatMessageInput};

/// Palavras que, sozinhas, não pedem dado nenhum.
const SMALL_TALK_WORDS: &[&str] = &[
    "oi", "ola", "opa", "eae", "e", "ai", "bom", "boa", "dia", "tarde", "noite", "tudo", "bem",
    "td", "blz", "beleza", "joia", "obrigado", "obrigada", "obg", "brigado", "valeu", "vlw", "ok",
    "okay", "certo", "entendi", "show", "legal", "perfeito", "top", "massa", "tchau", "ate",
    "mais", "logo", "hi", "hello", "hey", "thanks", "thank", "you", "como", "vai", "voce", "vc",
    "muito", "pessoal",
];

/// Frase mais longa que isso já é pergunta, mesmo com palavras de cortesia.
const MAX_SMALL_TALK_WORDS: usize = 6;

/// Resultado de ferramenta devolvido à IA, em caracteres.
pub const MAX_TOOL_RESULT_CHARS: usize = 8_000;

/// Minúsculas sem acento, para comparar palavras.
fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn words(text: &str) -> Vec<String> {
    fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

/// A última mensagem é só cortesia ("oi", "bom dia, tudo bem?", "valeu").
///
/// Não vale quando a IA acabou de perguntar algo: ali, "ok" é resposta — e
/// pode ser o "pode rodar o ping" que ela esperava.
#[must_use]
pub fn is_small_talk(messages: &[ChatMessageInput]) -> bool {
    let Some((last, before)) = messages.split_last() else {
        return false;
    };
    if last.role != "user" {
        return false;
    }
    let asked = before
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .is_some_and(|message| message.content.contains('?'));
    if asked {
        return false;
    }
    let words = words(&last.content);
    !words.is_empty()
        && words.len() <= MAX_SMALL_TALK_WORDS
        && words
            .iter()
            .all(|word| SMALL_TALK_WORDS.contains(&word.as_str()))
}

/// Trechos que denunciam cada grupo. Casam no começo da palavra, sem acento.
const HINTS: &[(ToolGroup, &[&str])] = &[
    (
        ToolGroup::History,
        &[
            "histor", "uptime", "disponib", "ontem", "semana", "mes", "metric", "cpu", "memoria",
            "trafego", "banda", "ultimas", "ultimos", "24h", "7d", "horas", "dias", "falhas",
        ],
    ),
    (
        ToolGroup::Charts,
        &["grafic", "chart", "curva", "evolu", "plot"],
    ),
    (
        ToolGroup::Analysis,
        &[
            "causa",
            "raiz",
            "porque",
            "normal",
            "baseline",
            "padrao",
            "horario",
            "aconteceu",
            "timeline",
            "incidente",
            "pior",
            "piorou",
            "comparar",
            "compare",
        ],
    ),
    (ToolGroup::Logs, &["log", "logs", "syslog"]),
    (ToolGroup::Docker, &["docker", "container", "containers"]),
    (
        ToolGroup::Diagnostics,
        &[
            "ping",
            "traceroute",
            "tracert",
            "rota",
            "porta",
            "portas",
            "scan",
            "dns",
            "resolv",
            "testar",
            "teste",
            "conectividade",
            "internet",
            "playbook",
            "latencia",
        ],
    ),
    (
        ToolGroup::Actions,
        &[
            "silenci", "reconhec", "manutenc", "criar", "crie", "cadastr", "adicion",
        ],
    ),
    (
        ToolGroup::AlertRules,
        &[
            "regra",
            "limiar",
            "threshold",
            "dispar",
            "origem",
            "gatilho",
            "apagar",
            "exclu",
            "remov",
        ],
    ),
];

/// Grupos que a pergunta (e o que foi marcado nela) obviamente pede.
///
/// Errar para mais custa só os schemas de um grupo; errar para menos custa
/// uma rodada de `load_tools` — nunca a resposta.
#[must_use]
pub fn preselect_groups(question: &str, mentions: &[AiMention]) -> ToolGroups {
    let words = words(question);
    let mut groups: ToolGroups = HINTS
        .iter()
        .filter(|(_, hints)| {
            words
                .iter()
                .any(|word| hints.iter().any(|hint| word.starts_with(hint)))
        })
        .map(|(group, _)| *group)
        .collect();
    let joined = words.join(" ");
    if joined.contains("por que") || joined.contains("linha do tempo") {
        groups.insert(ToolGroup::Analysis);
    }
    for mention in mentions {
        match (mention.kind, mention.id.as_str()) {
            (AiMentionKind::Container, _) | (AiMentionKind::Source, "docker") => {
                groups.insert(ToolGroup::Docker);
            }
            (AiMentionKind::Monitor, _) => {
                groups.insert(ToolGroup::History);
            }
            (AiMentionKind::Source, "logs") => {
                groups.insert(ToolGroup::Logs);
            }
            _ => {}
        }
    }
    groups
}

/// Os grupos pedidos no `load_tools`, sem repetir e sem inventar.
#[must_use]
pub fn requested_groups(arguments: &Value) -> Vec<ToolGroup> {
    let mut seen = HashSet::new();
    let items = match arguments.get("groups") {
        Some(Value::Array(items)) => items.clone(),
        Some(single) => vec![single.clone()],
        None => Vec::new(),
    };
    items
        .iter()
        .filter_map(Value::as_str)
        .filter_map(ToolGroup::from_id)
        .filter(|group| seen.insert(*group))
        .collect()
}

/// O resultado como a IA o recebe: inteiro quando cabe, cortado com aviso
/// quando não — o aviso diz como pedir menos.
#[must_use]
pub fn compact_for_model(result: &Value) -> String {
    let text = result.to_string();
    if text.chars().count() <= MAX_TOOL_RESULT_CHARS {
        return text;
    }
    let cut: String = text.chars().take(MAX_TOOL_RESULT_CHARS).collect();
    format!(
        "{cut}… [resultado cortado em {MAX_TOOL_RESULT_CHARS} de {} caracteres; refine os filtros para ver o resto]",
        text.chars().count()
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn msg(role: &str, content: &str) -> ChatMessageInput {
        ChatMessageInput {
            role: role.into(),
            content: content.into(),
        }
    }

    #[test]
    fn cortesia_nao_pede_ferramenta() {
        for texto in [
            "Oi",
            "olá!",
            "Bom dia, tudo bem?",
            "valeu",
            "obrigado 👍",
            "E aí",
        ] {
            assert!(is_small_talk(&[msg("user", texto)]), "{texto}");
        }
    }

    #[test]
    fn pergunta_de_verdade_nao_e_cortesia() {
        for texto in [
            "oi, o link caiu?",
            "bom dia, como está a borda?",
            "status",
            "",
            "ok ok ok ok ok ok ok",
        ] {
            assert!(!is_small_talk(&[msg("user", texto)]), "{texto}");
        }
    }

    #[test]
    fn ok_depois_de_uma_pergunta_da_ia_e_resposta() {
        let conversa = [
            msg("user", "a borda está lenta"),
            msg("assistant", "Quer que eu rode um ping?"),
            msg("user", "ok"),
        ];
        assert!(!is_small_talk(&conversa));
    }

    #[test]
    fn pergunta_pre_carrega_o_grupo_que_obviamente_pede() {
        let grupos = preselect_groups("Mostre o gráfico de latência das últimas 24h", &[]);
        assert!(grupos.contains(&ToolGroup::Charts));
        assert!(grupos.contains(&ToolGroup::History));
        assert!(
            grupos.contains(&ToolGroup::Diagnostics),
            "latência sugere teste"
        );

        assert!(preselect_groups("Por que a borda caiu?", &[]).contains(&ToolGroup::Analysis));
        assert!(preselect_groups("quais dispositivos estão offline?", &[]).is_empty());

        let container = AiMention {
            kind: AiMentionKind::Container,
            id: "abc".into(),
            label: "api".into(),
            detail: None,
        };
        assert_eq!(
            preselect_groups("e esse aqui?", &[container]),
            ToolGroups::from([ToolGroup::Docker])
        );
    }

    #[test]
    fn load_tools_aceita_grupo_solto_e_ignora_inventado() {
        assert_eq!(
            requested_groups(&json!({ "groups": ["charts", "CHARTS", "etc", "core"] })),
            vec![ToolGroup::Charts]
        );
        assert_eq!(
            requested_groups(&json!({ "groups": "history" })),
            vec![ToolGroup::History]
        );
    }

    #[test]
    fn resultado_grande_e_cortado_com_aviso() {
        let pequeno = json!({ "total": 1 });
        assert_eq!(compact_for_model(&pequeno), pequeno.to_string());

        let grande = json!({ "texto": "x".repeat(MAX_TOOL_RESULT_CHARS * 2) });
        let cortado = compact_for_model(&grande);
        assert!(cortado.contains("resultado cortado"));
        assert!(cortado.chars().count() < MAX_TOOL_RESULT_CHARS + 200);
    }
}
