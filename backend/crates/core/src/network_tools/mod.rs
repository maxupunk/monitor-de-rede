//! Primitivos de rede do núcleo (AGENTS §11): o socket ICMP DGRAM (ADR 003),
//! a sonda TCP, as sondas UDP com payload conhecido, o cache de vizinhos
//! (ARP/NDP), a detecção de NAT do container e o IP público. Os checkers do núcleo e a área
//! Rede usam os mesmos.

pub mod icmp_probe;
pub mod nat;
pub mod neighbor_cache;
pub mod ping_client;
pub mod public_ip;
pub mod tcp_probe;
pub mod udp_probes;
