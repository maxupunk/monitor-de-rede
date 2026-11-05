//! O layout do dashboard e as operações sobre ele — puras, sem banco.
//!
//! O layout gravado é a lista que a tela salva (`stores/dashboard.ts`):
//! objetos em camelCase com `id`, `type`, colunas, `visible`, `order` e
//! `config`. Campo que este módulo não conhece passa intacto (`extra`), para
//! a tela poder evoluir sem a IA apagar o que ela gravou.
//!
//! Posições são contadas entre os cards **visíveis**, a partir de 0 — é o que
//! a pessoa vê. O `order` gravado é renumerado a cada operação.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use super::catalog::{DashboardWidgetSize, DashboardWidgetType, DEFAULT_LAYOUT};
use crate::services::shared::errors::{AppError, AppResult};

/// Períodos que os gráficos do dashboard entendem.
pub const TIMEFRAMES: [&str; 4] = ["5m", "15m", "1h", "24h"];

/// Chaves de `config` que uma operação pode mudar.
pub const CONFIG_KEYS: [&str; 8] = [
    "timeframe",
    "monitorId",
    "deviceId",
    "interfaceId",
    "interfaceName",
    "dnsServerIds",
    "warningThreshold",
    "criticalThreshold",
];

/// Um card gravado no layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardWidget {
    pub id: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub widget_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(
        default,
        deserialize_with = "lenient_int",
        skip_serializing_if = "Option::is_none"
    )]
    pub cols: Option<i64>,
    #[serde(
        default,
        deserialize_with = "lenient_int",
        skip_serializing_if = "Option::is_none"
    )]
    pub sm: Option<i64>,
    #[serde(
        default,
        deserialize_with = "lenient_int",
        skip_serializing_if = "Option::is_none"
    )]
    pub md: Option<i64>,
    #[serde(
        default,
        deserialize_with = "lenient_int",
        skip_serializing_if = "Option::is_none"
    )]
    pub lg: Option<i64>,
    #[serde(default = "shown", deserialize_with = "lenient_bool")]
    pub visible: bool,
    #[serde(default, deserialize_with = "lenient_int")]
    pub order: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, deserialize_with = "lenient_object")]
    pub config: Map<String, Value>,
    /// O que a tela gravou e este módulo não conhece.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

const fn shown() -> bool {
    true
}

fn lenient_int<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|value| value.round() as i64)),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    })
}

fn lenient_bool<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(!matches!(
        Value::deserialize(deserializer)?,
        Value::Bool(false)
    ))
}

fn lenient_object<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Map<String, Value>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Object(map) => map,
        _ => Map::new(),
    })
}

impl DashboardWidget {
    /// Card novo de um tipo do catálogo, com o tamanho e o título padrão.
    #[must_use]
    pub fn new(id: String, kind: DashboardWidgetType, size: DashboardWidgetSize) -> Self {
        let spec = kind.spec();
        let mut widget = Self {
            id,
            widget_type: Some(kind.id().to_string()),
            title: Some(spec.title.to_string()),
            category: Some(spec.category.id().to_string()),
            cols: None,
            sm: None,
            md: None,
            lg: None,
            visible: true,
            order: None,
            description: Some(spec.description.to_string()),
            icon: Some(spec.icon.to_string()),
            config: Map::new(),
            extra: Map::new(),
        };
        widget.set_size(size);
        widget
    }

    /// O tipo gravado; sem ele, a tela usa o `id`.
    #[must_use]
    pub fn type_id(&self) -> &str {
        self.widget_type.as_deref().unwrap_or(&self.id)
    }

    #[must_use]
    pub fn kind(&self) -> Option<DashboardWidgetType> {
        DashboardWidgetType::from_id(self.type_id())
    }

    #[must_use]
    pub fn size(&self) -> DashboardWidgetSize {
        DashboardWidgetSize::from_columns(self.lg, self.md)
    }

    fn set_size(&mut self, size: DashboardWidgetSize) {
        let columns = size.columns();
        self.cols = Some(columns.cols);
        self.sm = Some(columns.sm);
        self.md = Some(columns.md);
        self.lg = Some(columns.lg);
    }

    /// O nome que a pessoa vê no card.
    #[must_use]
    pub fn label(&self) -> String {
        self.title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .or_else(|| self.kind().map(|kind| kind.spec().title.to_string()))
            .unwrap_or_else(|| self.id.clone())
    }

    /// O lugar fixo de um tipo (o `id` é o próprio tipo): ao remover, só some
    /// da tela; uma instância extra é apagada.
    fn is_fixed_slot(&self) -> bool {
        self.id == self.type_id()
    }
}

/// O que um `add` pede.
#[derive(Debug, Clone, PartialEq)]
pub struct AddWidget {
    pub kind: DashboardWidgetType,
    pub size: Option<DashboardWidgetSize>,
    pub title: Option<String>,
    pub config: Option<Map<String, Value>>,
    pub position: Option<usize>,
}

/// Uma mudança pedida no layout.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutOperation {
    Add(AddWidget),
    Remove {
        id: String,
    },
    Move {
        id: String,
        position: usize,
    },
    Resize {
        id: String,
        size: DashboardWidgetSize,
    },
    Configure {
        id: String,
        title: Option<String>,
        config: Option<Map<String, Value>>,
    },
    Reset,
}

fn text_field(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn required_id(value: &Value) -> AppResult<String> {
    text_field(value, "id").ok_or_else(|| AppError::validation("Informe o id do card"))
}

fn position_field(value: &Value) -> AppResult<Option<usize>> {
    match value.get("position") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|position| usize::try_from(position).ok())
            .map(Some)
            .ok_or_else(|| AppError::validation("A posição é um inteiro a partir de 0")),
        Some(Value::String(text)) => text
            .trim()
            .parse()
            .map(Some)
            .map_err(|_| AppError::validation("A posição é um inteiro a partir de 0")),
        Some(_) => Err(AppError::validation("A posição é um inteiro a partir de 0")),
    }
}

fn size_field(value: &Value) -> AppResult<Option<DashboardWidgetSize>> {
    let Some(text) = text_field(value, "size") else {
        return Ok(None);
    };
    DashboardWidgetSize::ALL
        .into_iter()
        .find(|size| size.id() == text)
        .map(Some)
        .ok_or_else(|| {
            AppError::validation(format!(
                "Tamanho \"{text}\" inválido; use third, half, two_thirds ou full"
            ))
        })
}

fn config_field(value: &Value) -> AppResult<Option<Map<String, Value>>> {
    match value.get("config") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(map)) => Ok(Some(map.clone())),
        Some(_) => Err(AppError::validation("A configuração é um objeto")),
    }
}

/// O título como veio: `Some("")` pede o título padrão de volta.
fn title_field(value: &Value) -> Option<String> {
    match value.get("title")? {
        Value::String(text) => Some(text.trim().to_string()),
        _ => None,
    }
}

impl LayoutOperation {
    /// Lê uma operação no formato da ferramenta da IA (`{"op": "add", ...}`).
    ///
    /// # Errors
    ///
    /// `Validation` com a mensagem para quem pediu corrigir.
    pub fn parse(value: &Value) -> AppResult<Self> {
        let op = text_field(value, "op").unwrap_or_default();
        match op.as_str() {
            "add" => {
                let type_id = text_field(value, "type")
                    .ok_or_else(|| AppError::validation("Informe o type do card a adicionar"))?;
                let kind = DashboardWidgetType::from_id(&type_id)
                    .ok_or_else(|| unknown_type(&type_id))?;
                Ok(Self::Add(AddWidget {
                    kind,
                    size: size_field(value)?,
                    title: title_field(value).filter(|title| !title.is_empty()),
                    config: config_field(value)?,
                    position: position_field(value)?,
                }))
            }
            "remove" => Ok(Self::Remove {
                id: required_id(value)?,
            }),
            "move" => Ok(Self::Move {
                id: required_id(value)?,
                position: position_field(value)?
                    .ok_or_else(|| AppError::validation("Informe a posição de destino"))?,
            }),
            "resize" => Ok(Self::Resize {
                id: required_id(value)?,
                size: size_field(value)?
                    .ok_or_else(|| AppError::validation("Informe o novo tamanho"))?,
            }),
            "configure" => {
                let title = title_field(value);
                let config = config_field(value)?;
                if title.is_none() && config.is_none() {
                    return Err(AppError::validation(
                        "Informe o título ou a configuração a mudar",
                    ));
                }
                Ok(Self::Configure {
                    id: required_id(value)?,
                    title,
                    config,
                })
            }
            "reset" => Ok(Self::Reset),
            other => Err(AppError::validation(format!(
                "Operação \"{other}\" desconhecida; use add, remove, move, resize, configure ou reset"
            ))),
        }
    }
}

fn unknown_type(type_id: &str) -> AppError {
    let known: Vec<&str> = DashboardWidgetType::ALL
        .into_iter()
        .map(DashboardWidgetType::id)
        .collect();
    AppError::validation(format!(
        "Tipo de card \"{type_id}\" desconhecido; use um de: {}",
        known.join(", ")
    ))
}

/// Id ou número de recurso (`monitorId`, `deviceId`, `interfaceId`): inteiro
/// positivo ou `"all"`, como a tela grava.
fn resource_id(key: &str, value: &Value) -> AppResult<Value> {
    let invalid = || AppError::validation(format!("{key} deve ser um id numérico ou \"all\""));
    match value {
        Value::Number(number) => number
            .as_i64()
            .filter(|id| *id > 0)
            .map(Value::from)
            .ok_or_else(invalid),
        Value::String(text) if text.trim().eq_ignore_ascii_case("all") => Ok(Value::from("all")),
        Value::String(text) => text
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|id| *id > 0)
            .map(Value::from)
            .ok_or_else(invalid),
        _ => Err(invalid()),
    }
}

fn threshold(key: &str, value: &Value) -> AppResult<Value> {
    let number = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
    .filter(|number| number.is_finite() && *number >= 0.0)
    .ok_or_else(|| AppError::validation(format!("{key} deve ser um número a partir de 0")))?;
    Ok(serde_json::Number::from_f64(number).map_or(Value::Null, Value::Number))
}

/// Confere e normaliza um valor de `config`. `Null` remove a chave.
fn config_value(key: &str, value: &Value) -> AppResult<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    match key {
        "timeframe" => match value.as_str().map(str::trim) {
            Some(timeframe) if TIMEFRAMES.contains(&timeframe) => Ok(Value::from(timeframe)),
            _ => Err(AppError::validation(format!(
                "Período inválido; use {}",
                TIMEFRAMES.join(", ")
            ))),
        },
        "monitorId" | "deviceId" | "interfaceId" => resource_id(key, value),
        "interfaceName" => value
            .as_str()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(Value::from)
            .ok_or_else(|| AppError::validation("interfaceName deve ser um texto")),
        "dnsServerIds" => {
            let invalid =
                || AppError::validation("dnsServerIds deve ser uma lista de ids numéricos");
            let items = value.as_array().ok_or_else(invalid)?;
            items
                .iter()
                .map(|item| {
                    resource_id("dnsServerIds", item)
                        .ok()
                        .filter(Value::is_number)
                        .ok_or_else(invalid)
                })
                .collect::<AppResult<Vec<Value>>>()
                .map(Value::Array)
        }
        "warningThreshold" | "criticalThreshold" => threshold(key, value),
        other => Err(AppError::validation(format!(
            "Configuração \"{other}\" desconhecida; use: {}",
            CONFIG_KEYS.join(", ")
        ))),
    }
}

/// O patch de `config` como a pessoa lê na confirmação.
fn describe_config(patch: &Map<String, Value>) -> Vec<String> {
    patch
        .iter()
        .map(|(key, value)| {
            if value.is_null() {
                return format!("sem {}", config_label(key));
            }
            let shown = match value {
                Value::String(text) if text == "all" => "todos".to_string(),
                Value::String(text) => text.clone(),
                Value::Array(items) => items
                    .iter()
                    .map(|item| format!("#{item}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                Value::Number(number) if key.ends_with("Id") => format!("#{number}"),
                other => other.to_string(),
            };
            format!("{} {shown}", config_label(key))
        })
        .collect()
}

fn config_label(key: &str) -> &str {
    match key {
        "timeframe" => "período",
        "monitorId" => "monitor",
        "deviceId" => "dispositivo",
        "interfaceId" => "interface",
        "interfaceName" => "nome da interface",
        "dnsServerIds" => "servidores DNS",
        "warningThreshold" => "aviso em",
        "criticalThreshold" => "crítico em",
        other => other,
    }
}

/// Gerador do id de uma instância nova (o serviço usa hora + sorteio; os
/// testes, um contador).
pub type NewId<'a> = &'a mut dyn FnMut(DashboardWidgetType) -> String;

/// O layout inteiro, sempre na ordem de exibição.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardLayout {
    widgets: Vec<DashboardWidget>,
}

impl Default for DashboardLayout {
    fn default() -> Self {
        Self::default_layout()
    }
}

impl DashboardLayout {
    /// O layout de fábrica ([`DEFAULT_LAYOUT`]).
    #[must_use]
    pub fn default_layout() -> Self {
        let mut layout = Self {
            widgets: DEFAULT_LAYOUT
                .into_iter()
                .map(|(kind, size)| DashboardWidget::new(kind.id().to_string(), kind, size))
                .collect(),
        };
        layout.renumber();
        layout
    }

    /// O layout gravado. Item sem `id` (ou que não é objeto) é descartado,
    /// como a tela faz; a ordem vem do `order`, e na falta dele da posição.
    #[must_use]
    pub fn from_values(values: &[Value]) -> Self {
        let mut indexed: Vec<(i64, usize, DashboardWidget)> = values
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                let widget = serde_json::from_value::<DashboardWidget>(value.clone()).ok()?;
                (!widget.id.trim().is_empty()).then_some((
                    widget
                        .order
                        .unwrap_or_else(|| i64::try_from(index).unwrap_or(i64::MAX)),
                    index,
                    widget,
                ))
            })
            .collect();
        indexed.sort_by_key(|(order, index, _)| (*order, *index));
        let mut layout = Self {
            widgets: indexed.into_iter().map(|(_, _, widget)| widget).collect(),
        };
        layout.renumber();
        layout
    }

    /// No formato gravado.
    #[must_use]
    pub fn to_values(&self) -> Vec<Value> {
        self.widgets
            .iter()
            .filter_map(|widget| serde_json::to_value(widget).ok())
            .collect()
    }

    #[must_use]
    pub fn widgets(&self) -> &[DashboardWidget] {
        &self.widgets
    }

    /// Posição entre os visíveis (base 0), ou `None` quando o card está oculto.
    #[must_use]
    pub fn position_of(&self, id: &str) -> Option<usize> {
        self.widgets
            .iter()
            .filter(|widget| widget.visible)
            .position(|widget| widget.id == id)
    }

    fn renumber(&mut self) {
        for (order, widget) in self.widgets.iter_mut().enumerate() {
            widget.order = Some(i64::try_from(order).unwrap_or(i64::MAX));
        }
    }

    fn index_of(&self, id: &str) -> AppResult<usize> {
        self.widgets
            .iter()
            .position(|widget| widget.id == id)
            .ok_or_else(|| {
                AppError::validation(format!(
                    "Card \"{id}\" não está no layout; consulte os ids com get_dashboard_layout"
                ))
            })
    }

    /// Aplica uma operação e devolve a frase que a descreve.
    ///
    /// # Errors
    ///
    /// `Validation` para card ou tipo desconhecido, card único repetido ou
    /// configuração inválida.
    pub fn apply(&mut self, operation: &LayoutOperation, new_id: NewId<'_>) -> AppResult<String> {
        match operation {
            LayoutOperation::Add(add) => self.add(add, new_id),
            LayoutOperation::Remove { id } => self.remove(id),
            LayoutOperation::Move { id, position } => self.move_to(id, *position),
            LayoutOperation::Resize { id, size } => self.resize(id, *size),
            LayoutOperation::Configure { id, title, config } => {
                self.configure(id, title.as_deref(), config.as_ref())
            }
            LayoutOperation::Reset => Ok(self.reset()),
        }
    }

    /// Aplica todas, em ordem; a primeira que falhar interrompe com o número
    /// dela. Quem chama aplica numa cópia para ser tudo ou nada.
    ///
    /// # Errors
    ///
    /// O de [`Self::apply`], prefixado com o número da operação.
    pub fn apply_all(
        &mut self,
        operations: &[LayoutOperation],
        new_id: NewId<'_>,
    ) -> AppResult<Vec<String>> {
        operations
            .iter()
            .enumerate()
            .map(|(index, operation)| {
                self.apply(operation, &mut *new_id).map_err(|error| {
                    AppError::validation(format!("Operação {}: {error}", index + 1))
                })
            })
            .collect()
    }

    /// Adiciona um card. Tipo único já visível é recusado; oculto, volta.
    ///
    /// # Errors
    ///
    /// Card único já visível ou configuração inválida.
    pub fn add(&mut self, add: &AddWidget, new_id: NewId<'_>) -> AppResult<String> {
        let kind = add.kind;
        let existing = (!kind.allows_multiple())
            .then(|| {
                self.widgets
                    .iter()
                    .position(|widget| widget.id == kind.id() || widget.type_id() == kind.id())
            })
            .flatten();
        let (index, restored) = match existing {
            Some(index) if self.widgets[index].visible => {
                let widget = &self.widgets[index];
                return Err(AppError::validation(format!(
                    "O card \"{}\" (id {}) já está no dashboard e só pode haver um; use move, resize ou configure",
                    widget.label(),
                    widget.id
                )));
            }
            Some(index) => {
                self.widgets[index].visible = true;
                (index, true)
            }
            None => {
                let id = if kind.allows_multiple() {
                    new_id(kind)
                } else {
                    kind.id().to_string()
                };
                let size = add.size.unwrap_or(kind.spec().default_size);
                self.widgets.push(DashboardWidget::new(id, kind, size));
                (self.widgets.len() - 1, false)
            }
        };
        if let Some(size) = add.size {
            self.widgets[index].set_size(size);
        }
        if let Some(title) = &add.title {
            self.widgets[index].title = Some(title.clone());
        }
        let applied = match &add.config {
            Some(config) => apply_config(&mut self.widgets[index].config, config)?,
            None => Map::new(),
        };
        let id = self.widgets[index].id.clone();
        self.renumber();
        if let Some(position) = add.position {
            self.move_to(&id, position)?;
        }
        let widget = &self.widgets[self.index_of(&id)?];
        let mut details = vec![widget.size().label().to_string()];
        details.extend(describe_config(&applied));
        let verb = if restored {
            "Mostrar de novo"
        } else {
            "Adicionar"
        };
        Ok(format!(
            "{verb} o card \"{}\" ({}) na posição {}",
            widget.label(),
            details.join(", "),
            self.position_of(&id).map_or(0, |position| position + 1)
        ))
    }

    /// Remove um card: o lugar fixo de um tipo só fica oculto (volta pelo
    /// catálogo); uma instância extra é apagada.
    ///
    /// # Errors
    ///
    /// Card inexistente.
    pub fn remove(&mut self, id: &str) -> AppResult<String> {
        let index = self.index_of(id)?;
        let label = self.widgets[index].label();
        if self.widgets[index].is_fixed_slot() {
            self.widgets[index].visible = false;
            return Ok(format!(
                "Ocultar o card \"{label}\" (pode voltar pelo catálogo)"
            ));
        }
        self.widgets.remove(index);
        self.renumber();
        Ok(format!("Remover o card \"{label}\""))
    }

    /// Leva um card visível à posição dada entre os visíveis. Os ocultos
    /// ficam onde estavam.
    ///
    /// # Errors
    ///
    /// Card inexistente ou oculto.
    pub fn move_to(&mut self, id: &str, position: usize) -> AppResult<String> {
        let index = self.index_of(id)?;
        if !self.widgets[index].visible {
            return Err(AppError::validation(format!(
                "O card \"{}\" está oculto; use add para mostrá-lo na posição desejada",
                self.widgets[index].label()
            )));
        }
        let slots: Vec<usize> = self
            .widgets
            .iter()
            .enumerate()
            .filter(|(_, widget)| widget.visible)
            .map(|(slot, _)| slot)
            .collect();
        let mut visible: Vec<DashboardWidget> = slots
            .iter()
            .map(|slot| self.widgets[*slot].clone())
            .collect();
        let from = visible
            .iter()
            .position(|widget| widget.id == id)
            .unwrap_or_default();
        let widget = visible.remove(from);
        let target = position.min(visible.len());
        let label = widget.label();
        visible.insert(target, widget);
        for (slot, widget) in slots.into_iter().zip(visible) {
            self.widgets[slot] = widget;
        }
        self.renumber();
        Ok(format!("Mover \"{label}\" para a posição {}", target + 1))
    }

    /// Muda a largura de um card.
    ///
    /// # Errors
    ///
    /// Card inexistente.
    pub fn resize(&mut self, id: &str, size: DashboardWidgetSize) -> AppResult<String> {
        let index = self.index_of(id)?;
        self.widgets[index].set_size(size);
        Ok(format!(
            "Mudar \"{}\" para {}",
            self.widgets[index].label(),
            size.label()
        ))
    }

    /// Muda o título e/ou parte da configuração. Título vazio volta ao padrão
    /// do tipo; valor `null` no patch remove a chave.
    ///
    /// # Errors
    ///
    /// Card inexistente, chave desconhecida ou valor inválido.
    pub fn configure(
        &mut self,
        id: &str,
        title: Option<&str>,
        config: Option<&Map<String, Value>>,
    ) -> AppResult<String> {
        let index = self.index_of(id)?;
        let label = self.widgets[index].label();
        let mut details = Vec::new();
        if let Some(config) = config {
            let applied = apply_config(&mut self.widgets[index].config, config)?;
            details.extend(describe_config(&applied));
        }
        if let Some(title) = title {
            let widget = &mut self.widgets[index];
            if title.is_empty() {
                widget.title = widget.kind().map(|kind| kind.spec().title.to_string());
                details.insert(0, "título padrão".to_string());
            } else {
                widget.title = Some(title.to_string());
                details.insert(0, format!("título \"{title}\""));
            }
        }
        Ok(format!("Configurar \"{label}\": {}", details.join(", ")))
    }

    /// Volta ao layout de fábrica.
    pub fn reset(&mut self) -> String {
        *self = Self::default_layout();
        "Restaurar o layout padrão do dashboard (os cards personalizados saem)".to_string()
    }
}

/// Confere o patch inteiro antes de mexer: inválido não muda nada. Devolve
/// o patch já normalizado (`"7"` vira `7`), para a frase da confirmação.
fn apply_config(
    config: &mut Map<String, Value>,
    patch: &Map<String, Value>,
) -> AppResult<Map<String, Value>> {
    let checked = patch
        .iter()
        .map(|(key, value)| Ok((key.clone(), config_value(key, value)?)))
        .collect::<AppResult<Map<String, Value>>>()?;
    let mut next = config.clone();
    for (key, value) in &checked {
        if value.is_null() {
            next.remove(key);
        } else {
            next.insert(key.clone(), value.clone());
        }
    }
    let limit = |key: &str| next.get(key).and_then(Value::as_f64);
    if let (Some(warning), Some(critical)) = (limit("warningThreshold"), limit("criticalThreshold"))
    {
        if warning > critical {
            return Err(AppError::validation(
                "O limite de aviso não pode passar do crítico",
            ));
        }
    }
    *config = next;
    Ok(checked)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn contador() -> impl FnMut(DashboardWidgetType) -> String {
        let mut n = 0;
        move |kind| {
            n += 1;
            format!("{}_{n}", kind.id())
        }
    }

    fn visiveis(layout: &DashboardLayout) -> Vec<String> {
        layout
            .widgets()
            .iter()
            .filter(|widget| widget.visible)
            .map(|widget| widget.id.clone())
            .collect()
    }

    fn op(value: Value) -> LayoutOperation {
        LayoutOperation::parse(&value).unwrap()
    }

    #[test]
    fn layout_padrao_na_ordem_e_tamanhos_combinados() {
        let layout = DashboardLayout::default_layout();
        assert_eq!(
            visiveis(&layout),
            vec![
                "stat_cards",
                "health_gauge",
                "active_alerts",
                "latency_time_series",
                "event_distribution",
                "unstable_targets",
                "events_feed",
                "network_monitors",
                "docker_summary"
            ]
        );
        let tamanhos: Vec<DashboardWidgetSize> =
            layout.widgets().iter().map(DashboardWidget::size).collect();
        assert_eq!(tamanhos[0], DashboardWidgetSize::Full);
        assert_eq!(tamanhos[1], DashboardWidgetSize::Half);
        assert_eq!(tamanhos[7], DashboardWidgetSize::Full);
        assert_eq!(layout.widgets()[3].order, Some(3));
    }

    #[test]
    fn campos_desconhecidos_sobrevivem_ida_e_volta() {
        let gravado = vec![
            json!({"id": "b", "type": "cpu_usage", "order": 1, "lg": 6, "md": 6, "visible": true,
                   "config": {"deviceId": 3, "chartType": "area"}, "pinned": true}),
            json!({"id": "a", "type": "stat_cards", "order": 0, "visible": false}),
            json!({"semId": true}),
        ];
        let layout = DashboardLayout::from_values(&gravado);
        let valores = layout.to_values();
        assert_eq!(valores.len(), 2, "item sem id é descartado, como na tela");
        assert_eq!(valores[0]["id"], "a");
        assert_eq!(valores[0]["visible"], false);
        assert_eq!(valores[1]["pinned"], true);
        assert_eq!(valores[1]["config"]["chartType"], "area");
        assert_eq!(valores[1]["order"], 1);
    }

    #[test]
    fn adicionar_instancia_gera_id_e_aceita_posicao_e_config() {
        let mut layout = DashboardLayout::default_layout();
        let mut ids = contador();
        let frase = layout
            .apply(
                &op(json!({"op": "add", "type": "cpu_usage", "position": 1,
                           "config": {"deviceId": "7", "timeframe": "1h"}, "title": "CPU da borda"})),
                &mut ids,
            )
            .unwrap();
        assert_eq!(layout.position_of("cpu_usage_1"), Some(1));
        let card = &layout.widgets()[layout.index_of("cpu_usage_1").unwrap()];
        assert_eq!(card.config["deviceId"], 7);
        assert_eq!(card.title.as_deref(), Some("CPU da borda"));
        assert_eq!(card.size(), DashboardWidgetSize::Half);
        assert!(
            frase.contains("Adicionar o card \"CPU da borda\""),
            "{frase}"
        );
        assert!(frase.contains("posição 2"), "{frase}");
        assert!(frase.contains("dispositivo #7"), "{frase}");

        layout
            .apply(&op(json!({"op": "add", "type": "cpu_usage"})), &mut ids)
            .unwrap();
        assert!(
            layout.position_of("cpu_usage_2").is_some(),
            "vários do mesmo tipo"
        );
    }

    #[test]
    fn card_unico_repetido_e_recusado_e_oculto_volta() {
        let mut layout = DashboardLayout::default_layout();
        let mut ids = contador();
        let erro = layout
            .apply(&op(json!({"op": "add", "type": "stat_cards"})), &mut ids)
            .unwrap_err();
        assert!(erro.to_string().contains("já está no dashboard"), "{erro}");

        layout.remove("stat_cards").unwrap();
        assert_eq!(layout.position_of("stat_cards"), None);
        let frase = layout
            .apply(
                &op(json!({"op": "add", "type": "stat_cards", "size": "half"})),
                &mut ids,
            )
            .unwrap();
        assert!(frase.starts_with("Mostrar de novo"), "{frase}");
        assert_eq!(layout.position_of("stat_cards"), Some(0), "volta ao lugar");
        assert_eq!(layout.widgets().len(), 9, "não duplica");

        layout
            .apply(&op(json!({"op": "add", "type": "saas_services"})), &mut ids)
            .unwrap();
        assert_eq!(layout.position_of("saas_services"), Some(9), "id = tipo");
    }

    #[test]
    fn remover_oculta_o_lugar_fixo_e_apaga_a_instancia() {
        let mut layout = DashboardLayout::default_layout();
        let mut ids = contador();
        layout
            .apply(&op(json!({"op": "add", "type": "ram_usage"})), &mut ids)
            .unwrap();
        assert_eq!(
            layout.remove("ram_usage_1").unwrap(),
            "Remover o card \"Uso de RAM\""
        );
        assert_eq!(layout.widgets().len(), 9);
        assert!(layout.remove("events_feed").unwrap().starts_with("Ocultar"));
        assert_eq!(layout.widgets().len(), 9);
        assert!(!visiveis(&layout).contains(&"events_feed".to_string()));
        assert!(layout.remove("inexistente").is_err());
    }

    #[test]
    fn mover_conta_so_os_visiveis_e_mantem_os_ocultos_no_lugar() {
        let mut layout = DashboardLayout::default_layout();
        layout.remove("health_gauge").unwrap();
        let frase = layout.move_to("docker_summary", 0).unwrap();
        assert_eq!(frase, "Mover \"Resumo Docker\" para a posição 1");
        assert_eq!(visiveis(&layout)[0], "docker_summary");
        assert_eq!(visiveis(&layout)[1], "stat_cards");
        assert_eq!(
            layout.widgets()[1].id,
            "health_gauge",
            "oculto no mesmo lugar"
        );
        let ordens: Vec<i64> = layout.widgets().iter().filter_map(|w| w.order).collect();
        assert_eq!(ordens, (0..9).collect::<Vec<_>>());

        layout.move_to("stat_cards", 99).unwrap();
        assert_eq!(
            visiveis(&layout).last().unwrap(),
            "stat_cards",
            "posição limitada ao fim"
        );
        assert!(
            layout.move_to("health_gauge", 0).is_err(),
            "oculto não se move"
        );
    }

    #[test]
    fn redimensionar_grava_as_colunas() {
        let mut layout = DashboardLayout::default_layout();
        let frase = layout
            .resize("active_alerts", DashboardWidgetSize::Third)
            .unwrap();
        assert_eq!(
            frase,
            "Mudar \"Alertas Críticos Ativos\" para um terço da largura"
        );
        let card = &layout.to_values()[2];
        assert_eq!(
            (&card["cols"], &card["sm"], &card["md"], &card["lg"]),
            (&json!(12), &json!(12), &json!(6), &json!(4))
        );
        assert!(
            LayoutOperation::parse(&json!({"op": "resize", "id": "x", "size": "huge"})).is_err()
        );
    }

    #[test]
    fn configurar_valida_periodo_ids_e_limites() {
        let mut layout = DashboardLayout::default_layout();
        let patch = |value: Value| value.as_object().unwrap().clone();
        let frase = layout
            .configure(
                "latency_time_series",
                Some("Latência da matriz"),
                Some(&patch(json!({"timeframe": "24h", "monitorId": "all"}))),
            )
            .unwrap();
        assert!(frase.contains("título \"Latência da matriz\""), "{frase}");
        assert!(frase.contains("período 24h"), "{frase}");
        assert!(frase.contains("monitor todos"), "{frase}");

        for invalido in [
            json!({"timeframe": "2h"}),
            json!({"monitorId": "abc"}),
            json!({"deviceId": -1}),
            json!({"dnsServerIds": [1, "x"]}),
            json!({"inventado": 1}),
            json!({"warningThreshold": 95, "criticalThreshold": 80}),
        ] {
            let antes = layout.clone();
            assert!(
                layout
                    .configure("latency_time_series", None, Some(&patch(invalido.clone())))
                    .is_err(),
                "{invalido}"
            );
            assert_eq!(layout, antes, "patch inválido não muda nada");
        }

        layout
            .configure(
                "latency_time_series",
                Some(""),
                Some(&patch(json!({"timeframe": null, "dnsServerIds": ["3", 4]}))),
            )
            .unwrap();
        let card = &layout.widgets()[3];
        assert_eq!(card.title.as_deref(), Some("Latência & Perda de Pacotes"));
        assert!(card.config.get("timeframe").is_none());
        assert_eq!(card.config["dnsServerIds"], json!([3, 4]));
        assert!(layout.configure("nada", Some("x"), None).is_err());
    }

    #[test]
    fn restaurar_volta_ao_padrao() {
        let mut layout = DashboardLayout::default_layout();
        let mut ids = contador();
        layout
            .apply(&op(json!({"op": "add", "type": "binary_status"})), &mut ids)
            .unwrap();
        layout.remove("stat_cards").unwrap();
        layout.apply(&LayoutOperation::Reset, &mut ids).unwrap();
        assert_eq!(layout, DashboardLayout::default_layout());
    }

    #[test]
    fn lote_para_na_primeira_falha_com_o_numero_dela() {
        let mut layout = DashboardLayout::default_layout();
        let mut ids = contador();
        let erro = layout
            .apply_all(
                &[
                    op(json!({"op": "move", "id": "events_feed", "position": 0})),
                    op(json!({"op": "remove", "id": "nao_existe"})),
                ],
                &mut ids,
            )
            .unwrap_err();
        assert!(erro.to_string().starts_with("Operação 2:"), "{erro}");
    }

    #[test]
    fn leitura_das_operacoes_explica_o_erro() {
        let erro = |value: Value| LayoutOperation::parse(&value).unwrap_err().to_string();
        assert!(erro(json!({"op": "add", "type": "grafico"})).contains("cpu_usage"));
        assert!(erro(json!({"op": "voar"})).contains("desconhecida"));
        assert!(erro(json!({"op": "move", "id": "x"})).contains("posição"));
        assert!(erro(json!({"op": "configure", "id": "x"})).contains("título"));
        assert_eq!(
            op(json!({"op": "move", "id": "x", "position": "2"})),
            LayoutOperation::Move {
                id: "x".into(),
                position: 2
            }
        );
    }
}
