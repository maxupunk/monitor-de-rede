//! Núcleo do NetMonitor (ADR 017).
//!
//! O que toda área usa e que não depende de área nenhuma: o modelo de dados
//! (`models`), os tipos e utilidades transversais (`shared`) e o barramento de
//! eventos que alimenta o SSE (`events`). A regra de dependência é garantida
//! pelo `Cargo.toml`: este crate não conhece Rede, Servidores, VPN nem a IA.

pub mod events;
pub mod models;
pub mod shared;
