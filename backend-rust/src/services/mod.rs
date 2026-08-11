//! Módulos de domínio — o equivalente de `backend/modules/**` do AdonisJS.
//!
//! Regra do §1.3.3: o controller extrai, valida, delega e serializa. Toda regra
//! de negócio vive aqui e é testável sem HTTP.

pub mod discovery;
pub mod events;
pub mod maintenance;
pub mod monitoring;
pub mod network_tools;
pub mod shared;
pub mod snmp;
pub mod topology;
pub mod zabbix;
