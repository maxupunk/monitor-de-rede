//! Consultas de leitura sobre a série temporal de métricas.
//!
//! Este módulo mantém a cardinalidade das consultas de "último valor"
//! proporcional ao número de séries pedidas, nunca ao tamanho do histórico.

use std::collections::BTreeSet;

use sea_orm::{DatabaseBackend, DatabaseConnection, FromQueryResult, Statement, Value};

use crate::{models::metrics, services::shared::errors::AppResult};

/// Mantém cada statement abaixo do limite histórico de 999 parâmetros do
/// SQLite, mesmo com várias séries por interface.
const INTERFACES_PER_QUERY: usize = 400;

/// Busca uma única amostra por `(interface_id, name)`.
///
/// Cada par solicitado faz um `ORDER BY ... LIMIT 1` apoiado pelos índices da
/// série. Assim nem o processo nem o SQLite ordenam o histórico inteiro. `id`
/// é o desempate estável para coletas gravadas no mesmo instante.
pub async fn latest_for_interfaces(
    db: &DatabaseConnection,
    device_id: Option<i64>,
    interface_ids: &[i64],
    names: &[&str],
) -> AppResult<Vec<metrics::Model>> {
    let interface_ids = interface_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let names = names
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if interface_ids.is_empty() || names.is_empty() {
        return Ok(Vec::new());
    }

    let mut latest = Vec::with_capacity(interface_ids.len().saturating_mul(names.len()));
    for interface_batch in interface_ids.chunks(INTERFACES_PER_QUERY) {
        latest.extend(latest_for_interface_batch(db, device_id, interface_batch, &names).await?);
    }
    Ok(latest)
}

async fn latest_for_interface_batch(
    db: &DatabaseConnection,
    device_id: Option<i64>,
    interface_ids: &[i64],
    names: &[&str],
) -> AppResult<Vec<metrics::Model>> {
    let backend = db.get_database_backend();
    let marker = |position: usize| match backend {
        DatabaseBackend::Postgres => format!("${position}"),
        _ => "?".to_string(),
    };
    let mut values = Vec::<Value>::new();
    let mut next_position = 1;

    let interface_markers = interface_ids
        .iter()
        .map(|id| {
            let current = marker(next_position);
            next_position += 1;
            values.push((*id).into());
            format!("(CAST({current} AS BIGINT))")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let name_markers = names
        .iter()
        .map(|name| {
            let current = marker(next_position);
            next_position += 1;
            values.push((*name).to_owned().into());
            format!("(CAST({current} AS TEXT))")
        })
        .collect::<Vec<_>>()
        .join(", ");

    let device_filter = if let Some(device_id) = device_id {
        let filter = format!("candidate.device_id = {} AND ", marker(next_position));
        values.push(device_id.into());
        filter
    } else {
        String::new()
    };

    let sql = format!(
        "WITH requested_interfaces(interface_id) AS (VALUES {interface_markers}), \
              requested_names(name) AS (VALUES {name_markers}) \
         SELECT m.id, m.device_id, m.interface_id, m.monitor_id, m.name, m.value, m.unit, \
                m.recorded_at, m.created_at \
           FROM requested_interfaces requested_interface \
          CROSS JOIN requested_names requested_name \
           JOIN metrics m ON m.id = (\
                SELECT candidate.id \
                  FROM metrics candidate \
                 WHERE {device_filter}candidate.interface_id = requested_interface.interface_id \
                   AND candidate.name = requested_name.name \
                 ORDER BY candidate.recorded_at DESC, candidate.id DESC \
                 LIMIT 1\
           ) \
          ORDER BY m.interface_id ASC, m.name ASC"
    );

    Ok(
        metrics::Model::find_by_statement(Statement::from_sql_and_values(backend, sql, values))
            .all(db)
            .await?,
    )
}
