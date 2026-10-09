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

use std::io;

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::protocol::{TunnelFrame, TunnelPayload, WireFrame, TUNNEL_DATA_MAX};

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

/// Decide, quadro a quadro, se a compressão compensa.
#[derive(Debug, Default)]
pub struct DataEncoder {
    misses: u32,
    skipped: u32,
}

impl DataEncoder {
    /// Quadro de dados pronto para o fio, comprimido quando vale a pena.
    pub fn encode(&mut self, id: Uuid, data: &[u8]) -> Vec<u8> {
        if data.len() >= COMPRESS_MIN_BYTES && self.should_try() {
            if let Some(packed) = TunnelFrame::encode_compressed(id, data) {
                self.misses = 0;
                return packed;
            }
            self.misses = self.misses.saturating_add(1);
        }
        TunnelFrame {
            id,
            payload: TunnelPayload::Data(data.to_vec()),
        }
        .encode()
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

/// Bombeia bytes entre `stream` e a ponte `id` até as duas pontas fecharem.
///
/// # Errors
///
/// Falha de leitura ou escrita no socket local.
pub async fn pump<S>(
    id: Uuid,
    stream: S,
    mut inbound: mpsc::Receiver<TunnelPayload>,
    outbound: mpsc::Sender<WireFrame>,
    cancel: CancellationToken,
) -> io::Result<TunnelTotals>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    let mut totals = TunnelTotals::default();
    let mut encoder = DataEncoder::default();
    // Crédito para mandar; o do outro lado começa igual, sem aviso.
    let mut credit = u64::from(WINDOW);
    // Recebido e já escrito, mas ainda não devolvido como crédito.
    let mut unacked: u32 = 0;
    let mut local_eof = false;
    let mut remote_eof = false;
    let mut buffer = vec![0_u8; TUNNEL_DATA_MAX];
    let send = |bytes: Vec<u8>| {
        let outbound = outbound.clone();
        async move {
            outbound
                .send(WireFrame::Binary(bytes))
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "canal do agente caiu"))
        }
    };
    let control = |payload: TunnelPayload| TunnelFrame { id, payload }.encode();

    while !(local_eof && remote_eof) {
        // Sem crédito para o piso, não lê: o socket local espera, e o banco
        // (ou o `sqlx`) desacelera sozinho.
        let room = if credit >= u64::from(MIN_FRAME_COST) {
            usize::try_from(credit)
                .unwrap_or(usize::MAX)
                .min(TUNNEL_DATA_MAX)
        } else {
            0
        };
        tokio::select! {
            () = cancel.cancelled() => break,
            read = reader.read(&mut buffer[..room]), if !local_eof && room > 0 => {
                let count = read?;
                if count == 0 {
                    local_eof = true;
                    send(control(TunnelPayload::Eof)).await?;
                } else {
                    credit -= u64::from(frame_cost(count));
                    totals.sent += count as u64;
                    let frame = encoder.encode(id, &buffer[..count]);
                    totals.sent_on_wire += frame.len() as u64;
                    send(frame).await?;
                }
            }
            frame = inbound.recv() => match frame {
                // O outro lado sumiu (sessão fechada ou ponte cancelada).
                None => break,
                Some(TunnelPayload::Data(bytes)) => {
                    writer.write_all(&bytes).await?;
                    totals.received += bytes.len() as u64;
                    unacked = unacked.saturating_add(frame_cost(bytes.len()));
                    if unacked >= WINDOW / 2 {
                        send(control(TunnelPayload::Ack(unacked))).await?;
                        unacked = 0;
                    }
                }
                Some(TunnelPayload::Ack(count)) => credit += u64::from(count),
                Some(TunnelPayload::Eof) => {
                    let _ = writer.shutdown().await;
                    remote_eof = true;
                }
            },
        }
    }
    let _ = writer.shutdown().await;
    Ok(totals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agents::protocol::TunnelFrame;
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
}
