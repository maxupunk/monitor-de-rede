//! Componentes especializados da orquestração do ciclo de agendamento.
//!
//! O que as áreas fazem no ciclo (coleta SNMP, varredura, túneis, Wi-Fi,
//! enlaces) chega pelo registro de rotinas do núcleo
//! ([`netmonitor_core::routines`]) e pelos grupos de monitores
//! ([`netmonitor_core::monitoring::MonitorGroups`]).

pub mod maintenance_runner;
pub mod monitor_executor;

/// A cadência das rotinas é do núcleo, para as áreas usarem a mesma.
pub use netmonitor_core::routines::cadence::is_due;

/// Cadência da purga de dados antigos.
pub const DATA_PRUNE_INTERVAL_SECONDS: i64 = 3_600;

/// Cadência do rollup de resultados brutos em buckets horários.
pub const ROLLUP_INTERVAL_SECONDS: i64 = 3_600;

pub use maintenance_runner::{
    dispatch_notifications, rollup_monitor_results_if_due, run_data_pruner_if_due,
};
pub use monitor_executor::{execute_one, run_local_confirming_failure, MAX_RETRIES};
