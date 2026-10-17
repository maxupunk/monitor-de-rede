//! Bomba de dados do btest em TCP, igual para servidor e cliente.
//!
//! Depois do handshake os dois lados são simétricos: quem transmite escreve
//! blocos zerados; quem recebe conta bytes e, a cada segundo, escreve uma
//! mensagem de estado ([`Status`]) na mesma conexão — intercalada com os
//! dados quando os dois sentidos estão ativos. O que muda entre servidor e
//! cliente é só quem transmite e como cada sentido se chama no relatório.

use std::time::Duration;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    time::{timeout, Instant},
};
use tokio_util::sync::CancellationToken;

use super::codec::{Status, StatusScanner, TCP_TX_SIZE};
use crate::services::{
    network_tools::throughput::{
        cancelled,
        meter::{ByteCounter, Pacer, Sampler, SAMPLE_INTERVAL},
        Flow, SampleSink, ThroughputSample,
    },
    shared::errors::AppResult,
};

/// Menor e maior bloco aceitos de um comando (o RouterOS usa 32 KiB em TCP).
const MIN_BLOCK: usize = 1_024;
const MAX_BLOCK: usize = 64 * 1_024;

/// O que este lado faz na conexão.
pub(super) struct DuplexPlan {
    /// Este lado transmite.
    pub send: bool,
    /// Este lado recebe (e, por isso, informa o estado a cada segundo).
    pub receive: bool,
    pub block: usize,
    pub pace_bps: Option<u64>,
    /// Encerra após este tempo, mesmo que o par não feche.
    pub max_duration: Duration,
    /// Como o envio deste lado aparece nas amostras.
    pub sent_flow: Flow,
    /// Como a recepção deste lado aparece nas amostras.
    pub received_flow: Flow,
}

/// Tamanho de bloco a partir do comando, dentro de limites sãos.
pub(super) fn block_size(tx_size: u16) -> usize {
    match usize::from(tx_size) {
        0 => usize::from(TCP_TX_SIZE),
        size => size.clamp(MIN_BLOCK, MAX_BLOCK),
    }
}

pub(super) struct DuplexOutcome {
    pub sent: u64,
    /// Bytes de dados recebidos, sem as mensagens de estado.
    pub received: u64,
    pub elapsed: Duration,
    pub samples: Vec<ThroughputSample>,
}

/// Roda até `max_duration`, até o par fechar ou até `cancel`.
///
/// # Errors
///
/// Só o cancelamento: queda do par é o fim normal de um teste btest (quem
/// dirige fecha a conexão quando a duração acaba).
pub(super) async fn run(
    stream: TcpStream,
    plan: &DuplexPlan,
    sink: &SampleSink,
    cancel: &CancellationToken,
) -> AppResult<DuplexOutcome> {
    let (reader, writer) = stream.into_split();
    let sent = ByteCounter::default();
    let received = ByteCounter::default();
    let stop = CancellationToken::new();
    let peer_gone = CancellationToken::new();

    let read_task = tokio::spawn(read_loop(
        reader,
        received.clone(),
        stop.clone(),
        peer_gone.clone(),
    ));
    let write_task = tokio::spawn(write_loop(
        writer,
        WriteSide {
            send: plan.send,
            block: plan.block,
            pace_bps: plan.pace_bps,
        },
        (sent.clone(), received.clone()),
        stop.clone(),
        peer_gone.clone(),
    ));

    let mut flows = Vec::with_capacity(2);
    if plan.send {
        flows.push((plan.sent_flow, sent.clone()));
    }
    if plan.receive {
        flows.push((plan.received_flow, received.clone()));
    }
    let sampler = Sampler::spawn(flows, sink.clone(), 0);
    let started = Instant::now();
    let was_cancelled = tokio::select! {
        () = cancel.cancelled() => true,
        () = tokio::time::sleep(plan.max_duration) => false,
        () = peer_gone.cancelled() => false,
    };
    let elapsed = started.elapsed();
    stop.cancel();
    let samples = sampler.finish().await;
    let scanner = read_task.await.unwrap_or_default();
    let _ = write_task.await;
    if was_cancelled {
        return Err(cancelled());
    }
    // O que saiu do socket não é o que chegou (fica no buffer): quando o par
    // informa o que recebeu, é essa a medida de quem transmite.
    let sent = if scanner.peer_received > 0 {
        scanner.peer_received
    } else {
        sent.get()
    };
    Ok(DuplexOutcome {
        sent,
        received: received.get().saturating_sub(scanner.control_bytes()),
        elapsed,
        samples,
    })
}

async fn read_loop(
    mut reader: OwnedReadHalf,
    received: ByteCounter,
    stop: CancellationToken,
    peer_gone: CancellationToken,
) -> StatusScanner {
    let mut scanner = StatusScanner::default();
    let mut buf = vec![0u8; MAX_BLOCK];
    loop {
        let read = tokio::select! {
            () = stop.cancelled() => break,
            read = reader.read(&mut buf) => read,
        };
        match read {
            Ok(0) | Err(_) => {
                peer_gone.cancel();
                break;
            }
            Ok(n) => {
                scanner.feed(&buf[..n]);
                received.add(n);
            }
        }
    }
    scanner
}

struct WriteSide {
    send: bool,
    block: usize,
    pace_bps: Option<u64>,
}

async fn write_loop(
    mut writer: OwnedWriteHalf,
    side: WriteSide,
    (sent, received): (ByteCounter, ByteCounter),
    stop: CancellationToken,
    peer_gone: CancellationToken,
) {
    let block = vec![0u8; side.block];
    let mut pacer = Pacer::new(side.pace_bps);
    let mut next_status = Instant::now() + SAMPLE_INTERVAL;
    let mut seq: u32 = 0;
    let mut reported = 0u64;
    loop {
        if Instant::now() >= next_status {
            seq = seq.wrapping_add(1);
            let total = received.get();
            let status = Status {
                seq,
                received: u32::try_from(total - reported).unwrap_or(u32::MAX),
            };
            reported = total;
            next_status += SAMPLE_INTERVAL;
            let message = status.encode();
            let written = tokio::select! {
                () = stop.cancelled() => break,
                written = writer.write_all(&message) => written,
            };
            if written.is_err() {
                peer_gone.cancel();
                break;
            }
        }
        if side.send {
            let written = tokio::select! {
                () = stop.cancelled() => break,
                written = writer.write_all(&block) => written,
            };
            if written.is_err() {
                peer_gone.cancel();
                break;
            }
            sent.add(block.len());
            tokio::select! {
                () = stop.cancelled() => break,
                () = pacer.pace(block.len()) => {}
            }
        } else {
            tokio::select! {
                () = stop.cancelled() => break,
                () = tokio::time::sleep_until(next_status) => {}
            }
        }
    }
    // Avisa o par que acabou; melhor esforço.
    let _ = timeout(Duration::from_secs(1), writer.shutdown()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloco_respeita_limites() {
        assert_eq!(block_size(0), 32_768);
        assert_eq!(block_size(0x8000), 32_768);
        assert_eq!(block_size(10), MIN_BLOCK);
        assert_eq!(block_size(u16::MAX), u16::MAX as usize);
        assert_eq!(block_size(1_500), 1_500);
    }
}
