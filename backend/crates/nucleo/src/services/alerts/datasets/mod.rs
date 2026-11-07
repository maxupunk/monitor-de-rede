//! Construtores de fatos: traduzem observações para o vocabulário das regras.

pub use netmonitor_core::alerts::datasets::interface_state;
pub mod log_pattern;
pub mod monitor_result;

// Os datasets das áreas implementam `AlertDataset` no crate de cada uma: o
// cliente Wi-Fi vigiado na Rede (`netmonitor_network_wifi`) e o túnel
// WireGuard na VPN (`netmonitor_vpn::alerts`).
