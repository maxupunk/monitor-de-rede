//! Seleção de checker e aplicação do timeout definido no monitor.

use loco_rs::app::AppContext;
use serde_json::Value;

use crate::services::{
    monitoring::{
        checkers::{
            dns::{DnsChecker, DnsConfig},
            http::{HttpChecker, HttpConfig},
            ping::{PingChecker, PingConfig},
            tcp::{TcpChecker, TcpConfig},
        },
        contracts::{CheckResult, Checker},
    },
    shared::errors::{AppError, AppResult},
};

/// Opções que pertencem ao monitor, e não ao JSON específico do checker.
#[derive(Debug, Default, Clone, Copy)]
pub struct RunOptions {
    pub timeout_ms: Option<u64>,
}

/// Injeta `timeoutMs` somente quando o checker não recebeu um timeout próprio.
///
/// A prioridade evita que um monitor HTTP com timeout explicitamente ajustado
/// seja silenciosamente sobrescrito pelo valor genérico da linha `monitors`.
#[must_use]
pub fn merge_timeout(config: &Value, timeout_ms: Option<u64>) -> Value {
    let mut merged = config.clone();
    let Some(timeout_ms) = timeout_ms.filter(|value| *value > 0) else {
        return merged;
    };
    if let Value::Object(ref mut map) = merged {
        map.entry("timeoutMs".to_string())
            .or_insert_with(|| Value::from(timeout_ms));
    }
    merged
}

/// Executa o checker do tipo informado sem deixar regra de negócio no controller.
pub async fn run_monitor(
    ctx: &AppContext,
    kind: &str,
    configuration: &Value,
    options: RunOptions,
) -> AppResult<CheckResult> {
    let configuration = merge_timeout(configuration, options.timeout_ms);
    let result = match kind.to_lowercase().as_str() {
        "ping" => {
            PingChecker::from_context(ctx)?
                .execute(parse_config::<PingConfig>(&configuration, "ping")?)
                .await
        }
        "tcp" => {
            TcpChecker
                .execute(parse_config::<TcpConfig>(&configuration, "tcp")?)
                .await
        }
        "http" | "https" => {
            HttpChecker
                .execute(parse_config::<HttpConfig>(&configuration, "http")?)
                .await
        }
        "dns" => {
            DnsChecker
                .execute(parse_config::<DnsConfig>(&configuration, "dns")?)
                .await
        }
        "snmp" => {
            return Err(AppError::business_rule(format!(
                "Tipo de monitor ainda não disponível nesta fase: {kind}"
            )))
        }
        _ => {
            return Err(AppError::business_rule(format!(
                "Tipo de monitor desconhecido ou não suportado: {kind}"
            )))
        }
    };
    Ok(result)
}

fn parse_config<T: serde::de::DeserializeOwned>(configuration: &Value, name: &str) -> AppResult<T> {
    serde_json::from_value(configuration.clone()).map_err(|error| {
        AppError::validation(format!(
            "Configuração inválida para monitor {name}: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_do_checker_tem_precedencia() {
        assert_eq!(
            merge_timeout(&serde_json::json!({ "timeoutMs": 250 }), Some(1_000))["timeoutMs"],
            250
        );
        assert_eq!(
            merge_timeout(&serde_json::json!({}), Some(1_000))["timeoutMs"],
            1_000
        );
    }
}
