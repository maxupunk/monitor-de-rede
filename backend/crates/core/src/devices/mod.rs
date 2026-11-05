//! O que o núcleo sabe de um dispositivo além da tabela: as plataformas
//! (`adapters`), o dispositivo do próprio sistema (`system_device`) e o
//! acesso ao equipamento que a Rede oferece (`access`).

pub mod access;
pub mod adapters;
pub mod kinds;
pub mod snmp_cadence;
pub mod system_device;
pub mod systems;
pub mod vpn_peer;
