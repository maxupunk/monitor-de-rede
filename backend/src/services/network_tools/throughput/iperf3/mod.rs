//! Cliente iperf3 nativo (TCP), compatível com o `iperf3 -s` 3.x (ADR 015 §1).
//!
//! Sequência de uma fase (um sentido):
//!
//! 1. controle: conecta, manda o cookie e segue os estados do servidor —
//!    `PARAM_EXCHANGE` (manda os parâmetros), `CREATE_STREAMS` (abre os
//!    fluxos, cada um mandando o mesmo cookie), `TEST_START`, `TEST_RUNNING`;
//! 2. dados: envia (ou, com `reverse`, recebe) durante a duração, com uma
//!    amostra por segundo no `sink`;
//! 3. fim: manda `TEST_END`, troca resultados em `EXCHANGE_RESULTS`,
//!    responde `DISPLAY_RESULTS` com `IPERF_DONE` e fecha.
//!
//! **Dois sentidos** ([`TestDirection::Both`]) são duas fases em sequência no
//! mesmo servidor: primeiro envia, depois recebe. O `iperf3 -s` precisa ser
//! iniciado **sem `-1`** (`--one-off`), senão encerra depois da primeira fase
//! e a segunda falha por conexão recusada.
//!
//! Origem de cada número do relatório:
//!
//! * `tx` (quem dirige envia): bytes que o **servidor** disse ter recebido —
//!   o que de fato atravessou o enlace, sem o que ficou no buffer local;
//! * `rx` (quem dirige recebe, `reverse`): bytes recebidos aqui até o
//!   `TEST_END`;
//! * `retransmits`: as do servidor quando ele foi quem enviou (`reverse`) e as
//!   informou (`sender_has_retransmits = 1`, Linux); do lado de cá não há
//!   `TCP_INFO`, então a fase de envio não as traz.

mod wire;

#[cfg(test)]
pub(crate) mod fake_server;

use std::{net::SocketAddr, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    task::JoinSet,
    time::{timeout, Instant},
};
use tokio_util::sync::CancellationToken;

use self::wire::{ServerResults, State, DEFAULT_TCP_BLKSIZE};
use super::{
    cancelled, elapsed_ms,
    meter::{ByteCounter, Pacer, Sampler},
    Flow, SampleSink, ThroughputEngine, ThroughputReport, ThroughputSample, ThroughputSpec,
    DEADLINE_MARGIN, HANDSHAKE_TIMEOUT,
};
use crate::services::shared::errors::{AppError, AppResult};

/// Porta padrão do `iperf3 -s`.
pub const DEFAULT_PORT: u16 = 5201;

/// Novas tentativas de uma fase enquanto o servidor responde "ocupado".
const BUSY_RETRIES: u32 = 4;
/// Espera entre essas tentativas.
const BUSY_BACKOFF: Duration = Duration::from_millis(500);

/// Executa um teste iperf3 contra o servidor em `addr`.
///
/// `spec.engine` precisa ser [`ThroughputEngine::Iperf3`]. Cada segundo de
/// medição vira uma [`ThroughputSample`] no `sink`; o relatório final traz as
/// mesmas amostras. Cada passo de conexão e handshake tem prazo de 5 s, e a
/// fase inteira, `duração + 15 s`.
///
/// # Errors
///
/// * [`AppError::Validation`] — especificação fora do contrato;
/// * [`AppError::ServiceUnavailable`] — servidor inalcançável, mudo ou que
///   fechou a conexão;
/// * [`AppError::Conflict`] — servidor ocupado com outro teste
///   (`ACCESS_DENIED`);
/// * [`AppError::BusinessRule`] — erro relatado pelo servidor, resposta fora
///   do protocolo ou cancelamento (ver [`super::is_cancelled`]).
pub async fn run_client(
    addr: SocketAddr,
    spec: &ThroughputSpec,
    sink: &SampleSink,
    cancel: &CancellationToken,
) -> AppResult<ThroughputReport> {
    spec.validate_for(ThroughputEngine::Iperf3)?;
    let mut report = ThroughputReport::empty(ThroughputEngine::Iperf3);
    let mut offset_ms = 0;
    if spec.direction.sends() {
        let phase = run_phase_patiently(addr, spec, false, sink, cancel, offset_ms).await?;
        report.set_tx(phase.server.bytes, phase.elapsed);
        report.samples.extend(phase.samples);
        offset_ms = elapsed_ms(phase.elapsed);
    }
    if spec.direction.receives() {
        let phase = run_phase_patiently(addr, spec, true, sink, cancel, offset_ms).await?;
        report.set_rx(phase.local_bytes, phase.elapsed);
        report.retransmits = phase.server.retransmits;
        report.samples.extend(phase.samples);
    }
    Ok(report)
}

/// Resultado de uma fase (um sentido).
struct PhaseOutcome {
    /// Bytes enviados ou recebidos do lado de cá até o `TEST_END`.
    local_bytes: u64,
    elapsed: Duration,
    server: ServerResults,
    samples: Vec<ThroughputSample>,
}

/// Uma fase, tentando de novo enquanto o servidor responde `ACCESS_DENIED`.
///
/// O `iperf3 -s` leva um instante para liberar a vaga depois de uma fase: a
/// volta, que conecta logo depois da ida, pode chegar antes disso — sobretudo
/// num equipamento de CPU fraca ou ocupada — e levar um "ocupado". A recusa
/// vem no handshake, antes de qualquer amostra, então tentar de novo é seguro.
async fn run_phase_patiently(
    addr: SocketAddr,
    spec: &ThroughputSpec,
    reverse: bool,
    sink: &SampleSink,
    cancel: &CancellationToken,
    offset_ms: u64,
) -> AppResult<PhaseOutcome> {
    let mut retries = 0;
    loop {
        match run_phase(addr, spec, reverse, sink, cancel, offset_ms).await {
            Err(AppError::Conflict(_)) if retries < BUSY_RETRIES => {
                retries += 1;
                tokio::select! {
                    () = cancel.cancelled() => return Err(cancelled()),
                    () = tokio::time::sleep(BUSY_BACKOFF) => {}
                }
            }
            outcome => return outcome,
        }
    }
}

async fn run_phase(
    addr: SocketAddr,
    spec: &ThroughputSpec,
    reverse: bool,
    sink: &SampleSink,
    cancel: &CancellationToken,
    offset_ms: u64,
) -> AppResult<PhaseOutcome> {
    let mut ctrl = tokio::select! {
        () = cancel.cancelled() => return Err(cancelled()),
        conn = connect(addr) => conn?,
    };
    let deadline = spec.duration() + DEADLINE_MARGIN;
    let result = tokio::select! {
        () = cancel.cancelled() => Err(cancelled()),
        outcome = timeout(deadline, drive(&mut ctrl, addr, spec, reverse, sink, offset_ms)) => {
            outcome.unwrap_or_else(|_| {
                Err(AppError::service_unavailable(
                    "O teste iperf3 excedeu o prazo sem terminar",
                ))
            })
        }
    };
    if result.is_err() {
        // Libera a vaga do servidor já, sem esperar o prazo dele. Melhor
        // esforço: o servidor pode ter sido quem fechou.
        let _ = timeout(
            Duration::from_secs(1),
            wire::write_state(&mut ctrl, State::ClientTerminate),
        )
        .await;
    }
    result
}

/// Conecta com prazo, sem Nagle (o controle troca mensagens de 1 byte).
async fn connect(addr: SocketAddr) -> AppResult<TcpStream> {
    let stream = timeout(HANDSHAKE_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| {
            AppError::service_unavailable(format!("Sem resposta do servidor iperf3 em {addr}"))
        })?
        .map_err(|error| {
            AppError::service_unavailable(format!(
                "Não foi possível conectar ao servidor iperf3 em {addr}: {error}"
            ))
        })?;
    let _ = stream.set_nodelay(true);
    Ok(stream)
}

/// Um passo do handshake, com o prazo de 5 s.
async fn step<T>(future: impl std::future::Future<Output = AppResult<T>>) -> AppResult<T> {
    timeout(HANDSHAKE_TIMEOUT, future).await.map_err(|_| {
        AppError::service_unavailable("O servidor iperf3 parou de responder no handshake")
    })?
}

/// Antes do `TEST_RUNNING`, o servidor que fecha o controle sem dizer nada é
/// o mesmo "ocupado" do `ACCESS_DENIED`: o `iperf3 -s` (OpenWrt 25.12, iperf3
/// 3.20) ainda liberando a fase anterior derruba a conexão que chegou cedo —
/// visto em equipamento real na volta, logo depois da ida. Nada foi medido:
/// tentar de novo é seguro. O prazo estourado continua sendo erro (servidor
/// mudo, não ocupado).
fn closed_early(error: AppError) -> AppError {
    match error {
        AppError::ServiceUnavailable(_) => AppError::conflict(
            "O servidor iperf3 fechou a conexão antes de começar o teste (ainda liberando o anterior)",
        ),
        other => other,
    }
}

/// Estados de erro que podem chegar a qualquer momento.
async fn state_error(ctrl: &mut TcpStream, state: State) -> AppError {
    match state {
        State::AccessDenied => {
            AppError::conflict("O servidor iperf3 recusou o teste: está ocupado com outro cliente")
        }
        State::ServerError => wire::read_server_error(ctrl).await,
        State::ServerTerminate => {
            AppError::service_unavailable("O servidor iperf3 encerrou o teste antes do fim")
        }
        other => wire::malformed(&format!("estado {other:?} fora de hora")),
    }
}

async fn drive(
    ctrl: &mut TcpStream,
    addr: SocketAddr,
    spec: &ThroughputSpec,
    reverse: bool,
    sink: &SampleSink,
    offset_ms: u64,
) -> AppResult<PhaseOutcome> {
    let cookie = wire::make_cookie();
    step(async {
        ctrl.write_all(&cookie)
            .await
            .map_err(|_| wire::control_closed())
    })
    .await?;

    // 1. Handshake até o TEST_RUNNING.
    let mut streams: Vec<TcpStream> = Vec::with_capacity(usize::from(spec.streams));
    loop {
        match step(async { wire::read_state(ctrl).await.map_err(closed_early) }).await? {
            State::ParamExchange => {
                let params =
                    wire::params_json(spec.duration_s, spec.streams, reverse, spec.limit_bps());
                step(wire::write_json(ctrl, &params)).await?;
            }
            State::CreateStreams => {
                for _ in streams.len()..usize::from(spec.streams) {
                    let mut stream = connect(addr).await?;
                    step(async {
                        stream.write_all(&cookie).await.map_err(|_| {
                            AppError::service_unavailable(
                                "O servidor iperf3 recusou um fluxo de dados",
                            )
                        })
                    })
                    .await?;
                    streams.push(stream);
                }
            }
            State::TestStart | State::IperfStart => {}
            State::TestRunning => break,
            other => return Err(state_error(ctrl, other).await),
        }
    }
    if streams.len() != usize::from(spec.streams) {
        return Err(wire::malformed("teste iniciado sem os fluxos de dados"));
    }

    // 2. Dados durante a duração.
    let total = ByteCounter::default();
    let per_stream: Vec<ByteCounter> = streams.iter().map(|_| ByteCounter::default()).collect();
    let stop_senders = CancellationToken::new();
    let mut senders: JoinSet<TcpStream> = JoinSet::new();
    let mut receivers: JoinSet<()> = JoinSet::new();
    let per_stream_bps = spec
        .limit_bps()
        .map(|bps| (bps / u64::from(spec.streams)).max(1));
    for (stream, counter) in streams.into_iter().zip(per_stream.iter()) {
        let counters = (counter.clone(), total.clone());
        if reverse {
            receivers.spawn(receive_loop(stream, counters));
        } else {
            let pacer = Pacer::new(per_stream_bps);
            senders.spawn(send_loop(stream, counters, pacer, stop_senders.clone()));
        }
    }
    let flow = if reverse { Flow::Rx } else { Flow::Tx };
    let sampler = Sampler::spawn(vec![(flow, total.clone())], sink.clone(), offset_ms);
    let started = Instant::now();
    tokio::select! {
        state = wire::read_state(ctrl) => {
            let state = state?;
            return Err(state_error(ctrl, state).await);
        }
        () = tokio::time::sleep_until(started + spec.duration()) => {}
    }

    // 3. Fim: para os envios (mantendo os sockets abertos até o fim, como o
    // iperf3 faz), fotografa os contadores e avisa o servidor.
    stop_senders.cancel();
    let mut parked = Vec::new();
    while let Some(joined) = senders.join_next().await {
        if let Ok(stream) = joined {
            parked.push(stream);
        }
    }
    let elapsed = started.elapsed();
    let bytes: Vec<u64> = per_stream.iter().map(ByteCounter::get).collect();
    let samples = sampler.finish().await;
    step(wire::write_state(ctrl, State::TestEnd)).await?;

    // 4. Troca de resultados. Os receptores seguem drenando até o fim.
    let mut server = None;
    loop {
        match step(wire::read_state(ctrl)).await? {
            State::ExchangeResults => {
                let ours = wire::results_json(&bytes, !reverse, elapsed.as_secs_f64());
                step(wire::write_json(ctrl, &ours)).await?;
                let theirs = step(wire::read_json(ctrl)).await?;
                server = Some(wire::parse_server_results(&theirs)?);
            }
            State::DisplayResults => {
                let server =
                    server.ok_or_else(|| wire::malformed("fim sem troca de resultados"))?;
                receivers.abort_all();
                drop(parked);
                // O servidor pode fechar antes de ler o IPERF_DONE: tanto faz.
                let _ = step(wire::write_state(ctrl, State::IperfDone)).await;
                return Ok(PhaseOutcome {
                    local_bytes: bytes.iter().sum(),
                    elapsed,
                    server,
                    samples,
                });
            }
            State::IperfDone | State::TestRunning => {}
            other => return Err(state_error(ctrl, other).await),
        }
    }
}

/// Envia blocos até `stop`; devolve o socket para ficar aberto até o fim.
async fn send_loop(
    mut stream: TcpStream,
    (per_stream, total): (ByteCounter, ByteCounter),
    mut pacer: Pacer,
    stop: CancellationToken,
) -> TcpStream {
    let block = vec![0u8; DEFAULT_TCP_BLKSIZE];
    loop {
        let written = tokio::select! {
            () = stop.cancelled() => break,
            written = stream.write(&block) => match written {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            },
        };
        per_stream.add(written);
        total.add(written);
        tokio::select! {
            () = stop.cancelled() => break,
            () = pacer.pace(written) => {}
        }
    }
    stream
}

/// Recebe até o servidor fechar ou a tarefa ser abortada.
async fn receive_loop(mut stream: TcpStream, (per_stream, total): (ByteCounter, ByteCounter)) {
    let mut buf = vec![0u8; DEFAULT_TCP_BLKSIZE];
    while let Ok(n) = stream.read(&mut buf).await {
        if n == 0 {
            break;
        }
        per_stream.add(n);
        total.add(n);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        fake_server::{Behavior, FakeServer},
        *,
    };
    use crate::services::network_tools::throughput::{
        is_cancelled, sample_channel, TestDirection, TestProtocol,
    };

    fn spec(direction: TestDirection, streams: u8) -> ThroughputSpec {
        ThroughputSpec {
            engine: ThroughputEngine::Iperf3,
            direction,
            protocol: TestProtocol::Tcp,
            duration_s: 1,
            streams,
            limit_mbps: None,
        }
    }

    async fn run(server: &FakeServer, spec: &ThroughputSpec) -> AppResult<ThroughputReport> {
        let (sink, _rx) = sample_channel();
        timeout(
            Duration::from_secs(5),
            run_client(server.addr, spec, &sink, &CancellationToken::new()),
        )
        .await
        .expect("teste dentro de 5 s")
    }

    #[tokio::test]
    async fn envia_e_usa_os_bytes_do_servidor() {
        let server = FakeServer::start(Behavior::Normal).await;
        let (sink, mut amostras) = sample_channel();
        let report = run_client(
            server.addr,
            &spec(TestDirection::Send, 1),
            &sink,
            &CancellationToken::new(),
        )
        .await
        .expect("teste ok");
        let visto = server.last_test().await;
        assert!(!visto.reverse);
        assert_eq!(visto.parallel, 1);
        assert_eq!(report.engine, ThroughputEngine::Iperf3);
        assert!(report.tx_bytes > 0);
        assert_eq!(report.tx_bytes, visto.bytes, "tx vem do servidor");
        assert!(report.tx_bps.unwrap_or_default() > 0.0);
        assert_eq!(
            (report.rx_bps, report.rx_bytes, report.retransmits),
            (None, 0, None)
        );
        assert!(!report.samples.is_empty());
        assert!(report.samples.iter().all(|s| s.flow == Flow::Tx));
        assert_eq!(amostras.try_recv().ok(), report.samples.first().copied());
    }

    #[tokio::test]
    async fn recebe_em_reverse_com_retransmissoes() {
        let server = FakeServer::start(Behavior::Normal).await;
        let report = run(&server, &spec(TestDirection::Receive, 1))
            .await
            .expect("ok");
        let visto = server.last_test().await;
        assert!(visto.reverse);
        assert!(report.rx_bytes > 0);
        assert!(report.rx_bps.unwrap_or_default() > 0.0);
        assert_eq!((report.tx_bps, report.tx_bytes), (None, 0));
        assert_eq!(report.retransmits, Some(7));
        assert!(report.samples.iter().all(|s| s.flow == Flow::Rx));
    }

    #[tokio::test]
    async fn dois_sentidos_em_duas_fases() {
        let server = FakeServer::start(Behavior::Normal).await;
        let report = run(&server, &spec(TestDirection::Both, 2))
            .await
            .expect("ok");
        assert_eq!(server.tests().await, 2);
        assert!(report.tx_bytes > 0 && report.rx_bytes > 0);
        assert_eq!(report.retransmits, Some(14), "7 por fluxo, 2 fluxos");
        let primeira_rx = report
            .samples
            .iter()
            .find(|s| s.flow == Flow::Rx)
            .expect("amostra de rx");
        assert!(
            primeira_rx.t_ms >= 1_000,
            "segunda fase deslocada: {primeira_rx:?}"
        );
    }

    #[tokio::test]
    async fn varios_fluxos_com_limite() {
        let server = FakeServer::start(Behavior::Normal).await;
        let s = ThroughputSpec {
            limit_mbps: Some(40),
            ..spec(TestDirection::Send, 4)
        };
        let report = run(&server, &s).await.expect("ok");
        let visto = server.last_test().await;
        assert_eq!(visto.parallel, 4);
        assert_eq!(visto.stream_ids, vec![1, 3, 4, 5]);
        assert_eq!(visto.bandwidth, Some(10_000_000));
        // 40 Mbit/s por 1 s ≈ 5 MB; folga para o arranque.
        assert!(
            report.tx_bytes > 0 && report.tx_bytes < 8_000_000,
            "{}",
            report.tx_bytes
        );
    }

    #[tokio::test]
    async fn servidor_ocupado_por_um_instante_tenta_de_novo() {
        // A volta conecta logo depois da ida e o servidor ainda não liberou a
        // vaga: o cliente espera e tenta de novo, sem perder a medida.
        let server = FakeServer::start(Behavior::BusyOnce).await;
        let report = run(&server, &spec(TestDirection::Both, 1))
            .await
            .expect("tenta de novo depois do ocupado");
        assert_eq!(server.tests().await, 2);
        assert!(report.tx_bps.is_some() && report.rx_bps.is_some());
    }

    #[tokio::test]
    async fn servidor_que_fecha_antes_de_comecar_e_tentado_de_novo() {
        let server = FakeServer::start(Behavior::ClosedOnce).await;
        let report = run(&server, &spec(TestDirection::Both, 1))
            .await
            .expect("a volta tenta de novo depois do fechamento");
        assert_eq!(server.tests().await, 2);
        assert!(report.tx_bps.is_some() && report.rx_bps.is_some());
    }

    #[tokio::test]
    async fn servidor_ocupado_vira_conflito() {
        let server = FakeServer::start(Behavior::AccessDenied).await;
        let err = run(&server, &spec(TestDirection::Send, 1))
            .await
            .expect_err("negado");
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
    }

    #[tokio::test]
    async fn erro_do_servidor_traz_os_codigos() {
        let server = FakeServer::start(Behavior::ServerError).await;
        let err = run(&server, &spec(TestDirection::Send, 1))
            .await
            .expect_err("erro");
        let AppError::BusinessRule(msg) = err else {
            panic!("esperava BusinessRule");
        };
        assert!(
            msg.contains("código 118") && msg.contains("errno 2"),
            "{msg}"
        );
    }

    #[tokio::test]
    async fn resultados_malformados_sao_recusados() {
        let server = FakeServer::start(Behavior::MalformedResults).await;
        let err = run(&server, &spec(TestDirection::Send, 1))
            .await
            .expect_err("malformado");
        assert!(
            matches!(&err, AppError::BusinessRule(m) if m.contains("inválida")),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn estado_desconhecido_e_recusado() {
        let server = FakeServer::start(Behavior::BogusState).await;
        let err = run(&server, &spec(TestDirection::Send, 1))
            .await
            .expect_err("estado");
        assert!(
            matches!(&err, AppError::BusinessRule(m) if m.contains("desconhecido")),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn servidor_mudo_estoura_o_prazo_do_handshake() {
        let server = FakeServer::start(Behavior::Silent).await;
        let inicio = Instant::now();
        let s = spec(TestDirection::Send, 1);
        let (sink, _rx) = sample_channel();
        let err = run_client(server.addr, &s, &sink, &CancellationToken::new())
            .await
            .expect_err("mudo");
        assert!(matches!(err, AppError::ServiceUnavailable(_)), "{err:?}");
        assert!(inicio.elapsed() < Duration::from_secs(6));
    }

    #[tokio::test]
    async fn cancelamento_interrompe_e_avisa_o_servidor() {
        let server = FakeServer::start(Behavior::Normal).await;
        let s = ThroughputSpec {
            duration_s: 30,
            ..spec(TestDirection::Send, 1)
        };
        let cancel = CancellationToken::new();
        let (sink, _rx) = sample_channel();
        let gatilho = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            gatilho.cancel();
        });
        let inicio = Instant::now();
        let err = run_client(server.addr, &s, &sink, &cancel)
            .await
            .expect_err("cancelado");
        assert!(is_cancelled(&err), "{err:?}");
        assert!(inicio.elapsed() < Duration::from_secs(2));
        assert!(server.client_terminated().await, "CLIENT_TERMINATE enviado");
    }

    #[tokio::test]
    async fn sem_servidor_e_indisponivel() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        drop(listener);
        let (sink, _rx) = sample_channel();
        let err = run_client(
            addr,
            &spec(TestDirection::Send, 1),
            &sink,
            &CancellationToken::new(),
        )
        .await
        .expect_err("recusado");
        assert!(matches!(err, AppError::ServiceUnavailable(_)), "{err:?}");
    }

    #[tokio::test]
    async fn recusa_spec_de_outro_motor() {
        let s = ThroughputSpec {
            engine: ThroughputEngine::Btest,
            ..spec(TestDirection::Send, 1)
        };
        let (sink, _rx) = sample_channel();
        let addr: SocketAddr = "127.0.0.1:9".parse().expect("addr");
        let err = run_client(addr, &s, &sink, &CancellationToken::new())
            .await
            .expect_err("motor");
        assert!(matches!(err, AppError::Validation(_)));
    }
}
