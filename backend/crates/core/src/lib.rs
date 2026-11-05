//! Núcleo do NetMonitor (ADR 017).
//!
//! O que toda área usa e que não depende de área nenhuma: o modelo de dados
//! (`models`), os tipos e utilidades transversais (`shared`), o barramento de
//! eventos que alimenta o SSE (`events`), as áreas do produto (`areas`), o
//! canal da central com os agentes (`agents`), a auditoria (`audit`), os
//! destinos de armazenamento (`storage`), o histórico de métricas de host
//! (`telemetry`) e os pontos de extensão que as áreas implementam
//! (`monitoring::MonitorChecker`, `areas::AreaPolicy`). A regra de dependência
//! é garantida pelo `Cargo.toml`: este crate só conhece o contrato de fio
//! (`agent-protocol`) — não Rede, Servidores, VPN nem a IA.

pub mod agents;
pub mod areas;
pub mod audit;
pub mod events;
pub mod models;
pub mod monitoring;
pub mod shared;
pub mod storage;
pub mod telemetry;
