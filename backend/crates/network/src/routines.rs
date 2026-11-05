//! O que a Rede contribui ao ciclo do scheduler e à purga (ADR 017, registro
//! de rotinas): a coleta SNMP consolidada por dispositivo (um
//! [`MonitorGroup`]), a coleta dos dispositivos sem monitor, a varredura
//! agendada, o vigia dos clientes Wi-Fi, os testes de enlace agendados e a
//! retenção do histórico deles. Montadas pelo ponto de montagem.
//!
//! Área desligada pausa a varredura, o vigia e os agendamentos de enlace — as
//! próprias rotinas consultam a área, como já faziam. A coleta SNMP alimenta
//! monitores e não pausa.

use std::{collections::HashSet, sync::Arc};

use agent_protocol::monitoring::CheckResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{stream, StreamExt};
use loco_rs::app::AppContext;
use netmonitor_core::{
    areas::{self, Area},
    models::{devices, monitors},
    monitoring::{execution_guard::try_acquire_snmp_device, MonitorGroup},
    routines::{
        cadence::{is_device_collection_due, is_due},
        retention_days, Cycle, RetentionJob, Routine,
    },
    shared::errors::{AppError, AppResult},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set,
};

use crate::{
    discovery::queue::{process_pending_runs, schedule_due_networks, spawn_pending_run},
    link_tests::{
        repository as link_test_repository, schedules as link_test_schedules, LinkTestEnv,
    },
    snmp::service as snmp_service,
    wifi,
};

/// Coletas SNMP simultâneas de dispositivos sem monitor (o mesmo teto dos
/// monitores no ciclo).
const COLLECTION_CONCURRENCY: usize = 16;

/// Cadência da conferência dos agendamentos de teste de enlace.
pub const LINK_TEST_SCHEDULES_INTERVAL_SECONDS: i64 = 60;

/// Cadência do vigia dos clientes Wi-Fi vigiados: uma sessão SSH por AP a
/// cada 5 min — leve para o AP e fino o bastante para a regra de 10 min.
pub const WIFI_WATCH_INTERVAL_SECONDS: i64 = 300;

/// Testes de enlace terminados (com pernas e amostras): o mesmo prazo das
/// execuções de plugin que cada etapa gerou.
pub const DEFAULT_RETENTION_LINK_TESTS_DAYS: i64 = 90;

/// Monitores SNMP locais do mesmo dispositivo compartilham uma única coleta.
#[must_use]
pub fn local_snmp_device_id(monitor: &monitors::Model) -> Option<i64> {
    (monitor.r#type == "snmp" && monitor.probe_id.is_none())
        .then_some(monitor.device_id)
        .flatten()
}

/// Executa todos os monitores SNMP locais vinculados a um dispositivo em uma
/// única requisição e devolve o resultado de cada um.
pub async fn execute_snmp_device_group(
    ctx: &AppContext,
    device_id: i64,
    scheduled_at: DateTime<Utc>,
) -> AppResult<Vec<(i64, CheckResult)>> {
    let Some(_guard) = try_acquire_snmp_device(device_id) else {
        tracing::debug!(device_id, "coleta SNMP do dispositivo já está em andamento");
        return Ok(Vec::new());
    };
    let Some(device) = devices::Entity::find_by_id(device_id).one(&ctx.db).await? else {
        return Ok(Vec::new());
    };
    let monitored_items = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(Some(device_id)))
        .filter(monitors::Column::Type.eq("snmp"))
        .filter(monitors::Column::Enabled.eq(true))
        .filter(monitors::Column::ProbeId.is_null())
        .all(&ctx.db)
        .await?;
    let next_run_at = (scheduled_at
        + chrono::Duration::seconds(i64::from(device.snmp_poll_interval_seconds.max(1))))
    .into();
    for monitor in &monitored_items {
        let mut active: monitors::ActiveModel = monitor.clone().into();
        active.next_run_at = Set(Some(next_run_at));
        active.update(&ctx.db).await?;
    }
    Ok(snmp_service::poll_device_monitors(ctx, &device, &monitored_items).await)
}

/// Dispositivos com SNMP ligado que nenhum monitor SNMP local agenda e cuja
/// coleta venceu pelo intervalo do próprio dispositivo.
///
/// O scheduler é o **único** dono da cadência SNMP: quem tem monitor é
/// coletado pelo grupo acima no `next_run_at` dele; o resto, por aqui. Os dois
/// conjuntos são disjuntos, então nenhum equipamento é consultado duas vezes
/// no mesmo intervalo — e o histórico deixa de depender de haver uma aba
/// aberta, como dependia quando o coletor da topologia fazia esse papel.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn due_unmonitored_snmp_devices(
    ctx: &AppContext,
    now: DateTime<Utc>,
) -> AppResult<Vec<devices::Model>> {
    let agendados: HashSet<i64> = monitors::Entity::find()
        .select_only()
        .column(monitors::Column::DeviceId)
        .filter(monitors::Column::Type.eq("snmp"))
        .filter(monitors::Column::Enabled.eq(true))
        .filter(monitors::Column::ProbeId.is_null())
        .filter(monitors::Column::DeviceId.is_not_null())
        .distinct()
        .into_tuple::<Option<i64>>()
        .all(&ctx.db)
        .await?
        .into_iter()
        .flatten()
        .collect();
    let candidatos = devices::Entity::find()
        .filter(devices::Column::SnmpEnabled.eq(true))
        .filter(devices::Column::IsMonitored.eq(true))
        .all(&ctx.db)
        .await?;
    Ok(candidatos
        .into_iter()
        .filter(|device| {
            device
                .ip_address
                .as_deref()
                .is_some_and(|ip| !ip.trim().is_empty())
        })
        .filter(|device| !agendados.contains(&device.id))
        .filter(|device| {
            is_device_collection_due(device.id, i64::from(device.snmp_poll_interval_seconds), now)
        })
        .collect())
}

/// Coleta um dispositivo sem monitor SNMP: interfaces, taxas e sistema, com o
/// mesmo `collect_device` dos monitores.
///
/// # Errors
///
/// Propaga erro da coleta.
pub async fn collect_unmonitored_device(
    ctx: &AppContext,
    device: &devices::Model,
) -> AppResult<()> {
    let Some(_guard) = try_acquire_snmp_device(device.id) else {
        tracing::debug!(
            device_id = device.id,
            "coleta SNMP do dispositivo já está em andamento"
        );
        return Ok(());
    };
    snmp_service::collect_device(ctx, device).await?;
    Ok(())
}

/// Os monitores SNMP locais de um mesmo dispositivo: uma coleta só.
pub struct SnmpDeviceGroup;

#[async_trait]
impl MonitorGroup for SnmpDeviceGroup {
    fn group_of(&self, monitor: &monitors::Model) -> Option<i64> {
        local_snmp_device_id(monitor)
    }

    async fn run(
        &self,
        ctx: &AppContext,
        key: i64,
        scheduled_at: DateTime<Utc>,
    ) -> AppResult<Vec<(i64, CheckResult)>> {
        execute_snmp_device_group(ctx, key, scheduled_at).await
    }

    async fn run_now(
        &self,
        ctx: &AppContext,
        monitor: &monitors::Model,
    ) -> Option<AppResult<Vec<(i64, CheckResult)>>> {
        if monitor.r#type != "snmp" {
            return None;
        }
        let device_id = monitor.device_id?;
        Some(poll_device_now(ctx, device_id).await)
    }
}

/// A coleta consolidada de todos os monitores SNMP do dispositivo, pedida pela
/// tela (fora da cadência).
async fn poll_device_now(ctx: &AppContext, device_id: i64) -> AppResult<Vec<(i64, CheckResult)>> {
    let _guard = try_acquire_snmp_device(device_id).ok_or_else(|| {
        AppError::conflict("Já existe uma coleta SNMP em andamento para este dispositivo")
    })?;
    let device = devices::Entity::find_by_id(device_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let related = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(Some(device_id)))
        .filter(monitors::Column::Type.eq("snmp"))
        .all(&ctx.db)
        .await?;
    Ok(snmp_service::poll_device_monitors(ctx, &device, &related).await)
}

/// A coleta dos dispositivos com SNMP ligado que nenhum monitor agenda.
pub struct UnmonitoredSnmpCollection;

#[async_trait]
impl Routine for UnmonitoredSnmpCollection {
    fn name(&self) -> &'static str {
        "coleta_snmp_sem_monitor"
    }

    fn area(&self) -> Area {
        Area::Network
    }

    async fn run(&self, ctx: &AppContext, cycle: Cycle) -> AppResult<()> {
        let devices = due_unmonitored_snmp_devices(ctx, cycle.now).await?;
        stream::iter(devices)
            .for_each_concurrent(COLLECTION_CONCURRENCY, |device| async move {
                if let Err(error) = collect_unmonitored_device(ctx, &device).await {
                    tracing::debug!(%error, device_id = device.id, "falha na coleta SNMP do dispositivo");
                }
            })
            .await;
        Ok(())
    }
}

/// A fila de descoberta é persistente: o ciclo apenas enfileira as redes
/// vencidas e processa uma por vez para não saturar a LAN.
pub struct DiscoveryQueue;

#[async_trait]
impl Routine for DiscoveryQueue {
    fn name(&self) -> &'static str {
        "descoberta"
    }

    fn area(&self) -> Area {
        Area::Network
    }

    async fn run(&self, ctx: &AppContext, cycle: Cycle) -> AppResult<()> {
        if let Err(error) = schedule_due_networks(&ctx.db).await {
            tracing::warn!(%error, "falha ao agendar discovery");
        }
        let result = if cycle.detached {
            spawn_pending_run(ctx).await
        } else {
            process_pending_runs(ctx).await
        };
        if let Err(error) = result {
            tracing::warn!(%error, "falha ao processar discovery");
        }
        Ok(())
    }
}

/// Dispara o vigia dos clientes Wi-Fi vigiados. Roda em segundo plano: a
/// leitura dos APs por SSH leva segundos e não pode atrasar os monitores.
/// Com a área Rede desligada (ADR 017) o vigia fica pausado.
pub struct WifiWatch;

#[async_trait]
impl Routine for WifiWatch {
    fn name(&self) -> &'static str {
        "vigia_wifi"
    }

    fn area(&self) -> Area {
        Area::Network
    }

    async fn run(&self, ctx: &AppContext, cycle: Cycle) -> AppResult<()> {
        if !is_due("wifi_watch", WIFI_WATCH_INTERVAL_SECONDS, cycle.now) {
            return Ok(());
        }
        let ctx = ctx.clone();
        tokio::spawn(async move {
            if !areas::is_enabled(&ctx.db, Area::Network)
                .await
                .unwrap_or(true)
            {
                return;
            }
            if let Err(error) = wifi::watch::run(&ctx).await {
                tracing::warn!(%error, "falha no vigia dos clientes Wi-Fi");
            }
        });
        Ok(())
    }
}

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

/// As rotinas do ciclo, na ordem em que rodam.
#[must_use]
pub fn routines() -> Vec<Arc<dyn Routine>> {
    vec![
        Arc::new(UnmonitoredSnmpCollection),
        Arc::new(DiscoveryQueue),
        Arc::new(WifiWatch),
        Arc::new(LinkTestSchedules),
    ]
}

/// A retenção desta área.
#[must_use]
pub fn retention() -> Vec<Arc<dyn RetentionJob>> {
    vec![Arc::new(LinkTestRetention)]
}

/// Os grupos de monitores desta área.
#[must_use]
pub fn monitor_groups() -> Vec<Arc<dyn MonitorGroup>> {
    vec![Arc::new(SnmpDeviceGroup)]
}
