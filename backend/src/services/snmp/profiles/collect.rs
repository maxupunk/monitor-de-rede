use std::collections::HashMap;

use super::model::{DiscoveredSensor, SensorStateSpec, SnmpDeviceProfile};
use crate::services::snmp::client::{SnmpClient, SnmpError};

/// Coleta os valores de todos os sensores descritos no perfil SNMP fornecido.
pub async fn collect_profile_sensors(
    client: &SnmpClient,
    profile: &SnmpDeviceProfile,
) -> Result<Vec<DiscoveredSensor>, SnmpError> {
    if profile.sensors.is_empty() {
        return Ok(Vec::new());
    }

    let oid_refs: Vec<&str> = profile.sensors.iter().map(|s| s.oid.as_str()).collect();
    let values = client.get(&oid_refs).await?;

    let mut result = Vec::with_capacity(profile.sensors.len());
    for sensor in &profile.sensors {
        let raw_opt = values
            .get(&sensor.oid)
            .and_then(|v| v.as_ref())
            .and_then(|v| v.number());

        let (raw_value, value, formatted_value) = match raw_opt {
            Some(num) => {
                #[allow(clippy::cast_precision_loss)]
                let raw = num as f64;
                let scaled = raw * sensor.scale;
                let fmt = format_sensor_value(raw, scaled, &sensor.unit, sensor.states.as_ref());
                (Some(raw), Some(scaled), fmt)
            }
            None => {
                let text = values
                    .get(&sensor.oid)
                    .and_then(|v| v.as_ref())
                    .map(|v| v.text())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| "Sem leitura".to_string());
                (None, None, text)
            }
        };

        result.push(DiscoveredSensor {
            key: sensor.key.clone(),
            label: sensor.label.clone(),
            name: sensor.label.clone(),
            oid: sensor.oid.clone(),
            unit: sensor.unit.clone(),
            scale: sensor.scale,
            category: sensor.category.clone(),
            data_type: sensor.data_type.clone(),
            raw_value,
            value,
            formatted_value,
            is_monitored: false,
            icon: sensor.icon.clone(),
            color: sensor.color.clone(),
            states: sensor.states.clone(),
        });
    }

    Ok(result)
}

/// Formata a leitura de um sensor de maneira amigável, decodificando estados ou adicionando unidades.
#[must_use]
pub fn format_sensor_value(
    raw: f64,
    scaled: f64,
    unit: &str,
    states: Option<&HashMap<String, SensorStateSpec>>,
) -> String {
    if let Some(state_map) = states {
        #[allow(clippy::cast_possible_truncation)]
        let raw_key = (raw.round() as i64).to_string();
        #[allow(clippy::cast_possible_truncation)]
        let scaled_key = (scaled.round() as i64).to_string();
        if let Some(spec) = state_map
            .get(&raw_key)
            .or_else(|| state_map.get(&scaled_key))
        {
            return spec.label.clone();
        }
    }

    if unit == "kWh" {
        format!("{scaled:.2} kWh")
    } else if unit.is_empty() {
        if scaled.fract() == 0.0 {
            format!("{scaled:.0}")
        } else {
            format!("{scaled:.1}")
        }
    } else if scaled.fract() == 0.0 {
        format!("{scaled:.0} {unit}")
    } else {
        format!("{scaled:.1} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sensor_value_with_states() {
        let mut states = HashMap::new();
        states.insert(
            "0".to_string(),
            SensorStateSpec {
                label: "Descarregando".to_string(),
                color: Some("warning".to_string()),
                icon: Some("mdi-battery-arrow-down".to_string()),
            },
        );
        states.insert(
            "1".to_string(),
            SensorStateSpec {
                label: "Carregando".to_string(),
                color: Some("success".to_string()),
                icon: Some("mdi-battery-charging".to_string()),
            },
        );

        assert_eq!(
            format_sensor_value(0.0, 0.0, "", Some(&states)),
            "Descarregando"
        );
        assert_eq!(
            format_sensor_value(1.0, 1.0, "", Some(&states)),
            "Carregando"
        );
        // Sem estado mapeado cai na formatação numérica
        assert_eq!(format_sensor_value(2.0, 2.0, "", Some(&states)), "2");
    }

    #[test]
    fn test_format_sensor_value_units() {
        assert_eq!(format_sensor_value(153.0, 15.3, "V", None), "15.3 V");
        assert_eq!(format_sensor_value(120.0, 12.0, "V", None), "12 V");
        assert_eq!(format_sensor_value(4119.0, 4.119, "kWh", None), "4.12 kWh");
        assert_eq!(format_sensor_value(29.0, 29.0, "°C", None), "29 °C");
    }
}
