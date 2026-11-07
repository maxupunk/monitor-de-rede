//! O estado dos túneis WireGuard no motor de alertas (§8.7, §8.10.4): o
//! dataset `vpn_peer` ([`vpn_peer::VpnPeerDataset`], um `AlertDataset`) e o
//! observador que o entrega à central a cada ciclo da telemetria.

pub mod vpn_peer;
pub mod watcher;
