//! Agentes remotos: o canal de comando entre a central e os servidores
//! (ADR 011).
//!
//! * [`protocol`] — o contrato de fio (crate `agent-protocol`), compartilhado
//!   com o binário do agente;
//! * [`policy`] — o que cada host aceita, decidido **no host**;
//! * [`session`] / [`hub`] — conexões vivas e correlação de pedidos (núcleo);
//! * [`connection`] — handshake e vida de uma conexão (sem WebSocket);
//! * [`service`] / [`enrollment`] — cadastro, código de uso único, token;
//! * [`local`] — o agente ao lado da central, dono do Docker dela (ADR 014);
//! * [`throughput`] — o agente como origem de teste de vazão (ADR 015 §6);
//! * [`inbound`] / [`background`] — eventos recebidos, modo ao vivo e pump
//!   de tarefas.
//!
//! O lado do agente mora em [`crate::services::agent_runtime`].

pub mod background;
pub mod connection;
pub mod enrollment;
pub mod inbound;
pub mod install;
pub mod install_address;
pub mod local;
pub mod service;
pub mod throughput;

/// O contrato de fio é o crate `agent-protocol`; o transporte do lado da
/// central (sessões, hub, pontes, rotas) é do núcleo. Os caminhos antigos
/// continuam valendo.
pub use agent_protocol::{policy, tunnel};
pub use netmonitor_core::agents::{bridge, hub, routes, session};

pub mod protocol {
    //! O contrato de fio (`agent-protocol`), pelo caminho antigo.
    pub use agent_protocol::*;
}
