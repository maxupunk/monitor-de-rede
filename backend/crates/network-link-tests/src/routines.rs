//! Os testes de enlace no ciclo do scheduler e na purga (registro de rotinas
//! do núcleo).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use netmonitor_core::{
    areas::{self, Area},
    routines::{cadence::is_due, retention_days, Cycle, RetentionJob, Routine},
    shared::errors::AppResult,
};
use sea_orm::DatabaseConnection;

use crate::{repository as link_test_repository, schedules as link_test_schedules, LinkTestEnv};

/// Cadência da conferência dos agendamentos de teste de enlace.
pub const LINK_TEST_SCHEDULES_INTERVAL_SECONDS: i64 = 60;

/// Testes de enlace terminados (com pernas e amostras): o mesmo prazo das
/// execuções de plugin que cada etapa gerou.
pub const DEFAULT_RETENTION_LINK_TESTS_DAYS: i64 = 90;

/// Dispara os agendamentos de teste de enlace vencidos (ADR 015 §5).
///
/// Confere a cada minuto; o teste em si roda em segundo plano, com a mesma
/// trava de concorrência dos testes pedidos pela tela. Com a área Rede
/// desligada (ADR 017) os agendamentos ficam pausados — não são apagados.
pub struct LinkTestSchedules;

#[async_trait]
impl Routine for LinkTestSchedules {
    fn name(&self) -> &'static str {
        "agendamentos_de_enlace"
    }

    fn area(&self) -> Area {
        Area::Network
    }

    async fn run(&self, ctx: &AppContext, cycle: Cycle) -> AppResult<()> {
        if !is_due(
            "link_test_schedules",
            LINK_TEST_SCHEDULES_INTERVAL_SECONDS,
            cycle.now,
        ) {
            return Ok(());
        }
        if !areas::is_enabled(&ctx.db, Area::Network).await? {
            return Ok(());
        }
        let started =
            link_test_schedules::run_due(LinkTestEnv::from_context(ctx), cycle.now).await?;
        if started > 0 {
            tracing::info!(started, "agendamentos de teste de enlace disparados");
        }
        Ok(())
    }
}

/// O histórico dos testes de enlace. Teste em curso não tem `finished_at` e
/// nunca envelhece.
pub struct LinkTestRetention;

#[async_trait]
impl RetentionJob for LinkTestRetention {
    fn name(&self) -> &'static str {
        "testes_de_enlace"
    }

    fn area(&self) -> Area {
        Area::Network
    }

    async fn prune(&self, db: &DatabaseConnection, now: DateTime<Utc>) -> AppResult<u64> {
        let days = retention_days(
            "RETENTION_LINK_TESTS_DAYS",
            DEFAULT_RETENTION_LINK_TESTS_DAYS,
        );
        link_test_repository::prune(db, now - chrono::Duration::days(days)).await
    }
}

/// As rotinas deste submódulo.
#[must_use]
pub fn routines() -> Vec<Arc<dyn Routine>> {
    vec![Arc::new(LinkTestSchedules)]
}

/// A retenção deste submódulo.
#[must_use]
pub fn retention() -> Vec<Arc<dyn RetentionJob>> {
    vec![Arc::new(LinkTestRetention)]
}
