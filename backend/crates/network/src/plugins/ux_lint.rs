//! Sugestões de usabilidade para a tela de um plugin.
//!
//! A validação ([`super::manifest::validate`]) recusa o que **quebra**; isto
//! aponta o que **funciona mas fica ruim de usar**: formulário com campos
//! demais à vista, opções sem nome, título técnico, senha sem `secret`, IP sem
//! componente de IP, escrita na frota sem pré-visualização. São avisos, não
//! erros — o plugin salva igual —, e vão no relatório de testes, que é o que a
//! IA lê depois de salvar: ela corrige sozinha e roda de novo.
//!
//! Funções puras sobre o pacote (e, quando há, as saídas dos testes, que dizem
//! que chaves a tela vai mostrar).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use super::{
    effect::Effect,
    item_list::{ItemLayout, ItemList},
    manifest::PluginManifest,
    package::PluginPackage,
    widgets::Widget,
};

/// Campos à vista num formulário antes de pedir "Opções avançadas".
pub const MAX_BASIC_FIELDS: usize = 5;
/// Teto de sugestões por pacote — uma lista enorme ninguém lê.
const MAX_HINTS: usize = 30;

/// Uma sugestão: onde (`action.encryption`) e o que fazer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct UxHint {
    pub at: String,
    pub message: String,
}

/// Nomes de campo que denunciam uma senha.
const SECRET_WORDS: &[&str] = &["password", "passwd", "senha", "secret", "token", "psk"];

/// Nome de campo → componente que já o valida.
const WIDGET_WORDS: &[(&str, Widget)] = &[
    ("ip", Widget::Ip),
    ("ipaddr", Widget::Ip),
    ("gateway", Widget::Ip),
    ("gw", Widget::Ip),
    ("dns", Widget::Ip),
    ("mac", Widget::Mac),
    ("port", Widget::Port),
    ("porta", Widget::Port),
    ("host", Widget::Hostname),
    ("hostname", Widget::Hostname),
    ("url", Widget::Url),
    ("cidr", Widget::Cidr),
    ("subnet", Widget::Cidr),
];

/// Final de chave da saída → formato que a tela já sabe escrever.
const FORMAT_WORDS: &[(&str, &str)] = &[
    ("bytes", "bytes"),
    ("bps", "bps"),
    ("latency", "latency"),
    ("rtt", "latency"),
    ("uptime", "uptime"),
    ("percent", "percent"),
];

/// As sugestões do pacote. `outputs` são as saídas dos testes por ação.
#[must_use]
pub fn lint(package: &PluginPackage, outputs: &[(&str, &Value)]) -> Vec<UxHint> {
    let manifest = &package.manifest;
    let mut hints = BTreeSet::new();
    for action in &manifest.actions {
        if looks_technical(&action.title) {
            hints.insert(hint(
                &action.id,
                "o título da ação aparece no botão — escreva em palavras (\"Instalar pacote\")",
            ));
        }
        if let Some(schema) = &action.params {
            lint_form(&action.id, schema, &mut hints);
        }
    }
    if let Some(settings) = &manifest.settings {
        for (scope, schema) in [
            ("settings.fleet", &settings.fleet),
            ("settings.device", &settings.device),
        ] {
            if let Some(schema) = schema {
                lint_form(scope, schema, &mut hints);
            }
        }
    }
    lint_fleet(manifest, &mut hints);
    if let Some(list) = manifest.item_list() {
        lint_list("list", &list, &mut hints);
    }
    for (action_id, output) in outputs {
        lint_output(manifest, action_id, output, &mut hints);
    }
    hints.into_iter().take(MAX_HINTS).collect()
}

fn hint(at: &str, message: &str) -> UxHint {
    UxHint {
        at: at.to_owned(),
        message: message.to_owned(),
    }
}

/// Título que parece identificador: `snake_case`, `camelCase`, `uci.opcao`.
/// Frase com espaço é texto, mesmo citando "dBm" ou "802.11s".
fn looks_technical(title: &str) -> bool {
    let title = title.trim();
    if title.is_empty() || title.contains('_') {
        return true;
    }
    let camel = title
        .as_bytes()
        .windows(2)
        .any(|pair| pair[0].is_ascii_lowercase() && pair[1].is_ascii_uppercase());
    !title.contains(' ') && (title.contains('.') || camel)
}

fn words(name: &str) -> Vec<String> {
    name.to_lowercase()
        .split(['_', '-'])
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

fn lint_form(owner: &str, schema: &Value, hints: &mut BTreeSet<UxHint>) {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    // O limite vale por seção: `group` já divide o formulário em blocos que
    // se leem um de cada vez.
    let mut per_section: BTreeMap<&str, usize> = BTreeMap::new();
    for property in properties.values().filter(|property| {
        !flag(property, "advanced")
            && !flag(property, "hidden")
            && property.get("visibleWhen").is_none()
    }) {
        let section = property.get("group").and_then(Value::as_str).unwrap_or("");
        *per_section.entry(section).or_default() += 1;
    }
    if let Some((section, count)) = per_section
        .iter()
        .find(|(_, count)| **count > MAX_BASIC_FIELDS)
    {
        let place = if section.is_empty() {
            String::new()
        } else {
            format!(" na seção \"{section}\"")
        };
        hints.insert(hint(
            owner,
            &format!(
                "o formulário mostra {count} campos de cara{place}; deixe à vista só o essencial (até {MAX_BASIC_FIELDS} por seção), separe em seções com `group` ou mande o resto para \"Opções avançadas\" com `advanced: true`"
            ),
        ));
    }
    for (name, property) in properties {
        lint_field(&format!("{owner}.{name}"), name, property, hints);
        if let Some(items) = property
            .get("items")
            .filter(|items| items.get("properties").is_some())
        {
            lint_form(&format!("{owner}.{name}[]"), items, hints);
        }
    }
}

fn flag(property: &Value, key: &str) -> bool {
    property.get(key).and_then(Value::as_bool) == Some(true)
}

fn lint_field(at: &str, name: &str, property: &Value, hints: &mut BTreeSet<UxHint>) {
    if flag(property, "hidden") {
        return;
    }
    let title = property.get("title").and_then(Value::as_str).unwrap_or("");
    if looks_technical(title) {
        hints.insert(hint(
            at,
            "dê ao campo um `title` em palavras — é o rótulo que o operador lê",
        ));
    }
    let options = property
        .get("enum")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if options > 1 && property.get("enumTitles").is_none() {
        hints.insert(hint(
            at,
            "dê nome às opções com `enumTitles` (\"WPA2 (aparelhos antigos)\" em vez de \"psk2\")",
        ));
    }
    let kind = property.get("type").and_then(Value::as_str).unwrap_or("");
    let split = words(name);
    let secret_like = split
        .iter()
        .any(|word| SECRET_WORDS.contains(&word.as_str()))
        || name.eq_ignore_ascii_case("key");
    if kind == "string" && secret_like && !flag(property, "secret") {
        hints.insert(hint(
            at,
            "parece uma senha: marque `secret: true` (fica mascarada e fora do histórico)",
        ));
    }
    let constrained = property.get("widget").is_some()
        || property.get("pattern").is_some()
        || property.get("enum").is_some();
    if !constrained {
        let suggestion = WIDGET_WORDS
            .iter()
            .find(|(word, _)| split.iter().any(|part| part == word));
        if let Some((_, widget)) = suggestion {
            let fits = matches!(
                (widget, kind),
                (Widget::Port, "integer")
                    | (
                        Widget::Ip | Widget::Mac | Widget::Hostname | Widget::Url | Widget::Cidr,
                        "string"
                    )
            );
            if fits {
                hints.insert(hint(
                    at,
                    &format!(
                        "use `widget: \"{}\"` — o campo ganha a validação e a dica certas sem `pattern` à mão",
                        widget.name()
                    ),
                ));
            }
        }
    }
}

fn lint_fleet(manifest: &PluginManifest, hints: &mut BTreeSet<UxHint>) {
    let Some(fleet) = &manifest.fleet else {
        return;
    };
    for action in &fleet.actions {
        let at = format!("fleet.{}", action.id);
        let writes = manifest
            .action(&action.action)
            .is_some_and(|device| device.effect == Effect::Write);
        if writes && action.preview.is_none() {
            hints.insert(hint(
                &at,
                "escrita em vários equipamentos sem `preview`: declare uma ação de leitura que mostra o que vai mudar — a revisão vira a confirmação",
            ));
        }
        if looks_technical(&action.title) {
            hints.insert(hint(&at, "escreva o título da ação de frota em palavras"));
        }
        let shown_as_card = fleet.tools.contains(&action.id);
        if shown_as_card && action.description.is_none() {
            hints.insert(hint(
                &at,
                "ferramenta da aba \"Avançado\" sem `description`: explique em uma frase o que ela faz e quando usar",
            ));
        }
    }
    if let Some(list) = fleet.item_list() {
        lint_list("fleet.list", &list, hints);
    }
}

fn lint_list(at: &str, list: &ItemList, hints: &mut BTreeSet<UxHint>) {
    if list.title.is_none() || list.item_name.is_none() {
        hints.insert(hint(
            at,
            "dê `title` (\"Redes\") e `itemName` (\"rede\") à lista — viram o cabeçalho e o botão \"Nova rede\"",
        ));
    }
    if list.layout == ItemLayout::Cards && list.subtitle.is_empty() {
        hints.insert(hint(
            at,
            "cartão sem `subtitle`: mostre abaixo do nome o que importa do item (ex.: segurança, bandas)",
        ));
    }
}

fn lint_output(
    manifest: &PluginManifest,
    action_id: &str,
    output: &Value,
    hints: &mut BTreeSet<UxHint>,
) {
    let Some(action) = manifest.action(action_id) else {
        return;
    };
    let mut keys = BTreeSet::new();
    collect_keys(output, &mut keys, 0);
    let labelled = |key: &str| {
        action
            .labels
            .as_ref()
            .is_some_and(|labels| labels.contains_key(key))
    };
    let formatted = |key: &str| {
        action
            .formats
            .as_ref()
            .is_some_and(|formats| formats.contains_key(key))
    };
    for key in keys {
        let at = format!("{action_id}.saída.{key}");
        if key.contains('_') && !labelled(&key) {
            hints.insert(hint(
                &at,
                "chave técnica na tela: dê um título com `labels` (`\"rx_bytes\": \"Recebido\"`)",
            ));
        }
        let lower = key.to_lowercase();
        let suggestion = FORMAT_WORDS
            .iter()
            .find(|(word, _)| lower == *word || lower.ends_with(&format!("_{word}")));
        if let Some((_, format)) = suggestion {
            if !formatted(&key) {
                hints.insert(hint(
                    &at,
                    &format!("use `formats: {{ \"{key}\": \"{format}\" }}` para a tela escrever o valor como o resto do sistema"),
                ));
            }
        }
    }
}

/// Chaves que a tela mostra: as do objeto e as das linhas de tabelas, até dois
/// níveis. Chave com `_` na frente é dado para a tela, não aparece.
fn collect_keys(value: &Value, keys: &mut BTreeSet<String>, depth: usize) {
    if depth > 2 {
        return;
    }
    match value {
        Value::Object(map) => collect_object(map, keys, depth),
        Value::Array(list) => {
            for item in list.iter().take(5) {
                collect_keys(item, keys, depth + 1);
            }
        }
        _ => {}
    }
}

fn collect_object(map: &Map<String, Value>, keys: &mut BTreeSet<String>, depth: usize) {
    for (key, item) in map {
        if key.starts_with('_') || key == "settings_patch" || key == "next" {
            continue;
        }
        keys.insert(key.clone());
        collect_keys(item, keys, depth + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pacote(manifest: Value) -> PluginPackage {
        serde_json::from_value(json!({
            "manifest": manifest, "script": "fn detect(device, params) { #{} }",
            "usage": "# x", "tests": { "unit": [] }
        }))
        .unwrap()
    }

    fn mensagens(hints: &[UxHint]) -> String {
        hints
            .iter()
            .map(|hint| format!("{}: {}", hint.at, hint.message))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn formulario_ruim_ganha_uma_sugestao_por_problema() {
        let pacote = pacote(json!({
            "slug": "x", "name": "X", "version": "1.0.0", "transports": ["ssh"],
            "actions": [
                { "id": "detect", "title": "Detectar", "effect": "read" },
                { "id": "set_lan", "title": "set_lan", "effect": "write", "output": "kv",
                  "params": { "type": "object", "properties": {
                      "ip": { "type": "string", "title": "IP" },
                      "gateway": { "type": "string", "title": "Gateway" },
                      "admin_password": { "type": "string", "title": "Senha" },
                      "mode": { "type": "string", "title": "Modo", "enum": ["a", "b"] },
                      "lan_name": { "type": "string" },
                      "mtu": { "type": "integer", "title": "MTU" }
                  } } }
            ]
        }));
        let saida = json!({ "rx_bytes": 10, "estado": "up" });
        let hints = lint(&pacote, &[("set_lan", &saida)]);
        let texto = mensagens(&hints);
        for esperado in [
            "set_lan: o título da ação",
            "set_lan: o formulário mostra 6 campos",
            "set_lan.ip: use `widget: \"ip\"`",
            "set_lan.gateway: use `widget: \"ip\"`",
            "set_lan.admin_password: parece uma senha",
            "set_lan.mode: dê nome às opções",
            "set_lan.lan_name: dê ao campo um `title`",
            "set_lan.saída.rx_bytes: chave técnica",
            "set_lan.saída.rx_bytes: use `formats",
        ] {
            assert!(texto.contains(esperado), "faltou «{esperado}» em:\n{texto}");
        }
        assert!(!texto.contains("estado"), "{texto}");
    }
}
