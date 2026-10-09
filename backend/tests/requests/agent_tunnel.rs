//! Ponte de banco pelo agente (ADR 013) sem banco nenhum: um servidor TCP
//! local faz o papel do banco da filial.

use std::time::{Duration, Instant};

use backend::{
    app::App,
    services::agents::{
        bridge::LocalBridge,
        hub::AgentHub,
        protocol::{Command, DockerCall, ErrorCode},
    },
};
use loco_rs::testing::prelude::*;
use serial_test::serial;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use super::agent_harness::{connect_tunnel_agent, enrolled_agent, tunnel_agent};

/// "Banco" que manda `size` bytes assim que alguém conecta.
async fn flood_server(size: usize) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let chunk = vec![42_u8; 64 * 1024];
                let mut left = size;
                while left > 0 {
                    let n = left.min(chunk.len());
                    if socket.write_all(&chunk[..n]).await.is_err() {
                        return;
                    }
                    left -= n;
                }
                let _ = socket.shutdown().await;
            });
        }
    });
    port
}

/// Um dump grande e um leitor lento: a janela segura o agente, e um pedido
/// comum do mesmo agente continua respondendo na hora — a fila de controle
/// passa na frente dos quadros da ponte.
#[tokio::test]
#[serial]
async fn ponte_cheia_nao_atrasa_os_outros_pedidos_do_agente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        const SIZE: usize = 48 * 1024 * 1024;
        let port = flood_server(SIZE).await;
        let probe = tunnel_agent(&ctx, "filial", &format!("127.0.0.1:{port}")).await;
        let session = AgentHub::from_context(&ctx).unwrap().get(probe.id).unwrap();

        let hub = AgentHub::from_context(&ctx).unwrap();
        let bridge = LocalBridge::start(hub, probe.id, "127.0.0.1".into(), port)
            .await
            .expect("ponte aberta");
        let mut client = TcpStream::connect(bridge.addr()).await.unwrap();

        // Lê devagar: a ponte fica cheia o tempo todo.
        let reader = tokio::spawn(async move {
            let mut buffer = vec![0_u8; 64 * 1024];
            let mut total = 0;
            loop {
                let n = client.read(&mut buffer).await.unwrap();
                if n == 0 {
                    break total;
                }
                total += n;
                if total < SIZE / 2 {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;
        for _ in 0..5 {
            let started = Instant::now();
            let error = session
                .request(
                    Command::Docker {
                        call: DockerCall::ListContainers,
                    },
                    Duration::from_secs(5),
                )
                .await
                .expect_err("o executor de teste recusa tudo");
            assert_eq!(error.code, ErrorCode::Unsupported, "{error}");
            assert!(
                started.elapsed() < Duration::from_millis(500),
                "pedido esperou {:?} atrás da ponte",
                started.elapsed()
            );
        }

        assert_eq!(reader.await.unwrap(), SIZE, "nenhum byte se perdeu");
    })
    .await;
}

/// O banco fecha a conexão: a ponte fecha do lado da central também, e a vaga
/// do agente volta (no máximo 2 pontes simultâneas).
#[tokio::test]
#[serial]
async fn ponte_encerrada_libera_a_vaga_do_agente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let port = flood_server(1024).await;
        let probe = tunnel_agent(&ctx, "filial", &format!("127.0.0.1:{port}")).await;
        let hub = AgentHub::from_context(&ctx).unwrap();

        for _ in 0..4 {
            let bridge = LocalBridge::start(hub.clone(), probe.id, "127.0.0.1".into(), port)
                .await
                .expect("vaga livre de novo");
            let mut client = TcpStream::connect(bridge.addr()).await.unwrap();
            let mut received = Vec::new();
            client.read_to_end(&mut received).await.unwrap();
            assert_eq!(received.len(), 1024);
            drop(client);
            drop(bridge);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
}

/// O canal com o agente cai e ele reconecta: a mesma ponte local passa a
/// abrir as conexões novas pela conexão nova — é o que deixa o backup refazer
/// um banco sem reabrir nada.
#[tokio::test]
#[serial]
async fn ponte_segue_o_agente_depois_da_reconexao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let port = flood_server(1024).await;
        let targets = format!("127.0.0.1:{port}");
        let (probe, _) = enrolled_agent(&ctx, "filial").await;
        let first = connect_tunnel_agent(&ctx, &probe, &targets).await;
        let hub = AgentHub::from_context(&ctx).unwrap();
        let old = hub.get(probe.id).unwrap();

        let bridge = LocalBridge::start(hub.clone(), probe.id, "127.0.0.1".into(), port)
            .await
            .expect("ponte aberta");

        // Queda: o canal some antes de a primeira ponte ser usada.
        first.abort();
        tokio::time::timeout(Duration::from_secs(2), async {
            while !old.is_closed() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("a central percebe a queda");
        connect_tunnel_agent(&ctx, &probe, &targets).await;

        for _ in 0..2 {
            let mut client = TcpStream::connect(bridge.addr()).await.unwrap();
            let mut received = Vec::new();
            client.read_to_end(&mut received).await.unwrap();
            assert_eq!(received.len(), 1024, "passou pela conexão nova");
        }
    })
    .await;
}
