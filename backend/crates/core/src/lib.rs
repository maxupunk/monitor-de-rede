//! Núcleo do NetMonitor (ADR 017).
//!
//! O que toda área usa e que não depende de área nenhuma: o modelo de dados
//! (`models`), os tipos e utilidades transversais (`shared`), o barramento de
//! eventos que alimenta o SSE (`events`), as áreas do produto (`areas`), o
//! canal da central com os agentes (`agents`), a auditoria (`audit`), os
//! destinos de armazenamento (`storage`), o histórico de métricas de host
//! (`telemetry`), as plataformas e o dispositivo do sistema (`devices`), a
//! remoção de agregados (`maintenance`), o `vpn-probe` (`vpn_probe`) e os
//! pontos de extensão que as áreas implementam
//! (`monitoring::MonitorChecker`, `areas::AreaPolicy`, `classifier::Classifier`
//! e o contrato das ferramentas da IA em `ai`, com os contratos e o feed dos
//! alertas em `alerts`). A regra de dependência é garantida pelo `Cargo.toml`:
//! este crate só conhece o contrato de fio (`agent-protocol`) — não Rede,
//! Servidores, VPN nem o assistente.

pub mod agents;
pub mod ai;
pub mod alerts;
pub mod areas;
pub mod audit;
pub mod classifier;
pub mod devices;
pub mod events;
pub mod maintenance;
pub mod models;
pub mod monitoring;
pub mod network_tools;
pub mod notifications;
pub mod preferences;
pub mod shared;
pub mod storage;
pub mod telemetry;
pub mod vpn_probe;
