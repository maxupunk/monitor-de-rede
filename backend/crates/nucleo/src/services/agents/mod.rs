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
//! * o agente como origem de teste de vazão (ADR 015 §6) é da área Rede
//!   (`netmonitor_network::throughput::agent`);
//! * [`inbound`] / [`background`] — eventos recebidos, modo ao vivo e pump
//!   de tarefas.
//!
//! O lado do agente mora em [`crate::services::agent_runtime`].

pub mod background;
pub mod connection;
pub mod inbound;
pub mod install;
pub mod install_address;
pub mod local;
pub mod service;

/// O contrato de fio é o crate `agent-protocol`; o transporte do lado da
/// central (sessões, hub, pontes, rotas) é do núcleo. Os caminhos antigos
/// continuam valendo.
pub use agent_protocol::{policy, tunnel};
pub use netmonitor_core::agents::{bridge, enrollment, hub, routes, session};

pub mod protocol {
    //! O contrato de fio (`agent-protocol`), pelo caminho antigo.
    pub use agent_protocol::*;
}

/// Escreve uma mensagem no socket sem deixar a escrita presa: desiste quando
/// `stop` é cancelado ou quando `limit` passa (buffer do TCP cheio de um par
/// half-open, que nunca drena). Devolve `false` quando a conexão deve cair.
pub(crate) async fn send_bounded<S, M>(
    sink: &mut S,
    message: M,
    stop: &tokio_util::sync::CancellationToken,
    limit: std::time::Duration,
) -> bool
where
    S: futures::Sink<M> + Unpin,
{
    use futures::SinkExt;
    tokio::select! {
        biased;
        () = stop.cancelled() => false,
        sent = tokio::time::timeout(limit, sink.send(message)) => match sent {
            Ok(Ok(())) => true,
            Ok(Err(_)) => false,
            Err(_) => {
                tracing::warn!(
                    segundos = limit.as_secs(),
                    "escrita no socket do agente presa (half-open); encerrando"
                );
                false
            }
        },
    }
}

/// Despachante de saída com prioridade para controle: envelopes JSON
/// (`Response`, `Chunk`, `Event`, `Cancel`) sempre saem antes dos quadros
/// binários das pontes (`tunnel_frames`). Cancela `on_closed` quando o canal
/// de saída cai para acordar o laço principal da sessão.
pub(crate) fn spawn_priority_writer(
    mut to_send: tokio::sync::mpsc::Receiver<protocol::Envelope>,
    mut tunnel_frames: tokio::sync::mpsc::Receiver<protocol::WireFrame>,
    outgoing: tokio::sync::mpsc::Sender<protocol::WireFrame>,
    on_closed: tokio_util::sync::CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let frame = tokio::select! {
                biased;
                envelope = to_send.recv() => match envelope {
                    Some(envelope) => match envelope.encode() {
                        Ok(text) => protocol::WireFrame::Text(text),
                        Err(error) => {
                            tracing::warn!(%error, "quadro de agente descartado por erro de codificação");
                            continue;
                        }
                    },
                    None => break,
                },
                Some(frame) = tunnel_frames.recv() => frame,
            };
            if outgoing.send(frame).await.is_err() {
                on_closed.cancel();
                break;
            }
        }
    })
}
