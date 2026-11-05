//! O canal da central com os agentes (ADR 011), do lado da central: conexões
//! vivas, correlação de pedidos e pontes. O contrato do fio mora em
//! `agent-protocol`; o cadastro, o handshake e os eventos recebidos, no
//! serviço de agentes. As áreas falam com um agente por aqui.
//!
//! * [`session`] / [`hub`] — conexões vivas e correlação de pedidos;
//! * [`bridge`] — pontes de bytes pelo canal (banco, volume);
//! * [`identity`] — quem é agente e quem é o local;
//! * [`registration`] / [`enrollment`] — cadastro e código de uso único;
//! * [`routes`] — agentes que servem de rota para um recurso;
//! * [`HostKey`] — a chave de um host Docker (`agent-<id>`).

pub mod bridge;
pub mod command;
pub mod enrollment;
mod errors;
mod host_key;
pub mod hub;
pub mod identity;
pub mod registration;
pub mod routes;
pub mod session;
pub mod targets;
pub mod views;

pub use host_key::HostKey;

/// Tópico SSE em que a central republica o snapshot Docker ao vivo de um
/// agente (`AgentEvent::DockerLive`).
pub const DOCKER_LIVE_EVENT: &str = "docker:snapshot";
/// Tópico SSE do inventário Docker de um agente (`AgentEvent::DockerInventory`).
pub const DOCKER_INVENTORY_EVENT: &str = "docker:inventory";
