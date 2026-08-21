//! Componentes especializados da orquestração do ciclo de agendamento.

pub mod cadence;
pub mod maintenance_runner;
pub mod monitor_executor;
pub mod snmp_group_executor;

pub use cadence::{
    is_due, DATA_PRUNE_INTERVAL_SECONDS, VPN_STATUS_INTERVAL_SECONDS, VPN_TRAFFIC_INTERVAL_SECONDS,
};
pub use maintenance_runner::{
    dispatch_notifications, rollup_monitor_results_if_due, run_data_pruner_if_due,
    sync_vpn_traffic_if_due,
};
pub use monitor_executor::{execute_one, run_local_confirming_failure, MAX_RETRIES};
pub use snmp_group_executor::{execute_snmp_device_group, local_snmp_device_id};
