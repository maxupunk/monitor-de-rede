//! Traduz o `CheckResult` de um monitor para o vocabulário das regras (§8.7).

use serde_json::{json, Value};

use crate::services::{
    alerts::{contracts::AlertDataset, fields},
    monitoring::contracts::CheckResult,
};

/// Nomes das métricas produzidas pelos checkers → chaves usadas nas condições.
/// Mantém o vocabulário da UI alinhado ao avaliador.
const METRIC_FIELD_MAP: [(&str, &str); 12] = [
    ("latency", fields::LATENCY_MS),
    ("response_time", fields::LATENCY_MS),
    ("packet_loss", fields::PACKET_LOSS),
    ("status_code", fields::STATUS_CODE),
    ("connect_time", fields::CONNECT_TIME_MS),
    ("resolution_time", fields::RESOLUTION_TIME_MS),
    ("dns_lookup_time", fields::RESOLUTION_TIME_MS),
    ("if_oper_status", fields::IF_OPER_STATUS),
    ("if_speed", fields::IF_SPEED),
    ("snmp_uptime", fields::SNMP_UPTIME),
    ("inBps", fields::IN_BPS),
    ("outBps", fields::OUT_BPS),
];

fn field_for(metric_name: &str) -> Option<&'static str> {
    METRIC_FIELD_MAP
        .iter()
        .find(|(name, _)| *name == metric_name)
        .map(|(_, field)| *field)
}

/// Monta os fatos de um resultado de monitor.
///
/// `latencyMs` nasce explicitamente `null` (e não ausente) para o avaliador
/// distinguir "o monitor não mede latência" de "o campo nem existe": uma regra
/// de latência aplicada a um monitor que não a produz nunca dispara.
#[must_use]
pub fn build(monitor_type: &str, result: &CheckResult) -> AlertDataset {
    let mut dataset = AlertDataset::new();
    dataset.insert(fields::STATUS.into(), json!(result.status.as_str()));
    dataset.insert("success".into(), json!(result.success));
    dataset.insert(fields::DURATION_MS.into(), json!(result.duration_ms));
    dataset.insert("type".into(), json!(monitor_type));
    dataset.insert(fields::LATENCY_MS.into(), Value::Null);

    for metric in &result.metrics {
        let Some(field) = field_for(&metric.name) else {
            continue;
        };
        // `latency` tem precedência sobre `response_time` quando ambos existem.
        if field == fields::LATENCY_MS
            && metric.name == "response_time"
            && dataset.get(fields::LATENCY_MS) != Some(&Value::Null)
        {
            continue;
        }
        dataset.insert(field.to_string(), json!(metric.value));
    }

    // Campos extras publicados no `data` do checker (ex.: `statusCode` do HTTP).
    if let Value::Object(extras) = &result.data {
        for (key, value) in extras {
            if !dataset.contains_key(key) {
                dataset.insert(key.clone(), value.clone());
            }
        }
    }

    dataset
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::monitoring::contracts::{CheckMetric, MonitorStatus};
    use chrono::Utc;

    fn resultado(metrics: Vec<CheckMetric>, data: Value) -> CheckResult {
        let now = Utc::now();
        CheckResult {
            success: true,
            status: MonitorStatus::Up,
            started_at: now,
            finished_at: now,
            duration_ms: 42,
            message: None,
            metrics,
            data,
        }
    }

    fn metrica(name: &str, value: f64) -> CheckMetric {
        CheckMetric {
            name: name.into(),
            value,
            unit: "ms".into(),
        }
    }

    #[test]
    fn latency_tem_precedencia_sobre_response_time() {
        let facts = build(
            "http",
            &resultado(
                vec![metrica("latency", 12.0), metrica("response_time", 99.0)],
                json!({}),
            ),
        );
        assert_eq!(facts[fields::LATENCY_MS], json!(12.0));
    }

    #[test]
    fn response_time_preenche_latency_quando_e_a_unica_medida() {
        let facts = build(
            "http",
            &resultado(vec![metrica("response_time", 99.0)], json!({})),
        );
        assert_eq!(facts[fields::LATENCY_MS], json!(99.0));
    }

    #[test]
    fn latency_ausente_e_null_explicito_e_nao_chave_faltando() {
        let facts = build("tcp", &resultado(vec![], json!({})));
        assert_eq!(facts.get(fields::LATENCY_MS), Some(&Value::Null));
    }

    #[test]
    fn campos_extras_do_checker_entram_sem_sobrescrever() {
        let facts = build(
            "http",
            &resultado(
                vec![metrica("status_code", 503.0)],
                json!({ "statusCode": 200, "redirects": 2 }),
            ),
        );
        // A métrica já ocupou `statusCode`: o `data` não a sobrepõe.
        assert_eq!(facts[fields::STATUS_CODE], json!(503.0));
        assert_eq!(facts["redirects"], json!(2));
    }

    #[test]
    fn metrica_desconhecida_e_ignorada() {
        let facts = build("ping", &resultado(vec![metrica("jitter", 3.0)], json!({})));
        assert!(!facts.contains_key("jitter"));
    }

    #[test]
    fn publica_status_duracao_e_tipo() {
        let facts = build("ping", &resultado(vec![], json!({})));
        assert_eq!(facts[fields::STATUS], json!("up"));
        assert_eq!(facts[fields::DURATION_MS], json!(42));
        assert_eq!(facts["type"], json!("ping"));
        assert_eq!(facts["success"], json!(true));
    }
}
