//! Área Rede (ADR 017): descoberta, fabricantes (OUI), varredura de portas,
//! SNMP, topologia, plugins de dispositivo, espelho de pacotes, testes de
//! vazão e de enlace e Wi-Fi, com as rotas e telas que os servem.
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
//! * [`plugins`] — plugins de dispositivo (ADR 012): o script Rhai roda só
//!   no sandbox desta central e toda E/S passa por
//!   [`plugins::transport::DeviceTransport`];
//! * [`package_mirror`] — o espelho de pacotes que os plugins oferecem ao
//!   equipamento;
//! * [`throughput`] — os motores de vazão (iperf3, btest) e o agente como
//!   origem do teste (ADR 015);
//! * [`agent_commands`] — a descoberta, o acesso a equipamento e a vazão no
//!   host do agente (`AgentCommandHandler`);
//! * [`routines`] — a coleta SNMP do ciclo e a varredura agendada (registro
//!   de rotinas do núcleo);
//! * [`controllers`], [`dtos`] — as rotas da área.
//!
//! Este crate é a **base** da área: os submódulos `netmonitor-network-wifi`
//! (clientes Wi-Fi) e `netmonitor-network-link-tests` (testes de enlace, ADR
//! 015) dependem dele e continuam sendo a área Rede (Fase 14).

pub mod agent_commands;
pub mod area_policy;
pub mod central;
pub mod controllers;
pub mod device_access;
pub mod discovery;
pub mod dtos;
pub mod migrations;
pub mod package_mirror;
pub mod plugins;
pub mod port_scanner;
pub mod routines;
pub mod snmp;
pub mod throughput;
pub mod topology;
pub mod vendors;
pub mod widgets;

use std::sync::Arc;

use netmonitor_core::monitoring::MonitorChecker;

pub use area_policy::NetworkPolicy;
pub use snmp::checker::SnmpMonitor;

/// Os tipos de monitor desta área, para o registro do núcleo.
#[must_use]
pub fn monitor_checkers() -> Vec<Arc<dyn MonitorChecker>> {
    vec![Arc::new(SnmpMonitor)]
}
