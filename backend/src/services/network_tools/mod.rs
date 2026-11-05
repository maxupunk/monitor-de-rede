//! Ferramentas de rede independentes de HTTP.
//!
//! Este módulo concentra os protocolos usados tanto pelos endpoints manuais
//! quanto pelos checkers e scanners de discovery. Assim controllers não
//! conhecem sockets, timeouts nem detalhes de wire-format.

pub mod dns;
pub mod mactelnet;
pub mod ping_capacity;
pub mod playbook;
pub mod speedtest;
pub mod throughput;
pub mod traceroute;

/// Os primitivos que os checkers do núcleo e a área Rede usam moram no núcleo.
pub use netmonitor_core::network_tools::{icmp_probe, neighbor_cache, tcp_probe, udp_probes};
