use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZabbixParseError {
    #[error("O conteudo enviado nao e um export JSON ou XML valido do Zabbix")]
    InvalidContent,
    #[error("O export do Zabbix nao contem templates")]
    MissingTemplates,
    #[error("Template Zabbix sem nome")]
    MissingTemplateName,
    #[error("Nenhum item SNMP_AGENT valido foi encontrado no template")]
    MissingSnmpItems,
}

#[derive(Debug, Clone)]
pub struct ParsedZabbixItem {
    pub uuid: Option<String>,
    pub name: String,
    pub key: String,
    pub snmp_oid: String,
    pub value_type: String,
    pub units: Option<String>,
    pub multiplier: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ParsedZabbixTemplate {
    pub uuid: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub items: Vec<ParsedZabbixItem>,
    pub skipped_items: usize,
}

#[derive(Debug, Clone)]
pub struct ParsedZabbixExport {
    pub raw_export: Value,
    pub templates: Vec<ParsedZabbixTemplate>,
}

pub fn parse_zabbix_template_export(content: &str) -> Result<ParsedZabbixExport, ZabbixParseError> {
    if let Ok(raw) = serde_json::from_str::<Value>(content) {
        return parse_json_export(raw);
    }
    parse_xml_export(content)
}

fn parse_json_export(raw: Value) -> Result<ParsedZabbixExport, ZabbixParseError> {
    let root = raw.get("zabbix_export").unwrap_or(&raw);
    let version = text(root, &["version"]);
    let templates = root
        .get("templates")
        .and_then(Value::as_array)
        .ok_or(ZabbixParseError::MissingTemplates)?;
    let templates = templates
        .iter()
        .map(|template| parse_json_template(template, version.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    if templates.is_empty() {
        return Err(ZabbixParseError::MissingTemplates);
    }
    Ok(ParsedZabbixExport {
        raw_export: raw,
        templates,
    })
}

fn parse_json_template(
    template: &Value,
    version: Option<String>,
) -> Result<ParsedZabbixTemplate, ZabbixParseError> {
    let name =
        text(template, &["name", "template"]).ok_or(ZabbixParseError::MissingTemplateName)?;
    let mut items = Vec::new();
    let mut skipped_items = 0;
    for item in template
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match parse_json_item(item) {
            Some(item) => items.push(item),
            None => skipped_items += 1,
        }
    }
    if items.is_empty() {
        return Err(ZabbixParseError::MissingSnmpItems);
    }
    Ok(ParsedZabbixTemplate {
        uuid: text(template, &["uuid"]),
        name,
        description: text(template, &["description"]),
        version,
        items,
        skipped_items,
    })
}

fn parse_json_item(item: &Value) -> Option<ParsedZabbixItem> {
    if !is_snmp_agent(text(item, &["type"]).as_deref()) {
        return None;
    }
    let key = text(item, &["key_", "key"])?;
    let snmp_oid = text(item, &["snmp_oid", "snmpOid"])?;
    Some(ParsedZabbixItem {
        uuid: text(item, &["uuid"]),
        name: text(item, &["name"]).unwrap_or_else(|| key.clone()),
        key,
        snmp_oid,
        value_type: text(item, &["value_type", "valueType"]).unwrap_or_else(|| "UNSIGNED".into()),
        units: text(item, &["units"]),
        multiplier: multiplier(item),
    })
}

fn text(value: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn multiplier(item: &Value) -> Option<f32> {
    item.get("multiplier")
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .or_else(|| {
            item.get("preprocessing")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|step| text(step, &["type"]).is_some_and(|kind| kind == "MULTIPLIER"))
                .and_then(|step| text(step, &["params", "parameters"]))
                .and_then(|value| value.parse().ok())
        })
}

fn is_snmp_agent(value: Option<&str>) -> bool {
    matches!(value.map(|value| value.trim().to_ascii_uppercase()), Some(value) if value == "SNMP_AGENT" || value == "4")
}

#[derive(Debug, Deserialize)]
struct XmlExport {
    version: Option<String>,
    templates: XmlTemplates,
}
#[derive(Debug, Deserialize, Default)]
struct XmlTemplates {
    #[serde(default)]
    template: Vec<XmlTemplate>,
}
#[derive(Debug, Deserialize)]
struct XmlTemplate {
    uuid: Option<String>,
    name: Option<String>,
    template: Option<String>,
    description: Option<String>,
    #[serde(default)]
    items: XmlItems,
}
#[derive(Debug, Deserialize, Default)]
struct XmlItems {
    #[serde(default)]
    item: Vec<XmlItem>,
}
#[derive(Debug, Deserialize)]
struct XmlItem {
    uuid: Option<String>,
    name: Option<String>,
    key: Option<String>,
    key_: Option<String>,
    r#type: Option<String>,
    snmp_oid: Option<String>,
    value_type: Option<String>,
    units: Option<String>,
    multiplier: Option<f32>,
    #[serde(default)]
    preprocessing: XmlPreprocessing,
}
#[derive(Debug, Deserialize, Default)]
struct XmlPreprocessing {
    #[serde(default)]
    step: Vec<XmlPreprocessingStep>,
}
#[derive(Debug, Deserialize)]
struct XmlPreprocessingStep {
    r#type: Option<String>,
    params: Option<String>,
}

fn parse_xml_export(content: &str) -> Result<ParsedZabbixExport, ZabbixParseError> {
    let root: XmlExport =
        quick_xml::de::from_str(content).map_err(|_| ZabbixParseError::InvalidContent)?;
    let templates = root
        .templates
        .template
        .into_iter()
        .map(|template| {
            let name = template
                .name
                .or(template.template)
                .filter(|value| !value.trim().is_empty())
                .ok_or(ZabbixParseError::MissingTemplateName)?;
            let mut items = Vec::new();
            let mut skipped_items = 0;
            for item in template.items.item {
                let key = item.key.or(item.key_);
                let oid = item.snmp_oid;
                if !is_snmp_agent(item.r#type.as_deref()) || key.is_none() || oid.is_none() {
                    skipped_items += 1;
                    continue;
                }
                let key = key.unwrap_or_default();
                let multiplier = item.multiplier.or_else(|| {
                    item.preprocessing
                        .step
                        .iter()
                        .find(|step| step.r#type.as_deref() == Some("MULTIPLIER"))
                        .and_then(|step| step.params.as_ref())
                        .and_then(|value| value.parse().ok())
                });
                items.push(ParsedZabbixItem {
                    uuid: item.uuid,
                    name: item.name.unwrap_or_else(|| key.clone()),
                    key,
                    snmp_oid: oid.unwrap_or_default(),
                    value_type: item.value_type.unwrap_or_else(|| "UNSIGNED".into()),
                    units: item.units,
                    multiplier,
                });
            }
            if items.is_empty() {
                return Err(ZabbixParseError::MissingSnmpItems);
            }
            Ok(ParsedZabbixTemplate {
                uuid: template.uuid,
                name,
                description: template.description,
                version: root.version.clone(),
                items,
                skipped_items,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if templates.is_empty() {
        return Err(ZabbixParseError::MissingTemplates);
    }
    Ok(ParsedZabbixExport {
        raw_export: serde_json::json!({ "format": "zabbix-xml", "content": content }),
        templates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtra_itens_nao_snmp_e_le_multiplier_json() {
        let export = parse_zabbix_template_export(
            r#"{"zabbix_export":{"version":"7.0","templates":[{"uuid":"t","name":"Router","items":[{"type":"SNMP_AGENT","key_":"if.in","snmp_oid":"1.3.6.1.2.1.2.2.1.10.1","preprocessing":[{"type":"MULTIPLIER","params":"0.125"}]},{"type":"ZABBIX_ACTIVE","key_":"ignored"}]}]}}"#,
        )
        .unwrap();
        assert_eq!(export.templates[0].items.len(), 1);
        assert_eq!(export.templates[0].items[0].multiplier, Some(0.125));
        assert_eq!(export.templates[0].skipped_items, 1);
    }

    #[test]
    fn le_export_xml() {
        let export = parse_zabbix_template_export(
            r#"<zabbix_export><version>7.0</version><templates><template><name>Switch</name><items><item><type>SNMP_AGENT</type><key>if.out</key><snmp_oid>1.3.6.1.2.1.2.2.1.16.1</snmp_oid></item></items></template></templates></zabbix_export>"#,
        )
        .unwrap();
        assert_eq!(export.templates[0].name, "Switch");
        assert_eq!(export.templates[0].items[0].value_type, "UNSIGNED");
    }
}
