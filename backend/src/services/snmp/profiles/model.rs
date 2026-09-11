use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorStateSpec {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

impl<'de> serde::Deserialize<'de> for SensorStateSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum StateSpecHelper {
            Simple(String),
            #[serde(rename_all = "camelCase")]
            Detailed {
                label: String,
                #[serde(default)]
                color: Option<String>,
                #[serde(default)]
                icon: Option<String>,
            },
        }

        match StateSpecHelper::deserialize(deserializer)? {
            StateSpecHelper::Simple(label) => Ok(Self {
                label,
                color: None,
                icon: None,
            }),
            StateSpecHelper::Detailed { label, color, icon } => Ok(Self { label, color, icon }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpProfileSensor {
    pub key: String,
    pub label: String,
    pub oid: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default = "default_data_type")]
    pub data_type: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub states: Option<HashMap<String, SensorStateSpec>>,
}

fn default_scale() -> f64 {
    1.0
}

fn default_category() -> String {
    "general".to_string()
}

fn default_data_type() -> String {
    "float".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpDeviceProfile {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub category: String,
    #[serde(default)]
    pub sys_object_id_prefix: Option<String>,
    #[serde(default)]
    pub sys_descr_pattern: Option<String>,
    pub sensors: Vec<SnmpProfileSensor>,
    #[serde(default)]
    pub is_builtin: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpProfileSummary {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub category: String,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredSensor {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub name: String,
    pub oid: String,
    pub unit: String,
    pub scale: f64,
    pub category: String,
    #[serde(default = "default_data_type")]
    pub data_type: String,
    pub raw_value: Option<f64>,
    pub value: Option<f64>,
    pub formatted_value: String,
    pub is_monitored: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub states: Option<HashMap<String, SensorStateSpec>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_simple_string_states() {
        let json = r#"{
            "0": "Inativo",
            "1": "Carga",
            "2": "Flutuação",
            "3": "Equalização"
        }"#;
        let states: HashMap<String, SensorStateSpec> = serde_json::from_str(json).unwrap();
        assert_eq!(states["0"].label, "Inativo");
        assert_eq!(states["1"].label, "Carga");
        assert_eq!(states["2"].label, "Flutuação");
        assert_eq!(states["3"].label, "Equalização");
        assert_eq!(states["1"].color, None);
        assert_eq!(states["1"].icon, None);
    }

    #[test]
    fn test_deserialize_detailed_object_states() {
        let json = r#"{
            "1": { "label": "Carga", "color": "success", "icon": "mdi-battery-charging" },
            "2": { "label": "Flutuação", "color": "info" }
        }"#;
        let states: HashMap<String, SensorStateSpec> = serde_json::from_str(json).unwrap();
        assert_eq!(states["1"].label, "Carga");
        assert_eq!(states["1"].color.as_deref(), Some("success"));
        assert_eq!(states["1"].icon.as_deref(), Some("mdi-battery-charging"));
        assert_eq!(states["2"].label, "Flutuação");
        assert_eq!(states["2"].color.as_deref(), Some("info"));
        assert_eq!(states["2"].icon, None);
    }
}
