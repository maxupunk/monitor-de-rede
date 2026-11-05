//! A cadência SNMP de um dispositivo: o intervalo é do equipamento (campo do
//! inventário), e todos os monitores SNMP dele seguem o mesmo.

use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};

use crate::{
    models::{_entities::monitors as monitors_entity, monitors},
    monitoring::execution_guard::calculate_smart_timeout_seconds,
    shared::errors::AppResult,
};

/// Intervalo padrão da coleta SNMP de um dispositivo novo.
pub const DEFAULT_SNMP_POLL_INTERVAL_SECONDS: i32 = 15;

/// Mantém a cadência e o timeout de todos os itens SNMP do equipamento alinhados
/// ao intervalo que pertence ao próprio dispositivo.
pub async fn sync_monitor_intervals(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    interval_seconds: i32,
) -> AppResult<()> {
    let interval_seconds = interval_seconds.max(1);
    let timeout_seconds = calculate_smart_timeout_seconds("snmp", interval_seconds)
        .min((interval_seconds - 1).max(1))
        .max(1);
    monitors::Entity::update_many()
        .col_expr(
            monitors_entity::Column::IntervalSeconds,
            Expr::value(interval_seconds),
        )
        .col_expr(
            monitors_entity::Column::TimeoutSeconds,
            Expr::value(timeout_seconds),
        )
        .filter(monitors_entity::Column::DeviceId.eq(Some(device_id)))
        .filter(monitors_entity::Column::Type.eq("snmp"))
        .exec(db)
        .await?;
    Ok(())
}
