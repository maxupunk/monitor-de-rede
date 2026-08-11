//! Purga de dados antigos (§8.13), executada de hora em hora pelo scheduler.
//!
//! O que é apagado aqui é histórico técnico com valor decrescente: o outbox de
//! eventos só serve ao relay dos últimos minutos, e resultados/métricas de
//! meses atrás não são consultados por tela nenhuma. Sem isso, `metrics` e
//! `monitor_results` crescem sem teto — são séries temporais gravadas a cada
//! ciclo de cada monitor.

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::{
    models::{
        _entities::{
            discovery_runs as discovery_runs_entity, event_outbox as event_outbox_entity,
            metrics as metrics_entity, monitor_results as monitor_results_entity,
        },
        discovery_runs, event_outbox, metrics, monitor_results,
    },
    services::shared::errors::AppResult,
};

/// O outbox é buffer de retransmissão, não histórico: o relay lê os últimos
/// segundos. Meia hora é folga de sobra para um processo que reiniciou.
pub const OUTBOX_RETENTION_MINUTES: i64 = 30;

pub const DEFAULT_RETENTION_MONITOR_RESULTS_DAYS: i64 = 14;
pub const DEFAULT_RETENTION_METRICS_DAYS: i64 = 30;
pub const DEFAULT_RETENTION_DISCOVERY_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneStats {
    pub outbox_deleted: u64,
    pub results_deleted: u64,
    pub metrics_deleted: u64,
    pub discovery_deleted: u64,
}

impl PruneStats {
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.outbox_deleted + self.results_deleted + self.metrics_deleted + self.discovery_deleted
    }
}

/// Lê uma variável de retenção, caindo no padrão quando ausente ou inválida.
///
/// Valor `0` ou negativo é tratado como "use o padrão": desligar a purga por
/// engano encheria o disco em silêncio, que é pior do que ignorar a
/// configuração.
#[must_use]
pub fn retention_days(variable: &str, default: i64) -> i64 {
    std::env::var(variable)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

/// Apaga tudo que passou da janela de retenção.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn prune_all<C: ConnectionTrait>(db: &C) -> AppResult<PruneStats> {
    let now = Utc::now();

    let outbox_deleted = event_outbox::Entity::delete_many()
        .filter(
            event_outbox_entity::Column::CreatedAt
                .lt(now - Duration::minutes(OUTBOX_RETENTION_MINUTES)),
        )
        .exec(db)
        .await?
        .rows_affected;

    let results_cutoff = now
        - Duration::days(retention_days(
            "RETENTION_MONITOR_RESULTS_DAYS",
            DEFAULT_RETENTION_MONITOR_RESULTS_DAYS,
        ));
    let results_deleted = monitor_results::Entity::delete_many()
        .filter(monitor_results_entity::Column::CreatedAt.lt(results_cutoff))
        .exec(db)
        .await?
        .rows_affected;

    let metrics_cutoff = now
        - Duration::days(retention_days(
            "RETENTION_METRICS_DAYS",
            DEFAULT_RETENTION_METRICS_DAYS,
        ));
    let metrics_deleted = metrics::Entity::delete_many()
        .filter(metrics_entity::Column::CreatedAt.lt(metrics_cutoff))
        .exec(db)
        .await?
        .rows_affected;

    let discovery_cutoff = now
        - Duration::days(retention_days(
            "RETENTION_DISCOVERY_DAYS",
            DEFAULT_RETENTION_DISCOVERY_DAYS,
        ));
    let discovery_deleted = discovery_runs::Entity::delete_many()
        .filter(discovery_runs_entity::Column::CreatedAt.lt(discovery_cutoff))
        .exec(db)
        .await?
        .rows_affected;

    Ok(PruneStats {
        outbox_deleted,
        results_deleted,
        metrics_deleted,
        discovery_deleted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn a_retencao_vem_do_ambiente_com_padrao() {
        std::env::remove_var("RETENTION_METRICS_DAYS");
        assert_eq!(
            retention_days("RETENTION_METRICS_DAYS", DEFAULT_RETENTION_METRICS_DAYS),
            30
        );
        std::env::set_var("RETENTION_METRICS_DAYS", "7");
        assert_eq!(retention_days("RETENTION_METRICS_DAYS", 30), 7);
        std::env::remove_var("RETENTION_METRICS_DAYS");
    }

    #[test]
    #[serial]
    fn valor_invalido_ou_zerado_nao_desliga_a_purga() {
        for invalido in ["0", "-1", "sempre", ""] {
            std::env::set_var("RETENTION_METRICS_DAYS", invalido);
            assert_eq!(
                retention_days("RETENTION_METRICS_DAYS", 30),
                30,
                "aceitou {invalido:?}"
            );
        }
        std::env::remove_var("RETENTION_METRICS_DAYS");
    }

    #[test]
    fn o_total_soma_as_quatro_tabelas() {
        let stats = PruneStats {
            outbox_deleted: 1,
            results_deleted: 2,
            metrics_deleted: 3,
            discovery_deleted: 4,
        };
        assert_eq!(stats.total(), 10);
        assert_eq!(PruneStats::default().total(), 0);
    }
}
