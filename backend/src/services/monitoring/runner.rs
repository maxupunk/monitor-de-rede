//! Seleção de checker e aplicação do timeout definido no monitor.
//!
//! Os tipos do núcleo estão no `match`; os das áreas (container, por
//! exemplo) vêm do registro de `MonitorChecker`s que o ponto de montagem
//! preenche (ADR 017). O motor não conhece área nenhuma.

use loco_rs::app::AppContext;
use netmonitor_core::monitoring::parse_config;
use serde_json::Value;

use crate::services::{
    monitoring::{
        checkers::{
            dns::{DnsChecker, DnsConfig},
            host_resources::{HostResourcesChecker, HostResourcesConfig},
            http::{HttpChecker, HttpConfig},
            ping::{PingChecker, PingConfig},
            snmp::{SnmpChecker, SnmpCheckerConfig},
            system_health::{SystemHealthChecker, SystemHealthConfig},
            tcp::{TcpChecker, TcpConfig},
        },
        contracts::{CheckResult, Checker},
        managed::SYSTEM_HEALTH,
        ping_diagnostics,
    },
    shared::errors::{AppError, AppResult},
};

/// Opções que pertencem ao monitor, e não ao JSON específico do checker.
#[derive(Debug, Default, Clone, Copy)]
pub struct RunOptions {
    pub timeout_ms: Option<u64>,
}

/// Impõe o `timeoutMs` automático calculado para o monitor.
#[must_use]
pub fn merge_timeout(config: &Value, timeout_ms: Option<u64>) -> Value {
    let mut merged = config.clone();
    let Some(timeout_ms) = timeout_ms.filter(|value| *value > 0) else {
        return merged;
    };
    if let Value::Object(ref mut map) = merged {
        map.insert("timeoutMs".to_string(), Value::from(timeout_ms));
    }
    merged
}

/// As dependências de processo dos checkers moram no núcleo (a descoberta
/// da área Rede também as usa); o caminho antigo continua valendo.
pub use netmonitor_core::monitoring::CheckDeps;

/// Executa o checker do tipo informado sem deixar regra de negócio no controller.
pub async fn run_monitor(
    ctx: &AppContext,
    kind: &str,
    configuration: &Value,
    options: RunOptions,
) -> AppResult<CheckResult> {
    run_monitor_with(&CheckDeps::from_context(ctx), kind, configuration, options).await
}

/// Mesma execução de [`run_monitor`], para quem não tem `AppContext` (agente).
pub async fn run_monitor_with(
    deps: &CheckDeps,
    kind: &str,
    configuration: &Value,
    options: RunOptions,
) -> AppResult<CheckResult> {
    let configuration = merge_timeout(configuration, options.timeout_ms);
    let result = match kind.to_lowercase().as_str() {
        "ping" => {
            ping_diagnostics::execute_ping(
                &PingChecker::new(deps.ping_client()?),
                parse_config::<PingConfig>(&configuration, "ping")?,
            )
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
        // Recursos do host onde o monitor roda: a central, ou o agente do
        // servidor remoto quando o monitor pertence a ele (ADR 011).
        "host_resources" => {
            HostResourcesChecker
                .execute(parse_config::<HostResourcesConfig>(
                    &configuration,
                    "host_resources",
                )?)
                .await
        }
        "snmp" => {
            SnmpChecker
                .execute(parse_config::<SnmpCheckerConfig>(&configuration, "snmp")?)
                .await
        }
        // Sem este ramo o agendador devolveria "tipo de monitor desconhecido"
        // a cada ciclo do monitor gerenciado do servidor — e o dispositivo
        // ficaria eternamente em `unknown`.
        SYSTEM_HEALTH => {
            SystemHealthChecker
                .execute(parse_config::<SystemHealthConfig>(
                    &configuration,
                    SYSTEM_HEALTH,
                )?)
                .await
        }
        // Tipos das áreas (container, na área Servidores).
        other => {
            let Some(checker) = deps.checkers.find(other) else {
                return Err(AppError::business_rule(format!(
                    "Tipo de monitor desconhecido ou não suportado: {kind}"
                )));
            };
            checker.check(&configuration).await?
        }
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_automatico_substitui_valor_salvo_anteriormente() {
        assert_eq!(
            merge_timeout(&serde_json::json!({ "timeoutMs": 250 }), Some(1_000))["timeoutMs"],
            1_000
        );
        assert_eq!(
            merge_timeout(&serde_json::json!({}), Some(1_000))["timeoutMs"],
            1_000
        );
    }

    #[tokio::test]
    async fn sem_cliente_icmp_so_o_ping_falha() {
        let deps = CheckDeps::default();
        let ping = run_monitor_with(
            &deps,
            "ping",
            &serde_json::json!({ "host": "127.0.0.1" }),
            RunOptions::default(),
        )
        .await;
        assert!(ping.is_err());

        let tcp = run_monitor_with(
            &deps,
            "tcp",
            &serde_json::json!({ "host": "127.0.0.1", "port": 1 }),
            RunOptions {
                timeout_ms: Some(1_000),
            },
        )
        .await;
        assert!(tcp.is_ok());
    }
}
