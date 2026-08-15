//! Purga de dados antigos (§8.13), executada de hora em hora pelo scheduler.
//!
//! O que é apagado aqui é histórico técnico com valor decrescente: o outbox de
//! eventos só serve ao relay dos últimos minutos, e resultados/métricas de
//! meses atrás não são consultados por tela nenhuma. Sem isso, `metrics` e
//! `monitor_results` crescem sem teto — são séries temporais gravadas a cada
//! ciclo de cada monitor.
//!
//! A Fase 4 do roadmap de alertas inteligentes acrescentou `alert_events` e
//! `notification_outbox` à lista. As duas cresciam sem teto pelo mesmo motivo
//! das outras, com um cuidado a mais: **evento aberto nunca é apagado**. Só o
//! que já foi resolvido envelhece — purgar um episódio em curso apagaria da
//! Central um alerta que o operador ainda precisa ver.

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter};

use crate::{
    models::{
        _entities::{
            alert_events as alert_events_entity, discovery_runs as discovery_runs_entity,
            event_outbox as event_outbox_entity, metrics as metrics_entity,
            monitor_results as monitor_results_entity,
        },
        alert_events, discovery_runs, event_outbox, metrics, monitor_results, notification_outbox,
    },
    services::{alerts::contracts::AlertStatus, shared::errors::AppResult},
};

/// O outbox é buffer de retransmissão, não histórico: o relay lê os últimos
/// segundos. Meia hora é folga de sobra para um processo que reiniciou.
pub const OUTBOX_RETENTION_MINUTES: i64 = 30;

pub const DEFAULT_RETENTION_MONITOR_RESULTS_DAYS: i64 = 14;
pub const DEFAULT_RETENTION_METRICS_DAYS: i64 = 30;
pub const DEFAULT_RETENTION_DISCOVERY_DAYS: i64 = 7;
/// Três meses de episódios resolvidos. É bem mais generoso que as séries
/// temporais de propósito: `alert_events` é o histórico de **incidentes**, a
/// tabela que responde "quantas vezes este link caiu neste trimestre" — e ela
/// cresce por evento, não por ciclo de checagem.
pub const DEFAULT_RETENTION_ALERT_EVENTS_DAYS: i64 = 90;
/// O diário de notificações acompanha as métricas: passado esse prazo ele já não
/// alimenta cooldown, agrupamento nem a pergunta "por que não fui avisado?".
pub const DEFAULT_RETENTION_NOTIFICATIONS_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneStats {
    pub outbox_deleted: u64,
    pub results_deleted: u64,
    pub metrics_deleted: u64,
    pub discovery_deleted: u64,
    pub alert_events_deleted: u64,
    pub notifications_deleted: u64,
}

impl PruneStats {
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.outbox_deleted
            + self.results_deleted
            + self.metrics_deleted
            + self.discovery_deleted
            + self.alert_events_deleted
            + self.notifications_deleted
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

    // Só episódio **fechado** envelhece: um alerta ainda aberto é o que o
    // operador precisa ver na Central, por mais antigo que seja.
    let alert_events_cutoff = now
        - Duration::days(retention_days(
            "RETENTION_ALERT_EVENTS_DAYS",
            DEFAULT_RETENTION_ALERT_EVENTS_DAYS,
        ));
    let alert_events_deleted = alert_events::Entity::delete_many()
        .filter(alert_events_entity::Column::Status.eq(AlertStatus::Resolved))
        .filter(
            // `resolved_at` pode faltar em linha antiga fechada antes de a
            // coluna ser preenchida: a abertura serve de âncora.
            Condition::any()
                .add(alert_events_entity::Column::ResolvedAt.lt(alert_events_cutoff))
                .add(
                    Condition::all()
                        .add(alert_events_entity::Column::ResolvedAt.is_null())
                        .add(alert_events_entity::Column::StartedAt.lt(alert_events_cutoff)),
                ),
        )
        .exec(db)
        .await?
        .rows_affected;

    // Idem para o diário: linha ainda pendente é notificação por entregar.
    let notifications_cutoff = now
        - Duration::days(retention_days(
            "RETENTION_NOTIFICATIONS_DAYS",
            DEFAULT_RETENTION_NOTIFICATIONS_DAYS,
        ));
    let notifications_deleted = notification_outbox::Entity::delete_many()
        .filter(notification_outbox::Column::Status.ne("pending"))
        .filter(notification_outbox::Column::CreatedAt.lt(notifications_cutoff))
        .exec(db)
        .await?
        .rows_affected;

    Ok(PruneStats {
        outbox_deleted,
        results_deleted,
        metrics_deleted,
        discovery_deleted,
        alert_events_deleted,
        notifications_deleted,
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
    fn o_total_soma_as_seis_tabelas() {
        let stats = PruneStats {
            outbox_deleted: 1,
            results_deleted: 2,
            metrics_deleted: 3,
            discovery_deleted: 4,
            alert_events_deleted: 5,
            notifications_deleted: 6,
        };
        assert_eq!(stats.total(), 21);
        assert_eq!(PruneStats::default().total(), 0);
    }

    #[test]
    fn o_historico_de_incidentes_e_guardado_por_muito_mais_tempo() {
        // `alert_events` cresce por episódio, não por ciclo: 90 dias respondem
        // "quantas vezes este link caiu neste trimestre" sem inchar o banco.
        assert_eq!(DEFAULT_RETENTION_ALERT_EVENTS_DAYS, 90);
        let series = [
            DEFAULT_RETENTION_MONITOR_RESULTS_DAYS,
            DEFAULT_RETENTION_METRICS_DAYS,
            DEFAULT_RETENTION_DISCOVERY_DAYS,
            DEFAULT_RETENTION_NOTIFICATIONS_DAYS,
        ];
        assert!(
            series
                .iter()
                .all(|days| *days < DEFAULT_RETENTION_ALERT_EVENTS_DAYS),
            "incidente tem de sobreviver a toda série temporal"
        );
    }
}
