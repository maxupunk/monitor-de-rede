//! Backup do próprio NetMonitor.
//!
//! | Peça | Responsabilidade |
//! |---|---|
//! | [`service`] | o arquivo: exportar, conferir e restaurar a configuração |
//! | [`plan`] | o plano: destino, agenda, retenção e o resultado da última cópia |
//! | [`copies`] | as cópias num destino: enviar, listar, reter, ler |
//! | [`schedule`] | o ciclo que faz a cópia automática |

pub mod copies;
pub mod plan;
pub mod schedule;
pub mod service;
