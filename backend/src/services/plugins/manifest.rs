//! O manifesto: o que o plugin declara ser, alcançar e fazer.
//!
//! O manifesto é a promessa que o operador lê antes de executar. Por isso a
//! validação aqui é estrita — um campo inválido é recusado, nunca ignorado:
//! uma ação que se diz "leitura" precisa sê-lo, um transporte não declarado
//! não é oferecido ao script, e `detect` existe sempre, porque é por ela que a
//! compatibilidade é provada.

use std::collections::{BTreeMap, HashSet};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{effect::Effect, params, version};

/// A ação obrigatória de todo plugin: identifica o equipamento (modelo,
/// firmware) sem alterar nada.
pub const DETECT_ACTION: &str = "detect";

/// Por onde o plugin fala com o equipamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum TransportKind {
    Ssh,
    Http,
    Telnet,
}

impl TransportKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Http => "http",
            Self::Telnet => "telnet",
        }
    }

    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Ssh => 22,
            Self::Http => 80,
            Self::Telnet => 23,
        }
    }

    /// # Errors
    ///
    /// Tipo fora de `ssh`, `http` e `telnet`.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ssh" => Ok(Self::Ssh),
            "http" => Ok(Self::Http),
            "telnet" => Ok(Self::Telnet),
            other => Err(format!("tipo de acesso desconhecido: {other}")),
        }
    }
}

/// Como a tela apresenta o resultado de uma ação.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum OutputKind {
    /// Texto corrido (saída de comando).
    #[default]
    Text,
    /// Lista de objetos — uma linha por objeto, colunas pelas chaves.
    Table,
    /// Objeto de chave e valor.
    Kv,
    /// Qualquer JSON, mostrado formatado.
    Json,
}

/// Uma operação que o plugin oferece.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginAction {
    /// Nome da função no script (`fn <id>(device, params)`).
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    pub effect: Effect,
    /// Esquema JSON (subconjunto — ver [`params`]) dos parâmetros.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, unknown>")]
    pub params: Option<Value>,
    #[serde(default)]
    pub output: OutputKind,
    /// Ação de escrita idempotente, que pode entrar no teste funcional.
    #[serde(default)]
    pub safe_to_retest: bool,
}

/// Com que equipamentos o plugin diz ser compatível. Todos os campos são
/// opcionais e valem juntos (E).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MatchRule {
    /// Ids do catálogo de sistemas (`openwrt`, `routeros`, `linux`, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub vendor_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model_regex: Option<String>,
    /// Restrição de firmware (`>=21.02, <24`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub firmware: Option<String>,
    /// Regex sobre o título/`Server` da página web — pista para o
    /// reconhecimento de equipamento que só tem interface HTTP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub http_fingerprint: Option<String>,
}

/// Uma ação oferecida sobre a linha selecionada do painel ("Instalar pacote").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PanelRowAction {
    /// Id da ação do manifesto.
    pub action: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    /// Parâmetro da ação → coluna da linha que o preenche.
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    /// Só aparece quando a coluna é verdadeira.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub show_when: Option<String>,
    /// Some quando a coluna é verdadeira.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hide_when: Option<String>,
}

/// Uma coluna da lista do painel, na ordem e com o rótulo da tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PanelColumn {
    pub key: String,
    pub label: String,
}

/// A tela própria do plugin instalado: uma lista vinda de uma ação de leitura
/// (com busca opcional), ações sobre a linha escolhida e botões de barra.
///
/// É declarativo de propósito: o plugin descreve a tela e a interface a
/// desenha — nenhum código do plugin roda no navegador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginPanel {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    /// Ação de leitura com `output: table` que preenche a lista.
    pub list_action: String,
    /// Parâmetro da ação de lista que recebe o texto da busca.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub search_param: Option<String>,
    /// Coluna que identifica a linha (e aparece em destaque).
    pub key_column: String,
    /// Colunas mostradas, na ordem. Vazio: todas as chaves da linha.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<PanelColumn>,
    /// Ações sem parâmetro da linha, como botões da barra ("Atualizar lista").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub toolbar: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_actions: Vec<PanelRowAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct PluginManifest {
    pub slug: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub author: Option<String>,
    pub transports: Vec<TransportKind>,
    #[serde(default, rename = "match")]
    pub matcher: MatchRule,
    pub actions: Vec<PluginAction>,
    /// Tela própria do plugin instalado. Sem ela, a aba mostra as ações.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub panel: Option<PluginPanel>,
}

impl PluginManifest {
    #[must_use]
    pub fn action(&self, id: &str) -> Option<&PluginAction> {
        self.actions.iter().find(|action| action.id == id)
    }

    #[must_use]
    pub fn uses(&self, transport: TransportKind) -> bool {
        self.transports.contains(&transport)
    }
}

fn is_slug(value: &str) -> bool {
    let bytes = value.as_bytes();
    (2..=64).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn is_action_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0] == b'_')
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
}

fn is_semver(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 6 && part.bytes().all(|b| b.is_ascii_digit())
        })
}

/// Problemas estruturais do manifesto. Vazio quando está tudo certo.
#[must_use]
pub fn validate(manifest: &PluginManifest) -> Vec<String> {
    let mut problems = Vec::new();
    if !is_slug(&manifest.slug) {
        problems.push(
            "o `slug` precisa ter de 2 a 64 caracteres: letras minúsculas, números e hífen".into(),
        );
    }
    if manifest.name.trim().is_empty() || manifest.name.chars().count() > 160 {
        problems.push("o `name` é obrigatório e tem no máximo 160 caracteres".into());
    }
    if !is_semver(&manifest.version) {
        problems.push("a `version` precisa estar no formato 1.2.3".into());
    }
    if manifest.transports.is_empty() {
        problems
            .push("declare ao menos um transporte em `transports` (ssh, http ou telnet)".into());
    }
    let unique_transports: HashSet<_> = manifest.transports.iter().collect();
    if unique_transports.len() != manifest.transports.len() {
        problems.push("há transporte repetido em `transports`".into());
    }
    validate_matcher(&manifest.matcher, &mut problems);

    if manifest.actions.is_empty() {
        problems.push("o plugin precisa de ao menos uma ação".into());
    }
    let mut ids = HashSet::new();
    for action in &manifest.actions {
        if !is_action_id(&action.id) {
            problems.push(format!(
                "a ação `{}` precisa de um id em minúsculas, números e sublinhado",
                action.id
            ));
        }
        if !ids.insert(action.id.as_str()) {
            problems.push(format!("a ação `{}` está repetida", action.id));
        }
        if action.title.trim().is_empty() {
            problems.push(format!("a ação `{}` precisa de `title`", action.id));
        }
        if let Some(schema) = &action.params {
            if let Err(error) = params::validate_schema(schema) {
                problems.push(format!("ação `{}`: {error}", action.id));
            }
        }
        if action.safe_to_retest && action.effect == Effect::Read {
            problems.push(format!(
                "a ação `{}` é de leitura; `safeToRetest` só faz sentido em escrita",
                action.id
            ));
        }
    }
    match manifest.action(DETECT_ACTION) {
        None => {
            problems.push("a ação `detect` é obrigatória: é ela que prova a compatibilidade".into())
        }
        Some(detect) if detect.effect != Effect::Read => {
            problems.push("a ação `detect` precisa ser de leitura (`effect: read`)".into());
        }
        Some(detect) if detect.params.is_some() => {
            problems.push("a ação `detect` não recebe parâmetros".into());
        }
        Some(_) => {}
    }
    if let Some(panel) = &manifest.panel {
        validate_panel(manifest, panel, &mut problems);
    }
    problems
}

/// Nomes das propriedades do esquema de parâmetros de uma ação, e as
/// obrigatórias.
fn param_names(action: &PluginAction) -> (Vec<String>, Vec<String>) {
    let schema = action.params.as_ref();
    let names = schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default();
    let required = schema
        .and_then(|schema| schema.get("required"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    (names, required)
}

fn validate_panel(manifest: &PluginManifest, panel: &PluginPanel, problems: &mut Vec<String>) {
    if panel.title.trim().is_empty() {
        problems.push("`panel.title` é obrigatório".into());
    }
    if panel.key_column.trim().is_empty() {
        problems.push("`panel.keyColumn` é obrigatório".into());
    }
    match manifest.action(&panel.list_action) {
        None => problems.push(format!(
            "`panel.listAction` cita `{}`, que não é uma ação do plugin",
            panel.list_action
        )),
        Some(list) => {
            if list.effect != Effect::Read || list.output != OutputKind::Table {
                problems.push(
                    "`panel.listAction` precisa ser uma ação de leitura com `output: table`".into(),
                );
            }
            let (names, required) = param_names(list);
            if let Some(search) = &panel.search_param {
                if !names.contains(search) {
                    problems.push(format!(
                        "`panel.searchParam` cita `{search}`, que não é parâmetro de `{}`",
                        list.id
                    ));
                }
            }
            if required
                .iter()
                .any(|name| Some(name) != panel.search_param.as_ref())
            {
                problems
                    .push("a ação de lista do painel só pode exigir o parâmetro de busca".into());
            }
        }
    }
    for column in &panel.columns {
        if column.key.trim().is_empty() || column.label.trim().is_empty() {
            problems.push("`panel.columns`: cada coluna precisa de `key` e `label`".into());
        }
    }
    for id in &panel.toolbar {
        match manifest.action(id) {
            None => problems.push(format!("`panel.toolbar` cita `{id}`, que não existe")),
            Some(action) if !param_names(action).1.is_empty() => problems.push(format!(
                "`panel.toolbar`: a ação `{id}` exige parâmetros e não pode ser um botão da barra"
            )),
            Some(_) => {}
        }
    }
    for row in &panel.row_actions {
        let Some(action) = manifest.action(&row.action) else {
            problems.push(format!(
                "`panel.rowActions` cita `{}`, que não existe",
                row.action
            ));
            continue;
        };
        let (names, required) = param_names(action);
        for param in row.params.keys() {
            if !names.contains(param) {
                problems.push(format!(
                    "`panel.rowActions`: `{param}` não é parâmetro de `{}`",
                    row.action
                ));
            }
        }
        for name in required {
            if !row.params.contains_key(&name) {
                problems.push(format!(
                    "`panel.rowActions`: o parâmetro obrigatório `{name}` de `{}` não vem da linha",
                    row.action
                ));
            }
        }
        if row.label.trim().is_empty() {
            problems.push(format!(
                "`panel.rowActions`: `{}` precisa de `label`",
                row.action
            ));
        }
    }
}

fn validate_matcher(matcher: &MatchRule, problems: &mut Vec<String>) {
    for (field, pattern) in [
        ("vendorRegex", &matcher.vendor_regex),
        ("modelRegex", &matcher.model_regex),
        ("httpFingerprint", &matcher.http_fingerprint),
    ] {
        if let Some(pattern) = pattern {
            if let Err(error) = Regex::new(pattern) {
                problems.push(format!("`match.{field}` é uma regex inválida: {error}"));
            }
        }
    }
    if let Some(constraint) = &matcher.firmware {
        if let Err(error) = version::satisfies("0", constraint) {
            problems.push(format!("`match.firmware`: {error}"));
        }
    }
    for platform in &matcher.platforms {
        if crate::services::devices::systems::find(platform).is_none() {
            problems.push(format!(
                "`match.platforms` cita `{platform}`, que não está no catálogo de sistemas"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn manifest() -> PluginManifest {
        serde_json::from_value(json!({
            "slug": "openwrt-opkg",
            "name": "OpenWrt – pacotes",
            "version": "1.0.0",
            "transports": ["ssh"],
            "match": { "platforms": ["openwrt"], "firmware": ">=21.02" },
            "actions": [
                { "id": "detect", "title": "Detectar", "effect": "read", "output": "kv" },
                { "id": "install_package", "title": "Instalar", "effect": "write",
                  "params": { "type": "object",
                              "properties": { "name": { "type": "string", "pattern": "[a-z0-9._+-]+" } },
                              "required": ["name"] } }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn manifesto_valido_nao_tem_problemas() {
        assert!(
            validate(&manifest()).is_empty(),
            "{:?}",
            validate(&manifest())
        );
    }

    #[test]
    fn detect_e_obrigatorio_e_de_leitura() {
        let mut sem_detect = manifest();
        sem_detect
            .actions
            .retain(|action| action.id != DETECT_ACTION);
        assert!(validate(&sem_detect).iter().any(|p| p.contains("detect")));

        let mut detect_escrita = manifest();
        detect_escrita.actions[0].effect = Effect::Write;
        assert!(validate(&detect_escrita)
            .iter()
            .any(|p| p.contains("leitura")));
    }

    #[test]
    fn campos_invalidos_sao_apontados() {
        let mut ruim = manifest();
        ruim.slug = "Com Espaço".into();
        ruim.version = "1.0".into();
        ruim.transports.clear();
        ruim.matcher.platforms = vec!["inexistente".into()];
        ruim.matcher.model_regex = Some("(".into());
        let problems = validate(&ruim);
        assert!(problems.len() >= 5, "{problems:?}");
    }

    fn com_painel() -> PluginManifest {
        let mut manifesto: PluginManifest = serde_json::from_value(json!({
            "slug": "pacotes", "name": "Pacotes", "version": "1.0.0", "transports": ["ssh"],
            "actions": [
                { "id": "detect", "title": "Detectar", "effect": "read" },
                { "id": "list_packages", "title": "Listar", "effect": "read", "output": "table",
                  "params": { "type": "object", "properties": { "query": { "type": "string" } } } },
                { "id": "update_index", "title": "Atualizar", "effect": "write" },
                { "id": "install_package", "title": "Instalar", "effect": "write",
                  "params": { "type": "object",
                              "properties": { "name": { "type": "string", "pattern": "[a-z]+" } },
                              "required": ["name"] } }
            ],
            "panel": {
                "title": "Gerenciador de pacotes", "listAction": "list_packages",
                "searchParam": "query", "keyColumn": "name", "toolbar": ["update_index"],
                "rowActions": [ { "action": "install_package", "label": "Instalar pacote",
                                  "params": { "name": "name" }, "hideWhen": "installed" } ]
            }
        }))
        .unwrap();
        manifesto.matcher = MatchRule::default();
        manifesto
    }

    #[test]
    fn painel_coerente_passa() {
        assert!(
            validate(&com_painel()).is_empty(),
            "{:?}",
            validate(&com_painel())
        );
    }

    #[test]
    fn painel_incoerente_e_apontado() {
        let mut ruim = com_painel();
        let panel = ruim.panel.as_mut().unwrap();
        panel.list_action = "update_index".into();
        panel.toolbar = vec!["install_package".into()];
        panel.row_actions[0].params.clear();
        let problems = validate(&ruim);
        assert!(
            problems.iter().any(|p| p.contains("output: table")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("botão da barra")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("não vem da linha")),
            "{problems:?}"
        );
    }

    #[test]
    fn acao_repetida_e_recusada() {
        let mut repetida = manifest();
        let copia = repetida.actions[1].clone();
        repetida.actions.push(copia);
        assert!(validate(&repetida).iter().any(|p| p.contains("repetida")));
    }
}
