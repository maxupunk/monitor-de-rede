//! Descoberta de rede (§8 do roadmap).
//!
//! A Fase 1 traz só o [`cidr_range`], porque os campos computados
//! `Network.scannable`/`usableHosts` (§6.1) dependem dele. Os scanners, a fila
//! e a sessão ao vivo entram na Fase 5.

pub mod cidr_range;
pub mod device_identifier;
pub mod merger;
pub mod oui_lookup;
pub mod queue;
pub mod scanners;
pub mod service;
