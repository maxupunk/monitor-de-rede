//! Ponte TCP pelo canal do agente (ADR 013): a bomba de bytes.
//!
//! O mesmo código roda nas duas pontas — na central, entre o `sqlx` e o
//! WebSocket; no agente, entre o WebSocket e o banco da filial. Cada lado lê o
//! seu socket e manda [`TunnelPayload::Data`]; escreve no socket o que chega.
//!
//! ## Janela de crédito
//!
//! Cada lado só manda até [`WINDOW`] de **custo** sem confirmação e devolve
//! crédito ([`TunnelPayload::Ack`]) à medida que escreve no seu socket. Sem
//! isso, uma ponta lenta (gzip + envio ao S3, ou um banco ocupado) acumularia
//! memória sem limite do outro lado, e um dump grande encheria a fila que os
//! monitores do mesmo agente também usam.
//!
//! O custo de um quadro é o tamanho dos dados, com piso de [`MIN_FRAME_COST`]:
//! a fila de quem recebe é limitada em **quadros**, e uma conversa de consultas
//! curtas (dezenas de bytes cada) caberia na janela em bytes com milhares de
//! quadros — e estouraria a fila. Com o piso, a janela também limita quantos
//! quadros estão a caminho ([`INBOUND_CAPACITY`]).
//!
//! ## Compressão
//!
//! Cada quadro de dados é comprimido sozinho com zstd quando isso compensa
//! ([`TunnelFrame::encode_compressed`]). O crédito conta os bytes
//! **originais**, que os dois lados conhecem. Tráfego que não comprime (banco
//! com TLS) desliga as tentativas por um tempo ([`DataEncoder`]) para não gastar
//! CPU à toa.
//!
//! ## Fim
//!
//! [`TunnelPayload::Eof`] é meio-fechamento: quem o recebe fecha a escrita no
//! seu socket e continua lendo. A ponte termina quando os dois lados fecharam,
//! quando o canal cai ou quando é cancelada.

use std::{
    fmt, io,
    sync::atomic::{AtomicU64, Ordering},
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{mpsc, Notify},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::protocol::{TunnelFrame, TunnelPayload, WireFrame, TUNNEL_DATA_MAX, ZSTD_LEVEL};

/// Custo que um lado manda sem esperar crédito.
pub const WINDOW: u32 = 256 * 1024;

/// Piso do custo de um quadro de dados.
pub const MIN_FRAME_COST: u32 = 4 * 1024;

/// Quadros que esperam a bomba de uma ponte: os de dados que a janela deixa
/// estar a caminho, mais folga para os de controle (`Ack`, `Eof`). Um lado que
/// estoura isso violou o protocolo, e a ponte cai.
pub const INBOUND_CAPACITY: usize = (WINDOW / MIN_FRAME_COST) as usize + 8;

/// Abaixo disto não vale tentar comprimir: o cabeçalho do zstd come o ganho.
const COMPRESS_MIN_BYTES: usize = 512;

/// Tentativas seguidas sem ganho que desligam a compressão da ponte.
const GIVE_UP_AFTER: u32 = 16;

/// Desligada, a compressão é tentada de novo a cada tantos quadros — o fluxo
/// pode mudar (o banco recusou TLS e o resto é texto, por exemplo).
const RETRY_EVERY: u32 = 256;

/// A ponta local de uma ponte: um socket até o banco (ADR 013) ou o tar de
/// um volume com a escrita descartada (ADR 014). A bomba não distingue.
pub trait TunnelIo: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Unpin> TunnelIo for T {}

pub type TunnelStream = Box<dyn TunnelIo>;

/// Custo de um quadro de dados de `len` bytes na janela.
#[must_use]
pub fn frame_cost(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX).max(MIN_FRAME_COST)
}

/// O que passou pela ponte.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TunnelTotals {
    /// Lidos do socket local e mandados ao outro lado.
    pub sent: u64,
    /// Recebidos do outro lado e escritos no socket local.
    pub received: u64,
    /// O que de fato foi ao fio para mandar `sent` (com a compressão).
    pub sent_on_wire: u64,
}

/// Decide, quadro a quadro, se a compressão compensa, reutilizando o contexto zstd.
pub struct DataEncoder {
    misses: u32,
    skipped: u32,
    compressor: Option<zstd::bulk::Compressor<'static>>,
}

impl Default for DataEncoder {
    fn default() -> Self {
        Self {
            misses: 0,
            skipped: 0,
            compressor: zstd::bulk::Compressor::new(ZSTD_LEVEL).ok(),
        }
    }
}

impl fmt::Debug for DataEncoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataEncoder")
            .field("misses", &self.misses)
            .field("skipped", &self.skipped)
            .finish()
    }
}

impl DataEncoder {
    /// Quadro de dados pronto para o fio, comprimido quando vale a pena.
    pub fn encode(&mut self, id: Uuid, data: &[u8]) -> Vec<u8> {
        if data.len() >= COMPRESS_MIN_BYTES && self.should_try() {
            if let Some(compressor) = self.compressor.as_mut() {
                if let Some(packed) = TunnelFrame::encode_compressed_with(id, data, compressor) {
                    self.misses = 0;
                    return packed;
                }
                self.misses = self.misses.saturating_add(1);
            }
        }
        TunnelFrame::encode_raw(id, data)
    }

    fn should_try(&mut self) -> bool {
        if self.misses < GIVE_UP_AFTER {
            return true;
        }
        self.skipped += 1;
        if self.skipped < RETRY_EVERY {
            return false;
        }
        // Uma chance: comprimindo, `misses` volta a zero; senão, desliga de novo.
        self.skipped = 0;
        self.misses = GIVE_UP_AFTER - 1;
        true
    }
}

/// Crédito compartilhado entre leitura local (que consome) e escrita local (que devolve via `Ack`).
struct CreditWindow {
    available: AtomicU64,
    notify: Notify,
}

impl CreditWindow {
    fn new() -> Self {
        Self {
            available: AtomicU64::new(u64::from(WINDOW)),
            notify: Notify::new(),
        }
    }

    fn load(&self) -> u64 {
        self.available.load(Ordering::Acquire)
    }

    fn consume(&self, cost: u64) {
        self.available.fetch_sub(cost, Ordering::AcqRel);
    }

    /// Devolve crédito limitado a [`WINDOW`], evitando estouro ou violação da janela.
    fn release(&self, count: u32) {
        let cap = u64::from(WINDOW);
        let add = u64::from(count);
        let _ = self
            .available
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                Some(current.checked_add(add).map_or(cap, |next| next.min(cap)))
            });
        self.notify.notify_one();
    }
}

/// Bombeia bytes entre `stream` e a ponte `id` até as duas pontas fecharem.
///
/// Leitura e escrita correm em paralelo sobre o `stream` dividido: uma escrita
/// local lenta não bloqueia a leitura nem o processamento de `Ack`/`Cancel`.
///
/// # Errors
///
/// Falha de leitura/escrita no socket local ou encerramento abrupto do canal
/// antes de `TunnelPayload::Eof`.
pub async fn pump<S>(
    id: Uuid,
    stream: S,
    inbound: mpsc::Receiver<TunnelPayload>,
    outbound: mpsc::Sender<WireFrame>,
    cancel: CancellationToken,
) -> io::Result<TunnelTotals>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pump_inner(id, stream, inbound, outbound, cancel, Ok).await
}

/// Como [`pump`], mas recebe quadros binários crus e descomprime com um
/// `zstd::bulk::Decompressor` próprio desta ponte, fora do laço do WebSocket.
///
/// # Errors
///
/// Ver [`pump`], além de quadro binário malformado (`InvalidData`).
pub async fn pump_raw<S>(
    id: Uuid,
    stream: S,
    inbound: mpsc::Receiver<Vec<u8>>,
    outbound: mpsc::Sender<WireFrame>,
    cancel: CancellationToken,
) -> io::Result<TunnelTotals>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut decompressor = zstd::bulk::Decompressor::new().map_err(io::Error::other)?;
    pump_inner(
        id,
        stream,
        inbound,
        outbound,
        cancel,
        move |bytes: Vec<u8>| {
            TunnelFrame::decode_with(&bytes, &mut decompressor)
                .map(|frame| frame.payload)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
        },
    )
    .await
}

async fn send_wire(
    outbound: &mpsc::Sender<WireFrame>,
    stop: &CancellationToken,
    bytes: Vec<u8>,
) -> io::Result<bool> {
    tokio::select! {
        biased;
        () = stop.cancelled() => Ok(false),
        res = outbound.send(WireFrame::Binary(bytes)) => {
            res.map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "canal do agente caiu"))?;
            Ok(true)
        }
    }
}

async fn pump_inner<S, M, F>(
    id: Uuid,
    stream: S,
    mut inbound: mpsc::Receiver<M>,
    outbound: mpsc::Sender<WireFrame>,
    cancel: CancellationToken,
    mut decode: F,
) -> io::Result<TunnelTotals>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: FnMut(M) -> io::Result<TunnelPayload>,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    let credit = CreditWindow::new();
    let stop = cancel.child_token();
    let local_done = CancellationToken::new();

    let read_half = async {
        let mut encoder = DataEncoder::default();
        let mut buffer = vec![0_u8; TUNNEL_DATA_MAX];
        let mut sent: u64 = 0;
        let mut sent_on_wire: u64 = 0;
        let min_cost = u64::from(MIN_FRAME_COST);

        loop {
            if stop.is_cancelled() {
                break;
            }
            // Inscrito antes de consultar o crédito: um Ack entre o load e o
            // await não se perde.
            let wakeup = credit.notify.notified();
            tokio::pin!(wakeup);
            wakeup.as_mut().enable();
            let available = credit.load();
            if available < min_cost {
                tokio::select! {
                    biased;
                    () = stop.cancelled() => break,
                    () = wakeup => continue,
                }
            }
            let room = usize::try_from(available)
                .unwrap_or(usize::MAX)
                .min(TUNNEL_DATA_MAX);
            let read = tokio::select! {
                biased;
                () = stop.cancelled() => break,
                read = reader.read(&mut buffer[..room]) => read,
            };
            let count = match read {
                Ok(count) => count,
                Err(error) => {
                    stop.cancel();
                    return Err(error);
                }
            };
            if count == 0 {
                let eof = TunnelFrame {
                    id,
                    payload: TunnelPayload::Eof,
                }
                .encode();
                if let Err(error) = send_wire(&outbound, &stop, eof).await {
                    stop.cancel();
                    return Err(error);
                }
                break;
            }
            credit.consume(u64::from(frame_cost(count)));
            sent += count as u64;
            let frame = encoder.encode(id, &buffer[..count]);
            sent_on_wire += frame.len() as u64;
            match send_wire(&outbound, &stop, frame).await {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    stop.cancel();
                    return Err(error);
                }
            }
        }
        local_done.cancel();
        Ok::<(u64, u64), io::Error>((sent, sent_on_wire))
    };

    let write_half = async {
        let mut received: u64 = 0;
        let mut unacked: u32 = 0;
        let mut remote_eof = false;

        loop {
            if stop.is_cancelled() || (remote_eof && local_done.is_cancelled()) {
                break;
            }
            let next = tokio::select! {
                biased;
                () = stop.cancelled() => break,
                () = local_done.cancelled(), if remote_eof => break,
                frame = inbound.recv() => frame,
            };
            let Some(raw) = next else {
                if !remote_eof {
                    stop.cancel();
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "ponte encerrada antes do fim dos dados",
                    ));
                }
                // Sem entrada não chega mais `Ack`: a metade de leitura
                // ficaria presa esperando crédito para sempre.
                stop.cancel();
                break;
            };
            let payload = match decode(raw) {
                Ok(payload) => payload,
                Err(error) => {
                    stop.cancel();
                    return Err(error);
                }
            };
            match payload {
                TunnelPayload::Data(bytes) => {
                    let write = tokio::select! {
                        biased;
                        () = stop.cancelled() => break,
                        res = writer.write_all(&bytes) => res,
                    };
                    if let Err(error) = write {
                        stop.cancel();
                        return Err(error);
                    }
                    received += bytes.len() as u64;
                    unacked = unacked.saturating_add(frame_cost(bytes.len()));
                    if unacked >= WINDOW / 2 {
                        let ack = TunnelFrame {
                            id,
                            payload: TunnelPayload::Ack(unacked),
                        }
                        .encode();
                        match send_wire(&outbound, &stop, ack).await {
                            Ok(true) => unacked = 0,
                            Ok(false) => break,
                            Err(error) => {
                                stop.cancel();
                                return Err(error);
                            }
                        }
                    }
                }
                TunnelPayload::Ack(count) => credit.release(count),
                TunnelPayload::Eof => {
                    let _ = writer.shutdown().await;
                    remote_eof = true;
                }
            }
        }
        if remote_eof || cancel.is_cancelled() {
            let _ = writer.shutdown().await;
        }
        Ok::<u64, io::Error>(received)
    };

    let (read_res, write_res) = tokio::join!(read_half, write_half);
    let received = write_res?;
    let (sent, sent_on_wire) = read_res?;
    Ok(TunnelTotals {
        sent,
        received,
        sent_on_wire,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::TunnelFrame;
    use tokio::io::duplex;

    /// Liga duas bombas pelo "canal" (o que uma manda vira o que a outra recebe).
    async fn relay(mut from: mpsc::Receiver<WireFrame>, to: mpsc::Sender<TunnelPayload>) {
        while let Some(WireFrame::Binary(bytes)) = from.recv().await {
            let frame = TunnelFrame::decode(&bytes).expect("quadro válido");
            if to.send(frame.payload).await.is_err() {
                break;
            }
        }
    }

    /// Duas bombas ligadas; devolve os sockets locais (central, agente) e as
    /// tarefas.
    fn bridged(
        capacity: usize,
    ) -> (
        tokio::io::DuplexStream,
        tokio::io::DuplexStream,
        tokio::task::JoinHandle<io::Result<TunnelTotals>>,
        tokio::task::JoinHandle<io::Result<TunnelTotals>>,
    ) {
        let id = Uuid::new_v4();
        let (client, central_side) = duplex(capacity);
        let (agent_side, database) = duplex(capacity);
        let (central_out, central_wire) = mpsc::channel(INBOUND_CAPACITY);
        let (agent_out, agent_wire) = mpsc::channel(INBOUND_CAPACITY);
        let (to_central, central_in) = mpsc::channel(INBOUND_CAPACITY);
        let (to_agent, agent_in) = mpsc::channel(INBOUND_CAPACITY);
        tokio::spawn(relay(central_wire, to_agent));
        tokio::spawn(relay(agent_wire, to_central));
        let central = tokio::spawn(pump(
            id,
            central_side,
            central_in,
            central_out,
            CancellationToken::new(),
        ));
        let agent = tokio::spawn(pump(
            id,
            agent_side,
            agent_in,
            agent_out,
            CancellationToken::new(),
        ));
        (client, database, central, agent)
    }

    #[tokio::test]
    async fn bytes_atravessam_nos_dois_sentidos_e_a_ponte_fecha() {
        let (mut client, mut database, central, agent) = bridged(1024);

        // Mais que a janela, para o crédito precisar voltar no meio.
        let query = vec![7_u8; (WINDOW as usize) * 3];
        let writer = tokio::spawn(async move {
            client.write_all(&query).await.unwrap();
            client.shutdown().await.unwrap();
            let mut answer = Vec::new();
            client.read_to_end(&mut answer).await.unwrap();
            answer
        });
        let mut received = Vec::new();
        database.read_to_end(&mut received).await.unwrap();
        assert_eq!(received.len(), (WINDOW as usize) * 3);
        database.write_all(b"resultado").await.unwrap();
        database.shutdown().await.unwrap();

        assert_eq!(writer.await.unwrap(), b"resultado");
        let central = central.await.unwrap().unwrap();
        let agent = agent.await.unwrap().unwrap();
        assert_eq!(central.sent, u64::from(WINDOW) * 3);
        assert_eq!(agent.received, u64::from(WINDOW) * 3);
        assert_eq!(agent.sent, 9);
    }

    #[tokio::test]
    async fn dado_repetitivo_vai_comprimido_pelo_fio() {
        let (mut client, mut database, _central, agent) = bridged(TUNNEL_DATA_MAX);

        // Linhas de COPY: o que um dump de verdade manda.
        let rows: Vec<u8> = (0..20_000)
            .flat_map(|n| format!("{n}\tcliente {n}\tativo\t2026-10-08\n").into_bytes())
            .collect();
        let expected = rows.clone();
        tokio::spawn(async move {
            database.write_all(&rows).await.unwrap();
            database.shutdown().await.unwrap();
            let mut rest = Vec::new();
            let _ = database.read_to_end(&mut rest).await;
        });
        client.shutdown().await.unwrap();
        let mut received = Vec::new();
        client.read_to_end(&mut received).await.unwrap();
        assert_eq!(received, expected, "os bytes chegam idênticos");

        let totals = agent.await.unwrap().unwrap();
        assert_eq!(totals.sent, expected.len() as u64);
        assert!(
            totals.sent_on_wire * 3 < totals.sent,
            "{} bytes no fio para {}",
            totals.sent_on_wire,
            totals.sent
        );
    }

    #[tokio::test]
    async fn sem_credito_o_lado_rapido_espera() {
        let id = Uuid::new_v4();
        let (mut local, side) = duplex(TUNNEL_DATA_MAX * 8);
        let (out, mut wire) = mpsc::channel(INBOUND_CAPACITY * 4);
        let (_keep_inbound_open, inbound) = mpsc::channel(INBOUND_CAPACITY);
        tokio::spawn(pump(id, side, inbound, out, CancellationToken::new()));

        local
            .write_all(&vec![1; TUNNEL_DATA_MAX * 8])
            .await
            .unwrap();
        let mut sent = 0;
        while let Ok(Some(WireFrame::Binary(bytes))) =
            tokio::time::timeout(std::time::Duration::from_millis(200), wire.recv()).await
        {
            if let TunnelPayload::Data(data) = TunnelFrame::decode(&bytes).unwrap().payload {
                sent += data.len();
            }
        }
        // Ninguém devolveu crédito: só a janela saiu.
        assert_eq!(sent, WINDOW as usize);
    }

    #[tokio::test]
    async fn entrada_fechada_depois_do_eof_remoto_encerra_a_bomba() {
        let id = Uuid::new_v4();
        let (mut local, side) = duplex(TUNNEL_DATA_MAX * 8);
        let (out, mut wire) = mpsc::channel(INBOUND_CAPACITY * 4);
        let (to_pump, inbound) = mpsc::channel(INBOUND_CAPACITY);
        let pumped = tokio::spawn(pump(id, side, inbound, out, CancellationToken::new()));
        tokio::spawn(async move { while wire.recv().await.is_some() {} });

        // Mais que a janela: sem `Ack`, a leitura esgota o crédito.
        local
            .write_all(&vec![1; TUNNEL_DATA_MAX * 8])
            .await
            .unwrap();
        to_pump.send(TunnelPayload::Eof).await.unwrap();
        drop(to_pump);

        let totals = tokio::time::timeout(std::time::Duration::from_secs(2), pumped)
            .await
            .expect("a bomba não pode ficar presa esperando crédito")
            .unwrap()
            .unwrap();
        assert!(totals.sent <= u64::from(WINDOW));
        drop(local);
    }

    /// Consultas curtas: a janela em bytes deixaria milhares de quadros a
    /// caminho; o piso de custo segura em `WINDOW / MIN_FRAME_COST`.
    #[tokio::test]
    async fn quadros_pequenos_nao_estouram_a_fila_de_quem_recebe() {
        let id = Uuid::new_v4();
        let (mut local, side) = duplex(64);
        let (out, mut wire) = mpsc::channel(4096);
        let (_keep_inbound_open, inbound) = mpsc::channel(INBOUND_CAPACITY);
        tokio::spawn(pump(id, side, inbound, out, CancellationToken::new()));

        // Escritas de 10 bytes, com o duplex pequeno forçando leituras curtas.
        tokio::spawn(async move {
            for _ in 0..10_000 {
                if local.write_all(b"SELECT 1;\n").await.is_err() {
                    break;
                }
            }
        });
        let mut frames = 0;
        while let Ok(Some(_)) =
            tokio::time::timeout(std::time::Duration::from_millis(300), wire.recv()).await
        {
            frames += 1;
        }
        assert!(frames > 0);
        assert!(
            frames <= (WINDOW / MIN_FRAME_COST) as usize,
            "{frames} quadros a caminho sem crédito"
        );
        assert!(frames < INBOUND_CAPACITY);
    }

    #[test]
    fn trafego_que_nao_comprime_desliga_as_tentativas_e_volta_a_tentar() {
        let mut encoder = DataEncoder::default();
        let mut state: u32 = 0x9E37_79B9;
        let mut noise = || -> Vec<u8> {
            (0..2048)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    state.to_le_bytes()[0]
                })
                .collect()
        };
        for _ in 0..GIVE_UP_AFTER {
            encoder.encode(Uuid::new_v4(), &noise());
        }
        assert!(
            !encoder.should_try(),
            "desligou depois das tentativas sem ganho"
        );
        for _ in 0..RETRY_EVERY - 2 {
            assert!(!encoder.should_try());
        }
        assert!(encoder.should_try(), "tenta de novo de tempos em tempos");
        let text = b"linha de texto repetida\n".repeat(200);
        let frame = encoder.encode(Uuid::new_v4(), &text);
        assert!(frame.len() < text.len() / 4, "voltou a comprimir");
        assert_eq!(encoder.misses, 0);
    }

    #[test]
    fn o_custo_de_um_quadro_tem_piso() {
        assert_eq!(frame_cost(10), MIN_FRAME_COST);
        assert_eq!(frame_cost(TUNNEL_DATA_MAX), TUNNEL_DATA_MAX as u32);
    }

    #[tokio::test]
    async fn cancelar_encerra_a_bomba() {
        let (_local, side) = duplex(64);
        let (out, _wire) = mpsc::channel(4);
        let (_keep, inbound) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let task = tokio::spawn(pump(Uuid::new_v4(), side, inbound, out, cancel.clone()));
        cancel.cancel();
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn encerramento_sem_eof_retorna_unexpected_eof() {
        let (_local, side) = duplex(1024);
        let (out, _wire) = mpsc::channel(4);
        let (tx, inbound) = mpsc::channel(4);
        tx.send(TunnelPayload::Data(b"parcial".to_vec()))
            .await
            .unwrap();
        drop(tx);
        let err = pump(Uuid::new_v4(), side, inbound, out, CancellationToken::new())
            .await
            .expect_err("sem Eof precisa falhar");
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn trafego_bidirecional_simultaneo_nao_entra_em_deadlock() {
        // Duplex pequeno (2 KiB) com envio pesado simultâneo nos dois sentidos:
        // um laço único travaria em write_all bloqueando read/Ack.
        let (client, database, central, agent) = bridged(2048);
        let payload_a = vec![0x41_u8; (WINDOW as usize) * 2];
        let payload_b = vec![0x42_u8; (WINDOW as usize) * 2];
        let expected_a = payload_a.clone();
        let expected_b = payload_b.clone();

        let (mut c_read, mut c_write) = tokio::io::split(client);
        let (mut d_read, mut d_write) = tokio::io::split(database);

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let (wa, wb, ra, rb) = tokio::join!(
                async move {
                    c_write.write_all(&payload_a).await.unwrap();
                    c_write.shutdown().await.unwrap();
                },
                async move {
                    d_write.write_all(&payload_b).await.unwrap();
                    d_write.shutdown().await.unwrap();
                },
                async move {
                    let mut buf = Vec::new();
                    d_read.read_to_end(&mut buf).await.unwrap();
                    buf
                },
                async move {
                    let mut buf = Vec::new();
                    c_read.read_to_end(&mut buf).await.unwrap();
                    buf
                }
            );
            let _ = (wa, wb);
            assert_eq!(ra, expected_a);
            assert_eq!(rb, expected_b);
            central.await.unwrap().unwrap();
            agent.await.unwrap().unwrap();
        })
        .await;
        assert!(result.is_ok(), "ponte bidirecional entrou em deadlock");
    }

    #[tokio::test]
    async fn cancelar_interrompe_escrita_bloqueada_em_buffer_cheio() {
        let (_local, side) = duplex(16);
        let (out, _wire) = mpsc::channel(4);
        let (tx, inbound) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let task = tokio::spawn(pump(Uuid::new_v4(), side, inbound, out, cancel.clone()));
        // Enche o duplex de 16 bytes e deixa writer.write_all bloqueado.
        tx.send(TunnelPayload::Data(vec![1_u8; 4096]))
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        cancel.cancel();
        let finished = tokio::time::timeout(std::time::Duration::from_secs(1), task).await;
        assert!(finished.is_ok(), "cancel não interrompeu write_all");
        assert!(finished.unwrap().unwrap().is_ok());
    }
}
