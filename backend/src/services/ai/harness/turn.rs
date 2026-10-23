//! Quanto a pergunta precisa, decidido antes de gastar tokens.
//!
//! Toda chamada ao provedor leva o system prompt, o histórico e os contratos
//! das ferramentas oferecidas. Um "oi" com o catálogo inteiro custou 22 mil
//! tokens (três consultas inúteis e quatro rodadas). As regras aqui cortam
//! isso sem tirar nada de uma pergunta de verdade:
//!
//! - [`is_small_talk`]: saudação e agradecimento respondem sem ferramenta.
//! - [`context_groups`]: o que a pessoa marcou (`@container`, o alerta da
//!   tela) já vai carregado; o resto quem escolhe é o Decisor — ou, sem ele,
//!   vai o catálogo inteiro (`agent::run_agent_loop`).
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
pub const MAX_TOOL_RESULT_CHARS: usize = 4_000;

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

/// Grupos que o contexto **marcado** pela pessoa pede: o container, o
/// monitor ou a fonte citada com `@`, e o alerta ou a regra da tela de onde o
/// chat foi aberto. Não é palpite sobre o texto — quem escolhe pelo texto é o
/// Decisor ([`tool_routing`](crate::services::ai::decider::tool_routing)).
#[must_use]
pub fn context_groups(mentions: &[AiMention], alert_or_rule: bool) -> ToolGroups {
    let mut groups = ToolGroups::new();
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
    if alert_or_rule {
        groups.insert(ToolGroup::AlertRules);
        groups.insert(ToolGroup::Analysis);
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

/// Marcador que substitui os itens cortados de uma lista.
const OMITTED_KEY: &str = "_omitidos";

/// Tamanho serializado de um valor, em caracteres.
fn size(value: &Value) -> usize {
    value.to_string().chars().count()
}

/// Passo do caminho até um valor dentro do JSON.
#[derive(Clone)]
enum Step {
    Key(String),
    Index(usize),
}

/// Caminho da lista mais pesada que ainda dá para encurtar (mais de um item
/// que não seja o marcador).
fn heaviest_array_path(value: &Value) -> Option<Vec<Step>> {
    let mut best: Option<(usize, Vec<Step>)> = None;
    let mut stack: Vec<(&Value, Vec<Step>)> = vec![(value, Vec::new())];
    while let Some((node, path)) = stack.pop() {
        match node {
            Value::Array(items) => {
                let real = items
                    .iter()
                    .filter(|item| item.get(OMITTED_KEY).is_none())
                    .count();
                let weight = items.iter().map(size).sum::<usize>();
                if real > 1 && best.as_ref().is_none_or(|(heaviest, _)| weight > *heaviest) {
                    best = Some((weight, path.clone()));
                }
                for (index, item) in items.iter().enumerate() {
                    let mut child = path.clone();
                    child.push(Step::Index(index));
                    stack.push((item, child));
                }
            }
            Value::Object(map) => {
                for (key, item) in map {
                    let mut child = path.clone();
                    child.push(Step::Key(key.clone()));
                    stack.push((item, child));
                }
            }
            _ => {}
        }
    }
    best.map(|(_, path)| path)
}

fn array_at<'a>(value: &'a mut Value, path: &[Step]) -> Option<&'a mut Vec<Value>> {
    let mut node = value;
    for step in path {
        node = match step {
            Step::Key(key) => node.get_mut(key.as_str())?,
            Step::Index(index) => node.get_mut(*index)?,
        };
    }
    node.as_array_mut()
}

/// Corta a lista pela metade e soma o que saiu no marcador do fim.
fn halve(items: &mut Vec<Value>) {
    let already = match items.last().and_then(|item| item.get(OMITTED_KEY)) {
        Some(count) => {
            let count = count.as_u64().unwrap_or(0);
            items.pop();
            count
        }
        None => 0,
    };
    let keep = items.len().div_ceil(2);
    let removed = (items.len() - keep) as u64;
    items.truncate(keep);
    items.push(serde_json::json!({ OMITTED_KEY: already + removed }));
}

/// O resultado como a IA o recebe: inteiro quando cabe; senão as listas mais
/// pesadas são encurtadas (com a contagem do que saiu) até caber, e o JSON
/// continua válido. Só em último caso o texto é cortado.
#[must_use]
pub fn compact_for_model(result: &Value) -> String {
    // Primeiro sem perder nada (tabelas, vazios, casas decimais); só depois,
    // se ainda não couber, encurtar listas.
    let result = &super::lean::lean(result);
    let text = result.to_string();
    if text.chars().count() <= MAX_TOOL_RESULT_CHARS {
        return text;
    }
    let total = text.chars().count();
    let mut shrunk = result.clone();
    while size(&shrunk) > MAX_TOOL_RESULT_CHARS {
        let Some(items) =
            heaviest_array_path(&shrunk).and_then(|path| array_at(&mut shrunk, &path))
        else {
            break;
        };
        halve(items);
    }
    let text = shrunk.to_string();
    if text.chars().count() <= MAX_TOOL_RESULT_CHARS {
        return text;
    }
    let cut: String = text.chars().take(MAX_TOOL_RESULT_CHARS).collect();
    format!(
        "{cut}… [resultado cortado em {MAX_TOOL_RESULT_CHARS} de {total} caracteres; refine os filtros para ver o resto]"
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
    fn contexto_marcado_carrega_o_grupo_sem_olhar_o_texto() {
        let container = AiMention {
            kind: AiMentionKind::Container,
            id: "abc".into(),
            label: "api".into(),
            detail: None,
        };
        assert_eq!(
            context_groups(&[container], false),
            ToolGroups::from([ToolGroup::Docker])
        );
        assert_eq!(
            context_groups(&[], true),
            ToolGroups::from([ToolGroup::AlertRules, ToolGroup::Analysis])
        );
        assert!(context_groups(&[], false).is_empty());
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

        let lista = json!({
            "total": 400,
            "itens": (0..400).map(|i| json!({ "id": i, "nome": format!("dispositivo-{i}") })).collect::<Vec<_>>(),
        });
        let encurtado = compact_for_model(&lista);
        assert!(encurtado.chars().count() <= MAX_TOOL_RESULT_CHARS);
        let lido: Value = serde_json::from_str(&encurtado).expect("continua JSON válido");
        // A lista chega como tabela (`lean`); o encurtamento age nas linhas.
        let itens = lido["itens"]["rows"].as_array().unwrap();
        let omitidos = itens.last().unwrap()[OMITTED_KEY].as_u64().unwrap();
        assert_eq!(
            itens.len() as u64 - 1 + omitidos,
            400,
            "nada some sem ser contado"
        );

        let grande = json!({ "texto": "x".repeat(MAX_TOOL_RESULT_CHARS * 2) });
        let cortado = compact_for_model(&grande);
        assert!(cortado.contains("resultado cortado"));
        assert!(cortado.chars().count() < MAX_TOOL_RESULT_CHARS + 200);
    }
}
