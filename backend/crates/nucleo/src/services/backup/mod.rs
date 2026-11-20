//! Backup do próprio NetMonitor.
//!
//! | Peça | Responsabilidade |
//! |---|---|
//! | [`service`] | o arquivo de configuração: exportar, conferir e restaurar |
//! | [`catalog`] | o que entra em cada escopo (configuração, completo) e como cada tabela vai e volta |
//! | [`archive`] | o arquivo em fluxo: o backup completo em gzip e a leitura sem carregá-lo |
//! | [`restore`] | a restauração dos dois escopos, numa transação por banco |
//! | [`plan`] | o plano: destino, agenda, retenção e o resultado da última cópia |
//! | [`copies`] | as cópias num destino: enviar, listar, reter, ler |
//! | [`schedule`] | o ciclo que faz a cópia automática |

pub mod archive;
pub mod catalog;
pub mod copies;
pub mod notify;
pub mod plan;
pub mod restore;
pub mod schedule;
pub mod service;
