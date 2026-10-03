//! A lista de itens: a tela de todo plugin que gerencia "coisas" — pacotes de
//! um roteador, redes Wi-Fi da frota, regras de firewall, usuários.
//!
//! Antes eram duas declarações com a mesma ideia e regras diferentes: o
//! `panel` (a aba do equipamento) e a `fleet.matrix` (a página da frota). Agora
//! as duas são uma [`ItemList`] — a IA aprende um vocabulário só, a interface
//! tem um componente só e a validação mora aqui. Os dois formatos antigos
//! continuam aceitos e são convertidos na leitura ([`From`]); o JSON gravado
//! não muda, então o checksum dos plugins instalados também não.
//!
//! O contexto decide de onde vêm as linhas e a que ações os botões se referem:
//!
//! * **equipamento**: `source` é uma ação de leitura do plugin; os botões
//!   citam ações do plugin;
//! * **frota**: as linhas vêm da `statusAction` de cada membro (`field` aponta
//!   a lista na saída) e os botões citam ações de frota.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{
    effect::Effect,
    manifest::{
        FleetMatrix, FleetSpec, MatrixAction, OutputKind, PanelRowAction, PluginAction,
        PluginManifest, PluginPanel,
    },
};

/// Uma coluna da lista em tabela, na ordem e com o rótulo da tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ItemColumn {
    pub key: String,
    pub label: String,
}

/// Um botão sobre um item ("Editar", "Instalar pacote").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ItemAction {
    /// Ação do plugin (equipamento) ou de frota.
    pub action: String,
    /// Texto do botão. Sem ele, o título da ação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    /// Parâmetro da ação → campo do item que o preenche.
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    /// Só aparece quando o campo do item é verdadeiro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub show_when: Option<String>,
    /// Some quando o campo do item é verdadeiro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hide_when: Option<String>,
}

/// Como os itens aparecem.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum ItemLayout {
    /// Um cartão por item — poucos itens com estado (redes, túneis).
    #[default]
    Cards,
    /// Tabela com busca e seleção — muitos itens (pacotes, regras).
    Table,
}

/// A lista de itens de um plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ItemList {
    /// Como a tela chama a lista ("Redes", "Pacotes").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    /// Como chama um item ("rede") — é o "Nova rede" do botão.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub item_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    /// Ação de leitura que dá a lista (equipamento). Na frota fica vazio: é a
    /// `statusAction` de cada membro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<String>,
    /// Onde está a lista na saída (`networks`). Vazio: a saída é a lista.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub field: Option<String>,
    /// Campo que identifica o item (e aparece em destaque).
    pub key: String,
    #[serde(default)]
    pub layout: ItemLayout,
    /// Colunas da tabela, na ordem. Vazio: todas as chaves do item.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ItemColumn>,
    /// Campos mostrados no cartão, abaixo do nome.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtitle: Vec<String>,
    /// Campo com o estado do item (`up`, `disabled`…), que vira a cor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub state: Option<String>,
    /// Campo numérico somado no cartão (ex.: `clients`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
    /// Parâmetro da ação de lista que recebe o texto da busca.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub search_param: Option<String>,
    /// Ações sem parâmetro obrigatório, como botões da barra.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub toolbar: Vec<String>,
    /// Ação do botão "Nova …".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub add: Option<String>,
    /// Aberta ao clicar no item, com o formulário preenchido por ele.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub edit: Option<ItemAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub remove: Option<ItemAction>,
    /// Outras ações sobre o item.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_actions: Vec<ItemAction>,
}

impl From<&PanelRowAction> for ItemAction {
    fn from(row: &PanelRowAction) -> Self {
        Self {
            action: row.action.clone(),
            label: Some(row.label.clone()),
            icon: row.icon.clone(),
            params: row.params.clone(),
            show_when: row.show_when.clone(),
            hide_when: row.hide_when.clone(),
        }
    }
}

impl From<&MatrixAction> for ItemAction {
    fn from(target: &MatrixAction) -> Self {
        Self {
            action: target.action.clone(),
            label: None,
            icon: None,
            params: target.params.clone(),
            show_when: None,
            hide_when: None,
        }
    }
}

/// O `panel` antigo: tabela vinda de uma ação, com busca e botões.
impl From<&PluginPanel> for ItemList {
    fn from(panel: &PluginPanel) -> Self {
        Self {
            title: Some(panel.title.clone()),
            item_name: None,
            icon: panel.icon.clone(),
            source: Some(panel.list_action.clone()),
            field: None,
            key: panel.key_column.clone(),
            layout: ItemLayout::Table,
            columns: panel
                .columns
                .iter()
                .map(|column| ItemColumn {
                    key: column.key.clone(),
                    label: column.label.clone(),
                })
                .collect(),
            subtitle: Vec::new(),
            state: None,
            detail: None,
            search_param: panel.search_param.clone(),
            toolbar: panel.toolbar.clone(),
            add: None,
            edit: None,
            remove: None,
            row_actions: panel.row_actions.iter().map(ItemAction::from).collect(),
        }
    }
}

/// A `fleet.matrix` antiga: cartões vindos da ação de estado da frota.
impl From<&FleetMatrix> for ItemList {
    fn from(matrix: &FleetMatrix) -> Self {
        Self {
            title: matrix.title.clone(),
            item_name: matrix.item_name.clone(),
            icon: matrix.icon.clone(),
            source: None,
            field: Some(matrix.field.clone()),
            key: matrix.key.clone(),
            layout: ItemLayout::Cards,
            columns: Vec::new(),
            subtitle: matrix.subtitle.clone(),
            state: Some(matrix.state.clone()),
            detail: matrix.detail.clone(),
            search_param: None,
            toolbar: Vec::new(),
            add: matrix.add.clone(),
            edit: matrix.edit.as_ref().map(ItemAction::from),
            remove: matrix.remove.as_ref().map(ItemAction::from),
            row_actions: Vec::new(),
        }
    }
}

impl ItemList {
    /// Todos os botões sobre um item, com o nome do campo no manifesto.
    fn item_actions(&self) -> impl Iterator<Item = (&'static str, &ItemAction)> {
        self.edit
            .iter()
            .map(|action| ("edit", action))
            .chain(self.remove.iter().map(|action| ("remove", action)))
            .chain(self.row_actions.iter().map(|action| ("rowActions", action)))
    }
}

/// Onde a lista está — e, por isso, a que ações os botões se referem.
#[derive(Clone, Copy)]
pub enum ListScope<'a> {
    Device(&'a PluginManifest),
    Fleet(&'a PluginManifest, &'a FleetSpec),
}

impl<'a> ListScope<'a> {
    /// A ação de dispositivo que um botão executa (na frota, a que a ação de
    /// frota roda em cada membro).
    fn device_action(self, id: &str) -> Option<&'a PluginAction> {
        match self {
            Self::Device(manifest) => manifest.action(id),
            Self::Fleet(manifest, fleet) => fleet
                .actions
                .iter()
                .find(|action| action.id == id)
                .and_then(|action| manifest.action(&action.action)),
        }
    }

    fn kind(self) -> &'static str {
        match self {
            Self::Device(_) => "ação do plugin",
            Self::Fleet(..) => "ação de frota",
        }
    }
}

/// Nomes das propriedades do esquema de parâmetros de uma ação, e as
/// obrigatórias.
#[must_use]
pub fn param_names(action: &PluginAction) -> (Vec<String>, Vec<String>) {
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

/// Problemas de uma lista de itens. `at` é o nome dela no manifesto (`list`,
/// `fleet.list`, ou o formato antigo de onde veio).
pub fn validate(list: &ItemList, scope: ListScope, at: &str, problems: &mut Vec<String>) {
    if list.key.trim().is_empty() {
        problems.push(format!("`{at}.key` é obrigatório"));
    }
    for (label, text) in [("title", &list.title), ("itemName", &list.item_name)] {
        if text
            .as_deref()
            .is_some_and(|text| text.trim().is_empty() || text.chars().count() > 60)
        {
            problems.push(format!("`{at}.{label}` tem de 1 a 60 caracteres"));
        }
    }
    for column in &list.columns {
        if column.key.trim().is_empty() || column.label.trim().is_empty() {
            problems.push(format!(
                "`{at}.columns`: cada coluna precisa de `key` e `label`"
            ));
        }
    }
    validate_source(list, scope, at, problems);

    if let Some(id) = &list.add {
        if scope.device_action(id).is_none() {
            problems.push(format!(
                "`{at}.add` cita `{id}`, que não é {}",
                scope.kind()
            ));
        }
    }
    let unique: HashSet<&String> = list.toolbar.iter().collect();
    if unique.len() != list.toolbar.len() {
        problems.push(format!("`{at}.toolbar` tem ação repetida"));
    }
    for id in &list.toolbar {
        match scope.device_action(id) {
            None => problems.push(format!(
                "`{at}.toolbar` cita `{id}`, que não é {}",
                scope.kind()
            )),
            Some(action) if !param_names(action).1.is_empty() => problems.push(format!(
                "`{at}.toolbar`: a ação `{id}` exige parâmetros e não pode ser um botão da barra"
            )),
            Some(_) => {}
        }
    }
    for (label, target) in list.item_actions() {
        let Some(action) = scope.device_action(&target.action) else {
            problems.push(format!(
                "`{at}.{label}` cita `{}`, que não é {}",
                target.action,
                scope.kind()
            ));
            continue;
        };
        let (names, required) = param_names(action);
        for param in target.params.keys() {
            if !names.contains(param) {
                problems.push(format!(
                    "`{at}.{label}`: `{param}` não é parâmetro de `{}`",
                    target.action
                ));
            }
        }
        // No equipamento o botão roda direto com o que vem do item; na frota
        // (e no editar) o formulário abre e o operador completa.
        let runs_directly = matches!(scope, ListScope::Device(_)) && label != "edit";
        if runs_directly {
            for name in required {
                if !target.params.contains_key(&name) {
                    problems.push(format!(
                        "`{at}.{label}`: o parâmetro obrigatório `{name}` de `{}` não vem do item",
                        target.action
                    ));
                }
            }
        }
        if target
            .label
            .as_deref()
            .is_some_and(|text| text.trim().is_empty())
        {
            problems.push(format!(
                "`{at}.{label}`: o `label` de `{}` está vazio",
                target.action
            ));
        }
    }
}

fn validate_source(list: &ItemList, scope: ListScope, at: &str, problems: &mut Vec<String>) {
    match scope {
        ListScope::Device(manifest) => {
            let Some(source) = &list.source else {
                problems.push(format!(
                    "`{at}.source` é obrigatório: a ação de leitura que dá a lista"
                ));
                return;
            };
            let Some(action) = manifest.action(source) else {
                problems.push(format!(
                    "`{at}.source` cita `{source}`, que não é uma ação do plugin"
                ));
                return;
            };
            let shape_ok = match &list.field {
                None => action.output == OutputKind::Table,
                Some(_) => matches!(action.output, OutputKind::Report | OutputKind::Json),
            };
            if action.effect != Effect::Read || !shape_ok {
                problems.push(format!(
                    "`{at}.source` precisa ser uma ação de leitura com `output: table` (ou `report`, com `field`)"
                ));
            }
            let (names, required) = param_names(action);
            if let Some(search) = &list.search_param {
                if !names.contains(search) {
                    problems.push(format!(
                        "`{at}.searchParam` cita `{search}`, que não é parâmetro de `{source}`"
                    ));
                }
            }
            if required
                .iter()
                .any(|name| Some(name) != list.search_param.as_ref())
            {
                problems.push(format!(
                    "`{at}`: a ação de lista só pode exigir o parâmetro de busca"
                ));
            }
        }
        ListScope::Fleet(_, fleet) => {
            if list.source.is_some() || list.search_param.is_some() {
                problems.push(format!(
                    "`{at}`: na frota a lista vem da `statusAction` — sem `source` nem `searchParam`"
                ));
            }
            if fleet.status_action.is_none() {
                problems.push(format!("`{at}` precisa de `fleet.statusAction`"));
            }
            if list.field.as_deref().is_none_or(str::is_empty) {
                problems.push(format!(
                    "`{at}.field` é obrigatório: a lista na saída da `statusAction`"
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifesto(extra: Value) -> PluginManifest {
        let mut base = json!({
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
            ]
        });
        for (key, value) in extra.as_object().unwrap() {
            base[key] = value.clone();
        }
        serde_json::from_value(base).unwrap()
    }

    fn problemas(manifest: &PluginManifest, list: &ItemList) -> Vec<String> {
        let mut problems = Vec::new();
        validate(list, ListScope::Device(manifest), "list", &mut problems);
        problems
    }

    #[test]
    fn o_painel_antigo_vira_lista_em_tabela_sem_perder_nada() {
        let antigo = manifesto(json!({ "panel": {
            "title": "Gerenciador de pacotes", "listAction": "list_packages",
            "searchParam": "query", "keyColumn": "name", "toolbar": ["update_index"],
            "columns": [{ "key": "name", "label": "Pacote" }],
            "rowActions": [{ "action": "install_package", "label": "Instalar pacote",
                             "params": { "name": "name" }, "hideWhen": "installed" }]
        }}));
        let lista = ItemList::from(antigo.panel.as_ref().unwrap());
        assert_eq!(lista.layout, ItemLayout::Table);
        assert_eq!(lista.source.as_deref(), Some("list_packages"));
        assert_eq!(lista.key, "name");
        assert_eq!(lista.row_actions[0].hide_when.as_deref(), Some("installed"));
        assert_eq!(
            lista.row_actions[0].label.as_deref(),
            Some("Instalar pacote")
        );
        assert!(
            problemas(&antigo, &lista).is_empty(),
            "{:?}",
            problemas(&antigo, &lista)
        );
    }

    #[test]
    fn lista_incoerente_aponta_cada_problema() {
        let manifest = manifesto(json!({}));
        let lista: ItemList = serde_json::from_value(json!({
            "source": "update_index", "key": "name", "toolbar": ["install_package"],
            "rowActions": [{ "action": "install_package" }],
            "edit": { "action": "fantasma" }
        }))
        .unwrap();
        let problems = problemas(&manifest, &lista);
        for esperado in [
            "output: table",
            "botão da barra",
            "não vem do item",
            "`fantasma`",
        ] {
            assert!(
                problems.iter().any(|p| p.contains(esperado)),
                "faltou {esperado}: {problems:?}"
            );
        }
    }

    #[test]
    fn na_frota_a_lista_vem_da_acao_de_estado_e_os_botoes_sao_acoes_de_frota() {
        let manifest = manifesto(json!({}));
        let fleet: FleetSpec = serde_json::from_value(json!({
            "title": "Pacotes da rede", "statusAction": "list_packages",
            "actions": [{ "id": "instalar", "title": "Instalar", "action": "install_package" }]
        }))
        .unwrap();
        let lista: ItemList = serde_json::from_value(json!({
            "field": "packages", "key": "name", "add": "instalar",
            "edit": { "action": "instalar", "params": { "name": "name" } },
            "remove": { "action": "install_package" }
        }))
        .unwrap();
        let mut problems = Vec::new();
        validate(
            &lista,
            ListScope::Fleet(&manifest, &fleet),
            "fleet.list",
            &mut problems,
        );
        assert_eq!(
            problems,
            vec!["`fleet.list.remove` cita `install_package`, que não é ação de frota".to_string()]
        );
    }
}
