//! Servidor iperf3 mínimo, em processo, para os testes do cliente.
//!
//! Segue o lado servidor do protocolo (`iperf_server_api.c`) só o bastante
//! para exercitar o cliente: cookie, parâmetros, fluxos, `TEST_END`, troca de
//! resultados. Atende um teste por vez, em sequência — como o `iperf3 -s`
//! sem `-1`. Escuta só em `127.0.0.1`.

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;

use super::wire::{self, COOKIE_SIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Behavior {
    Normal,
    AccessDenied,
    ServerError,
    MalformedResults,
    BogusState,
    Silent,
}

/// O que o servidor viu de um teste completo.
#[derive(Debug, Clone, Default)]
pub(super) struct SeenTest {
    pub reverse: bool,
    pub parallel: usize,
    pub bandwidth: Option<u64>,
    /// Ids citados nos resultados do cliente.
    pub stream_ids: Vec<u64>,
    /// Bytes informados ao cliente (recebidos, ou enviados em `reverse`).
    pub bytes: u64,
}

#[derive(Default)]
struct Shared {
    tests: Vec<SeenTest>,
    client_terminated: bool,
}

pub(super) struct FakeServer {
    pub addr: SocketAddr,
    shared: Arc<Mutex<Shared>>,
}

impl FakeServer {
    pub(super) async fn start(behavior: Behavior) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind do servidor falso");
        let addr = listener.local_addr().expect("endereço local");
        let shared = Arc::new(Mutex::new(Shared::default()));
        let state = shared.clone();
        tokio::spawn(async move {
            while let Ok((ctrl, _)) = listener.accept().await {
                let _ = handle(ctrl, &listener, behavior, &state).await;
            }
        });
        Self { addr, shared }
    }

    pub(super) async fn last_test(&self) -> SeenTest {
        self.shared
            .lock()
            .await
            .tests
            .last()
            .cloned()
            .expect("um teste completo")
    }

    pub(super) async fn tests(&self) -> usize {
        self.shared.lock().await.tests.len()
    }

    /// O cliente mandou `CLIENT_TERMINATE` (espera até 2 s pela tarefa).
    pub(super) async fn client_terminated(&self) -> bool {
        for _ in 0..40 {
            if self.shared.lock().await.client_terminated {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }
}

async fn handle(
    mut ctrl: TcpStream,
    listener: &TcpListener,
    behavior: Behavior,
    shared: &Mutex<Shared>,
) -> std::io::Result<()> {
    let mut cookie = [0u8; COOKIE_SIZE];
    ctrl.read_exact(&mut cookie).await?;
    match behavior {
        Behavior::Silent => {
            tokio::time::sleep(Duration::from_secs(10)).await;
            return Ok(());
        }
        Behavior::AccessDenied => return ctrl.write_i8(-1).await,
        _ => {}
    }

    ctrl.write_i8(9).await?;
    let params = wire::read_json(&mut ctrl).await.map_err(io_err)?;
    match behavior {
        Behavior::ServerError => {
            ctrl.write_i8(-2).await?;
            ctrl.write_i32(118).await?;
            return ctrl.write_i32(2).await;
        }
        Behavior::BogusState => return ctrl.write_i8(42).await,
        _ => {}
    }
    let parallel = params["parallel"].as_u64().unwrap_or(1) as usize;
    let reverse = params["reverse"] == Value::Bool(true);
    let bandwidth = params["bandwidth"].as_u64();

    ctrl.write_i8(10).await?;
    let mut data = Vec::with_capacity(parallel);
    for _ in 0..parallel {
        let (mut stream, _) = listener.accept().await?;
        let mut stream_cookie = [0u8; COOKIE_SIZE];
        stream.read_exact(&mut stream_cookie).await?;
        assert_eq!(stream_cookie, cookie, "fluxo com o cookie do controle");
        data.push(stream);
    }
    ctrl.write_i8(1).await?;
    ctrl.write_i8(2).await?;

    let stop = CancellationToken::new();
    let counters: Vec<Arc<AtomicU64>> = (0..parallel).map(|_| Arc::default()).collect();
    for (mut stream, counter) in data.into_iter().zip(counters.iter().cloned()) {
        let stop = stop.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let moved = if reverse {
                    tokio::select! {
                        () = stop.cancelled() => break,
                        r = stream.write(&buf) => r,
                    }
                } else {
                    stream.read(&mut buf).await
                };
                match moved {
                    Ok(0) | Err(_) => break,
                    Ok(n) => counter.fetch_add(n as u64, Ordering::Relaxed),
                };
            }
        });
    }

    match ctrl.read_i8().await {
        Ok(4) => {}
        Ok(12) => {
            shared.lock().await.client_terminated = true;
            return Ok(());
        }
        _ => return Ok(()),
    }
    stop.cancel();
    let per_stream: Vec<u64> = counters.iter().map(|c| c.load(Ordering::Relaxed)).collect();

    ctrl.write_i8(13).await?;
    let client = wire::read_json(&mut ctrl).await.map_err(io_err)?;
    let stream_ids = client["streams"]
        .as_array()
        .map(|s| s.iter().filter_map(|x| x["id"].as_u64()).collect())
        .unwrap_or_default();
    let ours = if behavior == Behavior::MalformedResults {
        json!({"cpu_util_total": 0})
    } else {
        let streams: Vec<Value> = per_stream
            .iter()
            .enumerate()
            .map(|(i, bytes)| {
                json!({
                    "id": wire::stream_id(i),
                    "bytes": bytes,
                    "retransmits": if reverse { 7 } else { -1 },
                    "jitter": 0, "errors": 0, "packets": 0,
                })
            })
            .collect();
        json!({
            "cpu_util_total": 1.0, "cpu_util_user": 0.5, "cpu_util_system": 0.5,
            "sender_has_retransmits": if reverse { 1 } else { -1 },
            "streams": streams,
        })
    };
    wire::write_json(&mut ctrl, &ours).await.map_err(io_err)?;
    shared.lock().await.tests.push(SeenTest {
        reverse,
        parallel,
        bandwidth,
        stream_ids,
        bytes: per_stream.iter().sum(),
    });
    ctrl.write_i8(14).await?;
    let _ = ctrl.read_i8().await;
    Ok(())
}

fn io_err(error: crate::services::shared::errors::AppError) -> std::io::Error {
    std::io::Error::other(error.to_string())
}
