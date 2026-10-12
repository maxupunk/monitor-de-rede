//! Decisões do Laya fora do chat. Cada uma é pura — perguntas e leitura das
//! respostas, sem banco nem rede — e se testa em unidade; quem consulta é o
//! [`runtime`](super::runtime).

pub mod device_identity;
pub mod incident_triage;
pub mod interfaces;
pub mod log_category;
