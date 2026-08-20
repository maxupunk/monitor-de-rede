//! Módulos de domínio.
//!
//! A regra da casa: o controller extrai, valida, delega e serializa. Toda regra
//! de negócio vive aqui e é testável sem HTTP.

pub mod alerts;
pub mod auth;
pub mod backup;
pub mod devices;
pub mod discovery;
pub mod events;
pub mod maintenance;
pub mod monitoring;
pub mod network_tools;
pub mod notifications;
pub mod onboarding;
pub mod preferences;
pub mod probes;
pub mod server_addresses;
pub mod shared;
pub mod snmp;
pub mod syslog;
pub mod topology;
pub mod users;
pub mod vpn;
