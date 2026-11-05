//! Área Rede (ADR 017): descoberta, fabricantes (OUI), varredura de portas,
//! SNMP e topologia, com as rotas e telas que os servem.
//!
//! Depende só do núcleo (`netmonitor-core`) e do contrato de fio
//! (`agent-protocol`) — nunca de outra área nem do crate de montagem. O que
//! esta área pede ao resto da central (o motor de alertas) chega pelo
//! [`central::CentralServices`]; o que o núcleo precisa dela, pela
//! [`NetworkPolicy`] (`AreaPolicy`). Os dois são montados pelo ponto de
//! montagem.
//!
//! * [`discovery`] — varredura das redes, identificação e conflitos;
//! * [`vendors`] — a base OUI dos fabricantes;
//! * [`port_scanner`] — a varredura de portas TCP/UDP;
//! * [`snmp`] — coleta, interfaces, a observação do estado delas e o monitor
//!   `snmp` ([`SnmpMonitor`], registrado como `MonitorChecker`);
//! * [`topology`] — o mapa, os enlaces e o layout;
//! * [`controllers`], [`dtos`] — as rotas da área.

pub mod area_policy;
pub mod central;
pub mod controllers;
pub mod discovery;
pub mod dtos;
pub mod port_scanner;
pub mod snmp;
pub mod topology;
pub mod vendors;

use std::sync::Arc;

use netmonitor_core::monitoring::MonitorChecker;

pub use area_policy::NetworkPolicy;
pub use snmp::checker::SnmpMonitor;

/// Os tipos de monitor desta área, para o registro do núcleo.
#[must_use]
pub fn monitor_checkers() -> Vec<Arc<dyn MonitorChecker>> {
    vec![Arc::new(SnmpMonitor)]
}
