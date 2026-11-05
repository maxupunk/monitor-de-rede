//! Construtores de fatos: traduzem observações para o vocabulário das regras.

pub use netmonitor_core::alerts::datasets::interface_state;
pub mod log_pattern;
pub mod monitor_result;
pub mod vpn_peer;

// O dataset do cliente Wi-Fi vigiado é da área Rede (`netmonitor_network::wifi`).
use netmonitor_core::alerts::datasets::format_seconds;
