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
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};

use crate::{
    models::_entities::{
        alert_events, audit_logs, discovery_runs, event_outbox, metrics, monitor_results,
        notification_outbox, plugin_runs, scheduled_task_runs,
    },
    services::{alerts::contracts::AlertStatus, shared::errors::AppResult, telemetry},
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
/// Um ano da trilha de auditoria — quem entrou, quem mudou o quê. É o prazo
/// que uma revisão de acesso costuma pedir, e a tabela cresce por ação de
/// gente, não por ciclo.
pub const DEFAULT_RETENTION_AUDIT_DAYS: i64 = 365;
/// Execuções de plugin terminadas, com a transcrição do acesso ao
/// equipamento: o mesmo prazo do histórico de incidentes.
pub const DEFAULT_RETENTION_PLUGIN_RUNS_DAYS: i64 = 90;
/// Testes de enlace terminados (com pernas e amostras): o mesmo prazo das
/// execuções de plugin que cada etapa gerou.
pub const DEFAULT_RETENTION_LINK_TESTS_DAYS: i64 = 90;
/// Execuções de tarefas agendadas: uma tarefa de minuto em minuto grava
/// ~43 mil por mês, então o prazo é o das métricas, não o dos incidentes.
pub const DEFAULT_RETENTION_SCHEDULED_TASK_RUNS_DAYS: i64 = 30;
const DELETE_BATCH_SIZE: u64 = 10_000;
/// Folga entre lotes. No SQLite há um escritor só: sem ela, a purga de uma
/// tabela grande segura o lock em sequência e a coleta, o syslog e as telas
/// esperam até estourar o `busy_timeout`.
const BATCH_PAUSE: std::time::Duration = std::time::Duration::from_millis(100);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneStats {
    pub outbox_deleted: u64,
    pub results_deleted: u64,
    pub metrics_deleted: u64,
    pub discovery_deleted: u64,
    pub alert_events_deleted: u64,
    pub notifications_deleted: u64,
    /// Histórico de 1 minuto de host e containers (ADR 011).
    pub telemetry_deleted: u64,
    pub audit_deleted: u64,
    pub plugin_runs_deleted: u64,
    pub link_tests_deleted: u64,
    pub scheduled_task_runs_deleted: u64,
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
            + self.telemetry_deleted
            + self.audit_deleted
            + self.plugin_runs_deleted
            + self.link_tests_deleted
            + self.scheduled_task_runs_deleted
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
    let cutoff =
        |variable: &str, default: i64| now - Duration::days(retention_days(variable, default));

    let outbox_deleted = delete_in_batches::<event_outbox::Entity, _>(
        db,
        event_outbox::Column::Id,
        Condition::all().add(
            event_outbox::Column::CreatedAt.lt(now - Duration::minutes(OUTBOX_RETENTION_MINUTES)),
        ),
    )
    .await?;

    let results_deleted = delete_in_batches::<monitor_results::Entity, _>(
        db,
        monitor_results::Column::Id,
        Condition::all().add(monitor_results::Column::CreatedAt.lt(cutoff(
            "RETENTION_MONITOR_RESULTS_DAYS",
            DEFAULT_RETENTION_MONITOR_RESULTS_DAYS,
        ))),
    )
    .await?;

    let metrics_cutoff = cutoff("RETENTION_METRICS_DAYS", DEFAULT_RETENTION_METRICS_DAYS);
    let metrics_deleted = delete_in_batches::<metrics::Entity, _>(
        db,
        metrics::Column::Id,
        Condition::all().add(metrics::Column::CreatedAt.lt(metrics_cutoff)),
    )
    .await?;
    // Mesma janela das métricas SNMP: é a mesma natureza de dado.
    let telemetry_deleted = telemetry::store::prune(db, metrics_cutoff).await?;

    let discovery_deleted = delete_in_batches::<discovery_runs::Entity, _>(
        db,
        discovery_runs::Column::Id,
        Condition::all().add(discovery_runs::Column::CreatedAt.lt(cutoff(
            "RETENTION_DISCOVERY_DAYS",
            DEFAULT_RETENTION_DISCOVERY_DAYS,
        ))),
    )
    .await?;

    // Só episódio **fechado** envelhece: um alerta ainda aberto é o que o
    // operador precisa ver na Central, por mais antigo que seja.
    let alert_events_cutoff = cutoff(
        "RETENTION_ALERT_EVENTS_DAYS",
        DEFAULT_RETENTION_ALERT_EVENTS_DAYS,
    );
    let alert_events_deleted = delete_in_batches::<alert_events::Entity, _>(
        db,
        alert_events::Column::Id,
        Condition::all()
            .add(alert_events::Column::Status.eq(AlertStatus::Resolved))
            .add(
                // `resolved_at` pode faltar em linha antiga fechada antes de a
                // coluna ser preenchida: a abertura serve de âncora.
                Condition::any()
                    .add(alert_events::Column::ResolvedAt.lt(alert_events_cutoff))
                    .add(
                        Condition::all()
                            .add(alert_events::Column::ResolvedAt.is_null())
                            .add(alert_events::Column::StartedAt.lt(alert_events_cutoff)),
                    ),
            ),
    )
    .await?;

    // Idem para o diário: linha ainda pendente é notificação por entregar.
    let notifications_deleted = delete_in_batches::<notification_outbox::Entity, _>(
        db,
        notification_outbox::Column::Id,
        Condition::all()
            .add(notification_outbox::Column::Status.ne("pending"))
            .add(notification_outbox::Column::CreatedAt.lt(cutoff(
                "RETENTION_NOTIFICATIONS_DAYS",
                DEFAULT_RETENTION_NOTIFICATIONS_DAYS,
            ))),
    )
    .await?;

    let audit_deleted = delete_in_batches::<audit_logs::Entity, _>(
        db,
        audit_logs::Column::Id,
        Condition::all().add(
            audit_logs::Column::CreatedAt
                .lt(cutoff("RETENTION_AUDIT_DAYS", DEFAULT_RETENTION_AUDIT_DAYS)),
        ),
    )
    .await?;

    // Execução em curso não tem `finished_at` e nunca envelhece.
    let plugin_runs_deleted = delete_in_batches::<plugin_runs::Entity, _>(
        db,
        plugin_runs::Column::Id,
        Condition::all().add(plugin_runs::Column::FinishedAt.lt(cutoff(
            "RETENTION_PLUGIN_RUNS_DAYS",
            DEFAULT_RETENTION_PLUGIN_RUNS_DAYS,
        ))),
    )
    .await?;

    // Teste em curso não tem `finished_at` e nunca envelhece.
    let link_tests_deleted = crate::services::link_tests::repository::prune(
        db,
        cutoff(
            "RETENTION_LINK_TESTS_DAYS",
            DEFAULT_RETENTION_LINK_TESTS_DAYS,
        ),
    )
    .await?;

    // Execução em curso não tem `finished_at` e nunca envelhece.
    let scheduled_task_runs_deleted = delete_in_batches::<scheduled_task_runs::Entity, _>(
        db,
        scheduled_task_runs::Column::Id,
        Condition::all().add(scheduled_task_runs::Column::FinishedAt.lt(cutoff(
            "RETENTION_SCHEDULED_TASK_RUNS_DAYS",
            DEFAULT_RETENTION_SCHEDULED_TASK_RUNS_DAYS,
        ))),
    )
    .await?;

    Ok(PruneStats {
        outbox_deleted,
        results_deleted,
        metrics_deleted,
        discovery_deleted,
        alert_events_deleted,
        notifications_deleted,
        telemetry_deleted,
        audit_deleted,
        plugin_runs_deleted,
        link_tests_deleted,
        scheduled_task_runs_deleted,
    })
}

/// Apaga, em lotes por `id`, as linhas que `filter` seleciona.
///
/// Um `DELETE` único de milhões de linhas seguraria o escritor do SQLite do
/// começo ao fim; em lotes, cada um é uma transação curta e a pausa entre eles
/// devolve a vez a quem está gravando.
async fn delete_in_batches<E, C>(db: &C, id: E::Column, filter: Condition) -> AppResult<u64>
where
    E: EntityTrait,
    C: ConnectionTrait,
{
    let mut deleted = 0;
    loop {
        let ids = E::find()
            .select_only()
            .column(id)
            .filter(filter.clone())
            .order_by_asc(id)
            .limit(DELETE_BATCH_SIZE)
            .into_tuple::<i64>()
            .all(db)
            .await?;
        if ids.is_empty() {
            return Ok(deleted);
        }
        let last_batch = (ids.len() as u64) < DELETE_BATCH_SIZE;
        deleted += E::delete_many()
            .filter(id.is_in(ids))
            .exec(db)
            .await?
            .rows_affected;
        if last_batch {
            return Ok(deleted);
        }
        tokio::time::sleep(BATCH_PAUSE).await;
    }
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
    fn o_total_soma_todas_as_origens() {
        let stats = PruneStats {
            outbox_deleted: 1,
            results_deleted: 2,
            metrics_deleted: 3,
            discovery_deleted: 4,
            alert_events_deleted: 5,
            notifications_deleted: 6,
            telemetry_deleted: 7,
            audit_deleted: 8,
            plugin_runs_deleted: 9,
            link_tests_deleted: 10,
            scheduled_task_runs_deleted: 11,
        };
        assert_eq!(stats.total(), 66);
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
