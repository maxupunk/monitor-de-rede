//! Ferramentas de rede independentes de HTTP.
//!
//! Este módulo concentra os protocolos usados tanto pelos endpoints manuais
//! quanto pelos checkers e scanners de discovery. Assim controllers não
//! conhecem sockets, timeouts nem detalhes de wire-format.

pub mod dns;
pub mod mactelnet;
pub mod port_scanner;
pub mod udp_probes;
