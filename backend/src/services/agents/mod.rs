//! Agentes remotos: o canal de comando entre a central e os servidores
//! (ADR 011).
//!
//! * [`protocol`] — o contrato de fio, compartilhado com o binário do agente;
//! * [`policy`] — o que cada host aceita, decidido **no host**;
//! * [`session`] / [`hub`] — conexões vivas e correlação de pedidos;
//! * [`connection`] — handshake e vida de uma conexão (sem WebSocket);
//! * [`service`] / [`enrollment`] — cadastro, código de uso único, token;
//! * [`remote_engine`] — o Docker do host remoto como fonte da central;
//! * [`inbound`] / [`background`] — eventos recebidos, modo ao vivo e pump
//!   de tarefas.
//!
//! O lado do agente mora em [`crate::services::agent_runtime`].

pub mod background;
pub mod connection;
pub mod enrollment;
pub mod hub;
pub mod inbound;
pub mod install;
pub mod install_address;
pub mod policy;
pub mod protocol;
pub mod remote_engine;
pub mod service;
pub mod session;
