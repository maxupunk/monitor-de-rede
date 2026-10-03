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

use super::{
    effect::Effect,
    item_list::{self, ItemList, ListScope},
    params, version,
};

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
    /// Relatório: campos simples viram ficha, listas de objetos viram tabelas
    /// com título (o estado de uma rede Wi-Fi, por exemplo).
    Report,
}

/// Como a tela escreve o valor de uma chave da saída — com os formatadores do
/// sistema, para um plugin mostrar bytes, latência e datas como o resto da
/// interface mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum OutputFormat {
    /// Quantidade de bytes (`1,2 MB`).
    Bytes,
    /// Taxa em bits por segundo (`120 Mbps`).
    Bps,
    /// Latência em milissegundos (`12,5 ms`).
    Latency,
    /// Percentual (`42,5%`).
    Percent,
    /// Duração em milissegundos (`4,2 s`).
    Duration,
    /// Tempo ligado em segundos (`3 dias e 4 horas`).
    Uptime,
    /// Data e hora (texto ISO ou segundos Unix).
    Datetime,
    /// Há quanto tempo (`há 5 minutos`).
    Relative,
    /// Contagem compacta (`1,2 mil`).
    Count,
    /// Estado (`up`, `down`, `disabled`…), que vira um chip colorido.
    State,
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
    /// Títulos das chaves da saída na tela (`clients` → "Clientes"). A chave
    /// continua o contrato com o script; o título é só apresentação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub labels: Option<BTreeMap<String, String>>,
    /// Como a tela escreve o valor de cada chave (`rx_bytes` → bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub formats: Option<BTreeMap<String, OutputFormat>>,
    /// Ordem das chaves na tela (colunas e campos). O JSON da saída chega com
    /// as chaves em ordem alfabética; sem isto, "Canal sugerido" viria antes de
    /// "Canal atual". Chave fora da lista vai para o fim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
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

/// Formato antigo da tela do plugin instalado — hoje uma [`ItemList`] em
/// tabela (ver [`item_list`]). Continua aceito e é convertido na leitura.
///
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

/// Onde o plugin aparece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum Surface {
    /// Aba própria em `/devices/{id}` de cada equipamento onde está instalado.
    Device,
    /// Página própria ("Aplicativos"), que reúne todos os equipamentos onde
    /// está instalado — a frota.
    Fleet,
}

fn default_surfaces() -> Vec<Surface> {
    vec![Surface::Device]
}

/// Configuração que o plugin guarda: o estado desejado.
///
/// `fleet` vale para todos os equipamentos onde o plugin está instalado (ex.:
/// os SSIDs da rede); `device` é o ajuste de um equipamento (ex.: o canal
/// daquele rádio). O script recebe as duas em `device.settings`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SettingsSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, unknown>")]
    pub fleet: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, unknown>")]
    pub device: Option<Value>,
}

/// Uma ação da frota: a mesma ação de dispositivo em vários equipamentos e,
/// opcionalmente, um cálculo central sobre os resultados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct FleetAction {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    /// Ação de dispositivo executada em cada membro.
    pub action: String,
    /// Função pura do script, `fn <reduce>(results, settings)`, que recebe o
    /// resultado de todos os membros e devolve o consolidado — sem acesso a
    /// equipamento. Pode propor `settings_patch` (ajustes por equipamento) que
    /// o operador aceita com um clique (ex.: o plano de canais).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reduce: Option<String>,
    /// Títulos das chaves do consolidado (ver [`PluginAction::labels`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub labels: Option<BTreeMap<String, String>>,
    /// Formato das chaves do consolidado (ver [`PluginAction::formats`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub formats: Option<BTreeMap<String, OutputFormat>>,
    /// Ordem das chaves do consolidado (ver [`PluginAction::order`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// Ação de leitura que mostra, com os mesmos parâmetros, o que esta vai
    /// mudar em cada equipamento — o "Pré-visualizar" antes de aplicar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub preview: Option<String>,
    /// Onde, na saída da ação de estado, estão os valores atuais dos
    /// parâmetros desta ação (`"_current.radios"`): o diálogo mostra os de cada
    /// equipamento e, com um só escolhido, já preenche o formulário.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub current: Option<String>,
}

/// Uma ação da frota disparada a partir de um item da grade (ex.: editar o
/// SSID da coluna): o formulário vem preenchido com campos do item e os
/// equipamentos que o têm já marcados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MatrixAction {
    /// Ação de frota (`fleet.actions[].id`).
    pub action: String,
    /// Parâmetro da ação → campo do item.
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

/// Formato antigo da lista da frota — hoje `fleet.list` (ver [`item_list`]).
/// Continua aceito e é convertido na leitura.
///
/// A grade membros × itens da página da frota (ex.: roteadores × SSIDs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct FleetMatrix {
    /// Lista no resultado da ação de estado (ex.: `networks`).
    pub field: String,
    /// Campo que identifica a coluna (ex.: `ssid`).
    pub key: String,
    /// Campo com o estado da célula (ex.: `state`).
    pub state: String,
    /// Campo com o detalhe mostrado na célula (ex.: `clients`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
    /// Editar o item da coluna.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub edit: Option<MatrixAction>,
    /// Remover o item da coluna.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub remove: Option<MatrixAction>,
    /// Como a tela chama a lista e um item ("Redes", "rede").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub item_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    /// Campos do item mostrados no cartão, abaixo do nome.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtitle: Vec<String>,
    /// Ação de frota do botão "Nova …".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub add: Option<String>,
}

/// A página da frota.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct FleetSpec {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    /// Ação de dispositivo que dá o estado de cada membro (a "visão geral").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status_action: Option<String>,
    /// Formato antigo de `list`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub matrix: Option<FleetMatrix>,
    /// Os itens que os equipamentos têm (ex.: as redes), um cartão cada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub list: Option<ItemList>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<FleetAction>,
    /// Ação de frota aberta ao clicar num equipamento, só para ele (ex.: os
    /// rádios daquele roteador).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub device_action: Option<String>,
    /// Ações de frota da aba "Avançado", na ordem. Sem lista, vão para lá as
    /// que nenhuma outra parte da tela usa.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
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
    /// Formato antigo de `list`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub panel: Option<PluginPanel>,
    /// Tela própria do plugin instalado: a lista de itens. Sem ela, a aba
    /// mostra as ações.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub list: Option<ItemList>,
    /// Onde aparece: aba do equipamento, página da frota, ou as duas.
    #[serde(default = "default_surfaces")]
    pub surfaces: Vec<Surface>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub settings: Option<SettingsSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fleet: Option<FleetSpec>,
    /// Plugins cujas ações este chama (`device.use_plugin`) — reaproveitar em
    /// vez de copiar. Precisam estar ativos.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uses: Vec<String>,
}

impl FleetSpec {
    /// A lista da frota, venha de `list` ou do formato antigo (`matrix`).
    #[must_use]
    pub fn item_list(&self) -> Option<ItemList> {
        self.list
            .clone()
            .or_else(|| self.matrix.as_ref().map(ItemList::from))
    }

    /// A frota como a tela a recebe: só `list`, já convertida.
    #[must_use]
    pub fn resolved(mut self) -> Self {
        self.list = self.item_list();
        self.matrix = None;
        self
    }
}

impl PluginManifest {
    /// A lista do plugin instalado, venha de `list` ou do formato antigo
    /// (`panel`).
    #[must_use]
    pub fn item_list(&self) -> Option<ItemList> {
        self.list
            .clone()
            .or_else(|| self.panel.as_ref().map(ItemList::from))
    }

    #[must_use]
    pub fn action(&self, id: &str) -> Option<&PluginAction> {
        self.actions.iter().find(|action| action.id == id)
    }

    #[must_use]
    pub fn uses(&self, transport: TransportKind) -> bool {
        self.transports.contains(&transport)
    }

    #[must_use]
    pub fn shows_on(&self, surface: Surface) -> bool {
        self.surfaces.contains(&surface)
    }

    #[must_use]
    pub fn fleet_action(&self, id: &str) -> Option<&FleetAction> {
        self.fleet
            .as_ref()
            .and_then(|fleet| fleet.actions.iter().find(|action| action.id == id))
    }

    /// Funções `reduce` que o script precisa definir.
    #[must_use]
    pub fn reduce_functions(&self) -> Vec<&str> {
        self.fleet
            .iter()
            .flat_map(|fleet| fleet.actions.iter())
            .filter_map(|action| action.reduce.as_deref())
            .collect()
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
        validate_labels(&action.id, action.labels.as_ref(), &mut problems);
        validate_formats(&action.id, action.formats.as_ref(), &mut problems);
        validate_order(&action.id, &action.order, &mut problems);
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
    match (&manifest.list, &manifest.panel) {
        (Some(_), Some(_)) => {
            problems.push("use só `list` — `panel` é o formato antigo da mesma tela".into());
        }
        (Some(list), None) => {
            item_list::validate(list, ListScope::Device(manifest), "list", &mut problems);
        }
        (None, Some(panel)) => item_list::validate(
            &ItemList::from(panel),
            ListScope::Device(manifest),
            "panel",
            &mut problems,
        ),
        (None, None) => {}
    }
    validate_surfaces(manifest, &mut problems);
    problems
}

/// Títulos curtos e em número razoável: é texto de cabeçalho, não conteúdo.
fn validate_labels(
    owner: &str,
    labels: Option<&BTreeMap<String, String>>,
    problems: &mut Vec<String>,
) {
    let Some(labels) = labels else {
        return;
    };
    if labels.len() > 100
        || labels.iter().any(|(key, title)| {
            key.is_empty() || title.trim().is_empty() || title.chars().count() > 60
        })
    {
        problems.push(format!(
            "`labels` de `{owner}`: até 100 títulos, cada um com 1 a 60 caracteres"
        ));
    }
}

/// Formatos de no máximo 100 chaves, nenhuma vazia (o valor o serde já
/// confere: só os do catálogo de [`OutputFormat`]).
fn validate_formats(
    owner: &str,
    formats: Option<&BTreeMap<String, OutputFormat>>,
    problems: &mut Vec<String>,
) {
    if formats.is_some_and(|formats| formats.len() > 100 || formats.keys().any(String::is_empty)) {
        problems.push(format!(
            "`formats` de `{owner}`: até 100 chaves, nenhuma vazia"
        ));
    }
}

/// Até 100 chaves, sem vazia nem repetida.
fn validate_order(owner: &str, order: &[String], problems: &mut Vec<String>) {
    let unique: HashSet<&String> = order.iter().collect();
    if order.len() > 100 || unique.len() != order.len() || order.iter().any(String::is_empty) {
        problems.push(format!(
            "`order` de `{owner}`: até 100 chaves, sem vazia nem repetida"
        ));
    }
}

fn validate_surfaces(manifest: &PluginManifest, problems: &mut Vec<String>) {
    if manifest.surfaces.is_empty() {
        problems.push("declare ao menos uma superfície em `surfaces` (device, fleet)".into());
    }
    if let Some(settings) = &manifest.settings {
        for (scope, schema) in [("fleet", &settings.fleet), ("device", &settings.device)] {
            if let Some(schema) = schema {
                if let Err(error) = params::validate_schema(schema) {
                    problems.push(format!("`settings.{scope}`: {error}"));
                }
            }
        }
        if settings.fleet.is_some() && !manifest.shows_on(Surface::Fleet) {
            problems.push("`settings.fleet` só faz sentido com a superfície `fleet`".into());
        }
    }
    match (&manifest.fleet, manifest.shows_on(Surface::Fleet)) {
        (None, true) => problems.push("a superfície `fleet` precisa do bloco `fleet`".into()),
        (Some(_), false) => problems.push("o bloco `fleet` precisa da superfície `fleet`".into()),
        _ => {}
    }
    if let Some(fleet) = &manifest.fleet {
        if fleet.title.trim().is_empty() {
            problems.push("`fleet.title` é obrigatório".into());
        }
        if let Some(status) = &fleet.status_action {
            match manifest.action(status) {
                None => problems.push(format!(
                    "`fleet.statusAction` cita `{status}`, que não existe"
                )),
                Some(action) if action.effect != Effect::Read => {
                    problems.push("`fleet.statusAction` precisa ser uma ação de leitura".into())
                }
                Some(_) => {}
            }
        }
        match (&fleet.list, &fleet.matrix) {
            (Some(_), Some(_)) => problems.push(
                "use só `fleet.list` — `fleet.matrix` é o formato antigo da mesma lista".into(),
            ),
            (Some(list), None) => item_list::validate(
                list,
                ListScope::Fleet(manifest, fleet),
                "fleet.list",
                problems,
            ),
            (None, Some(matrix)) => item_list::validate(
                &ItemList::from(matrix),
                ListScope::Fleet(manifest, fleet),
                "fleet.matrix",
                problems,
            ),
            (None, None) => {}
        }
        let is_fleet_action = |id: &str| fleet.actions.iter().any(|action| action.id == id);
        let referenced = fleet
            .device_action
            .as_deref()
            .map(|id| ("deviceAction", id))
            .into_iter()
            .chain(fleet.tools.iter().map(|id| ("tools", id.as_str())));
        for (label, id) in referenced {
            if !is_fleet_action(id) {
                problems.push(format!(
                    "`fleet.{label}` cita `{id}`, que não é ação de frota"
                ));
            }
        }
        let mut ids = HashSet::new();
        for action in &fleet.actions {
            if !is_action_id(&action.id) || !ids.insert(action.id.as_str()) {
                problems.push(format!(
                    "ação de frota `{}` com id inválido ou repetido",
                    action.id
                ));
            }
            if manifest.action(&action.action).is_none() {
                problems.push(format!(
                    "a ação de frota `{}` cita `{}`, que não é ação do plugin",
                    action.id, action.action
                ));
            }
            if let Some(reduce) = &action.reduce {
                if !is_action_id(reduce) {
                    problems.push(format!("`reduce` inválido em `{}`", action.id));
                }
            }
            validate_labels(&action.id, action.labels.as_ref(), problems);
            validate_formats(&action.id, action.formats.as_ref(), problems);
            validate_order(&action.id, &action.order, problems);
            if action.current.is_some() && fleet.status_action.is_none() {
                problems.push(format!(
                    "o `current` de `{}` precisa de `fleet.statusAction`",
                    action.id
                ));
            }
            if let Some(preview) = &action.preview {
                if manifest
                    .action(preview)
                    .is_none_or(|found| found.effect != Effect::Read)
                {
                    problems.push(format!(
                        "o `preview` de `{}` precisa ser uma ação de leitura do plugin",
                        action.id
                    ));
                }
            }
        }
    }
    for slug in &manifest.uses {
        if !is_slug(slug) || slug == &manifest.slug {
            problems.push(format!("`uses` cita `{slug}`, que não é um plugin válido"));
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
    fn titulos_da_saida_sao_curtos() {
        let mut ok = manifest();
        ok.actions[0].labels = Some(BTreeMap::from([("firmware".into(), "Versão".into())]));
        assert!(validate(&ok).is_empty());
        let mut longo = manifest();
        longo.actions[0].labels = Some(BTreeMap::from([("firmware".into(), "x".repeat(61))]));
        assert!(validate(&longo)
            .iter()
            .any(|p| p.contains("`labels` de `detect`")));
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
            problems.iter().any(|p| p.contains("não vem do item")),
            "{problems:?}"
        );
        assert!(
            problems.iter().all(|p| !p.contains("`list")),
            "o formato antigo é apontado pelo próprio nome: {problems:?}"
        );
    }

    #[test]
    fn lista_e_painel_juntos_sao_recusados() {
        let mut duplo = com_painel();
        duplo.list = duplo.item_list();
        assert!(validate(&duplo).iter().any(|p| p.contains("use só `list`")));
        duplo.panel = None;
        assert!(validate(&duplo).is_empty(), "{:?}", validate(&duplo));
    }

    #[test]
    fn superficies_e_frota_coerentes() {
        let mut frota = manifest();
        frota.surfaces = vec![Surface::Device, Surface::Fleet];
        assert!(validate(&frota)
            .iter()
            .any(|p| p.contains("precisa do bloco `fleet`")));
        frota.fleet = Some(FleetSpec {
            title: "Rede".into(),
            icon: None,
            description: None,
            status_action: Some("install_package".into()),
            matrix: None,
            list: None,
            device_action: Some("fantasma".into()),
            tools: Vec::new(),
            actions: vec![FleetAction {
                id: "aplicar".into(),
                title: "Aplicar".into(),
                description: None,
                icon: None,
                action: "inexistente".into(),
                reduce: None,
                labels: None,
                formats: None,
                order: Vec::new(),
                preview: None,
                current: None,
            }],
        });
        let problems = validate(&frota);
        assert!(
            problems.iter().any(|p| p.contains("ação de leitura")),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.contains("`fleet.deviceAction` cita `fantasma`")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("inexistente")),
            "{problems:?}"
        );

        let mut so_device = manifest();
        so_device.settings = Some(SettingsSpec {
            fleet: Some(serde_json::json!({ "type": "object" })),
            device: None,
        });
        assert!(validate(&so_device)
            .iter()
            .any(|p| p.contains("superfície `fleet`")));
    }

    #[test]
    fn manifesto_antigo_continua_so_no_dispositivo() {
        assert_eq!(manifest().surfaces, vec![Surface::Device]);
    }

    #[test]
    fn acao_repetida_e_recusada() {
        let mut repetida = manifest();
        let copia = repetida.actions[1].clone();
        repetida.actions.push(copia);
        assert!(validate(&repetida).iter().any(|p| p.contains("repetida")));
    }
}
