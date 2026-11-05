//! Coletores de discovery independentes. Cada um é best-effort: uma interface
//! sem multicast ou SNMP não invalida os resultados de ICMP/ARP.

pub mod arp;
pub mod http;
pub mod icmp;
pub mod mdns;
pub mod names;
pub mod ports;
pub mod snmp;
pub mod ssdp;
