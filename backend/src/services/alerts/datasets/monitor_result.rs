//! Traduz o `CheckResult` de um monitor para o vocabulário das regras (§8.7).

use serde_json::{json, Value};

use crate::services::{
    alerts::{baseline, contracts::AlertDataset, fields},
    monitoring::contracts::CheckResult,
};

/// Nomes das métricas produzidas pelos checkers → chaves usadas nas condições.
/// Mantém o vocabulário da UI alinhado ao avaliador.
/// A tradução acontece **num único ponto**, e é de propósito: `metrics.name`
/// (a série persistida) e `condition.field` (o vocabulário da regra) são duas
/// camadas com convenções diferentes, e espalhar a conversão faria as duas
/// divergirem no primeiro nome novo.
const METRIC_FIELD_MAP: [(&str, &str); 16] = [
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
    // Saúde do equipamento. Mesmos nomes de série que o SNMP já grava, então
    // uma regra de CPU vale igualmente para o servidor e para o roteador.
    ("cpu_usage", fields::CPU_USAGE_PERCENT),
    ("memory_usage", fields::MEMORY_USED_PERCENT),
    ("storage_usage", fields::STORAGE_USED_PERCENT),
    ("load_average_1m", fields::LOAD_AVERAGE_1M),
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
pub fn build(
    monitor_type: &str,
    result: &CheckResult,
    baseline: &baseline::MonitorBaseline,
) -> AlertDataset {
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

    let latency_ms = dataset
        .get(fields::LATENCY_MS)
        .and_then(|value| value.as_f64());
    let packet_loss = dataset
        .get(fields::PACKET_LOSS)
        .and_then(|value| value.as_f64());
    let status = dataset.get(fields::STATUS).and_then(|value| value.as_str());
    let uptime_percent = status.map(|value| if value == "up" { 100.0 } else { 0.0 });

    let enriched = baseline::with_current_value(baseline, latency_ms, packet_loss, uptime_percent);
    if let Some(value) = enriched.latency_baseline_ms {
        dataset.insert(fields::LATENCY_BASELINE_MS.into(), json!(value));
    }
    if let Some(value) = enriched.latency_stddev_ms {
        dataset.insert(fields::LATENCY_STDDEV_MS.into(), json!(value));
    }
    if let Some(value) = enriched.latency_deviation_percent {
        dataset.insert(fields::LATENCY_DEVIATION_PERCENT.into(), json!(value));
    }
    if let Some(value) = enriched.latency_z_score {
        dataset.insert(fields::LATENCY_Z_SCORE.into(), json!(value));
    }
    if let Some(value) = enriched.latency_upper_band_ms {
        dataset.insert(fields::LATENCY_UPPER_BAND_MS.into(), json!(value));
    }
    if let Some(value) = enriched.packet_loss_baseline_percent {
        dataset.insert(fields::PACKET_LOSS_BASELINE_PERCENT.into(), json!(value));
    }
    if let Some(value) = enriched.packet_loss_stddev_percent {
        dataset.insert(fields::PACKET_LOSS_STDDEV_PERCENT.into(), json!(value));
    }
    if let Some(value) = enriched.packet_loss_deviation_percent {
        dataset.insert(fields::PACKET_LOSS_DEVIATION_PERCENT.into(), json!(value));
    }
    if let Some(value) = enriched.packet_loss_z_score {
        dataset.insert(fields::PACKET_LOSS_Z_SCORE.into(), json!(value));
    }
    if let Some(value) = enriched.packet_loss_upper_band_percent {
        dataset.insert(fields::PACKET_LOSS_UPPER_BAND_PERCENT.into(), json!(value));
    }
    if let Some(value) = enriched.uptime_baseline_percent {
        dataset.insert(fields::UPTIME_BASELINE_PERCENT.into(), json!(value));
    }
    if let Some(value) = enriched.uptime_stddev_percent {
        dataset.insert(fields::UPTIME_STDDEV_PERCENT.into(), json!(value));
    }
    if let Some(value) = enriched.uptime_deviation_percent {
        dataset.insert(fields::UPTIME_DEVIATION_PERCENT.into(), json!(value));
    }
    if let Some(value) = enriched.uptime_z_score {
        dataset.insert(fields::UPTIME_Z_SCORE.into(), json!(value));
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

    fn sem_baseline() -> baseline::MonitorBaseline {
        baseline::MonitorBaseline::default()
    }

    #[test]
    fn latency_tem_precedencia_sobre_response_time() {
        let facts = build(
            "http",
            &resultado(
                vec![metrica("latency", 12.0), metrica("response_time", 99.0)],
                json!({}),
            ),
            &sem_baseline(),
        );
        assert_eq!(facts[fields::LATENCY_MS], json!(12.0));
    }

    #[test]
    fn response_time_preenche_latency_quando_e_a_unica_medida() {
        let facts = build(
            "http",
            &resultado(vec![metrica("response_time", 99.0)], json!({})),
            &sem_baseline(),
        );
        assert_eq!(facts[fields::LATENCY_MS], json!(99.0));
    }

    #[test]
    fn latency_ausente_e_null_explicito_e_nao_chave_faltando() {
        let facts = build("tcp", &resultado(vec![], json!({})), &sem_baseline());
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
            &sem_baseline(),
        );
        // A métrica já ocupou `statusCode`: o `data` não a sobrepõe.
        assert_eq!(facts[fields::STATUS_CODE], json!(503.0));
        assert_eq!(facts["redirects"], json!(2));
    }

    #[test]
    fn metrica_desconhecida_e_ignorada() {
        let facts = build(
            "ping",
            &resultado(vec![metrica("jitter", 3.0)], json!({})),
            &sem_baseline(),
        );
        assert!(!facts.contains_key("jitter"));
    }

    #[test]
    fn publica_status_duracao_e_tipo() {
        let facts = build("ping", &resultado(vec![], json!({})), &sem_baseline());
        assert_eq!(facts[fields::STATUS], json!("up"));
        assert_eq!(facts[fields::DURATION_MS], json!(42));
        assert_eq!(facts["type"], json!("ping"));
        assert_eq!(facts["success"], json!(true));
    }

    #[test]
    fn baseline_e_enriquecida_no_dataset() {
        let baseline = baseline::MonitorBaseline {
            latency_baseline_ms: Some(100.0),
            latency_stddev_ms: Some(10.0),
            latency_upper_band_ms: Some(130.0),
            latency_lower_band_ms: Some(70.0),
            packet_loss_baseline_percent: Some(2.0),
            packet_loss_stddev_percent: Some(1.0),
            packet_loss_upper_band_percent: Some(5.0),
            uptime_baseline_percent: Some(99.9),
            uptime_stddev_percent: Some(0.5),
            ..Default::default()
        };
        let facts = build(
            "ping",
            &resultado(
                vec![metrica("latency", 150.0), metrica("packet_loss", 5.0)],
                json!({}),
            ),
            &baseline,
        );
        assert_eq!(facts[fields::LATENCY_BASELINE_MS], json!(100.0));
        assert_eq!(facts[fields::LATENCY_STDDEV_MS], json!(10.0));
        assert_eq!(facts[fields::LATENCY_DEVIATION_PERCENT], json!(50.0));
        assert_eq!(facts[fields::LATENCY_Z_SCORE], json!(5.0));
        assert_eq!(facts[fields::LATENCY_UPPER_BAND_MS], json!(130.0));
        assert_eq!(facts[fields::PACKET_LOSS_BASELINE_PERCENT], json!(2.0));
        assert_eq!(facts[fields::PACKET_LOSS_STDDEV_PERCENT], json!(1.0));
        assert_eq!(facts[fields::PACKET_LOSS_DEVIATION_PERCENT], json!(3.0));
        assert_eq!(facts[fields::PACKET_LOSS_Z_SCORE], json!(3.0));
        assert_eq!(facts[fields::PACKET_LOSS_UPPER_BAND_PERCENT], json!(5.0));
        assert_eq!(facts[fields::UPTIME_BASELINE_PERCENT], json!(99.9));
        assert_eq!(facts[fields::UPTIME_STDDEV_PERCENT], json!(0.5));
        assert_eq!(facts[fields::UPTIME_DEVIATION_PERCENT], json!(0.0));
        assert_eq!(facts[fields::UPTIME_Z_SCORE], json!(0.0));
    }
}
