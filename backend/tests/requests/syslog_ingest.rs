//! Ingestão de syslog de ponta a ponta: socket → parser → resolvedor → banco.
//!
//! Os testes unitários cobrem cada peça isolada; o que só aparece aqui é a
//! junção — enquadramento de TCP casando com o parser, o `SocketAddr` virando
//! `source_ip`, o escritor descarregando de verdade.
//!
//! **Porta 0 em tudo.** Porta fixa colidiria entre testes rodando em paralelo,
//! e 5514 colidiria com um servidor de desenvolvimento aberto na mesma
//! máquina. O sistema atribui uma efêmera e o teste lê de volta qual foi.
//!
//! O inventário é um SQLite em memória próprio, e não o banco de teste
//! compartilhado: assim estes testes não semeiam `networks` que outro teste
//! veria.

use std::{net::IpAddr, sync::Arc, time::Duration};

use backend::{
    models::logs::device_logs,
    services::syslog::{
        config::SyslogConfig,
        db,
        ingest::Ingestor,
        listener,
        queue::{IngestMetrics, LogQueue},
        sources::SourceRegistry,
        writer,
    },
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectOptions, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait,
};
use serial_test::serial;
use tokio::{io::AsyncWriteExt, net::UdpSocket};

/// Teto de espera de qualquer teste de rede daqui (`AGENTS.md` §4).
const TIMEOUT: Duration = Duration::from_secs(5);

/// Inventário em memória. `com_rede` decide se `127.0.0.1` é fonte conhecida.
async fn inventario(com_rede: bool) -> DatabaseConnection {
    let db = Database::connect(
        ConnectOptions::new("sqlite::memory:".to_owned())
            .max_connections(1)
            .min_connections(1)
            .to_owned(),
    )
    .await
    .expect("banco de inventário");
    Migrator::up(&db, None).await.expect("migrations");

    if com_rede {
        backend::models::_entities::networks::ActiveModel {
            name: Set("loopback".into()),
            cidr: Set("127.0.0.0/8".into()),
            scan_enabled: Set(false),
            scan_interval: Set(3600),
            active: Set(true),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("rede de teste");
    }
    db
}

/// Monta o pipeline inteiro e devolve o banco de logs e o ingestor.
async fn pipeline(com_rede: bool, config: SyslogConfig) -> (DatabaseConnection, Ingestor) {
    std::env::remove_var("SYSLOG_DB_URL");
    let logs = db::connect("sqlite::memory:")
        .await
        .expect("banco de logs")
        .connection()
        .clone();
    let metrics = Arc::new(IngestMetrics::default());
    let (queue, receiver) = LogQueue::create(1024, Arc::clone(&metrics));
    let ingestor = Ingestor::new(
        inventario(com_rede).await,
        config,
        queue,
        Arc::new(SourceRegistry::create()),
    );

    // Lote de 1 linha para o teste não depender do relógio: cada mensagem
    // desce assim que chega.
    let escritor = logs.clone();
    tokio::spawn(async move {
        let mut receiver = receiver;
        writer::run_with(
            escritor,
            &mut receiver,
            metrics,
            1,
            Duration::from_millis(10),
        )
        .await;
    });

    (logs, ingestor)
}

/// Espera a linha chegar ao banco, ou desiste dentro do teto.
async fn aguarda_linhas(logs: &DatabaseConnection, esperadas: u64) -> u64 {
    let prazo = tokio::time::Instant::now() + TIMEOUT;
    loop {
        let total = device_logs::Entity::find()
            .count(logs)
            .await
            .expect("contagem");
        if total >= esperadas || tokio::time::Instant::now() >= prazo {
            return total;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn config_de_teste() -> SyslogConfig {
    SyslogConfig {
        udp_port: 0,
        tcp_port: 0,
        ..SyslogConfig::default()
    }
}

#[tokio::test]
#[serial]
async fn o_datagrama_udp_atravessa_ate_o_banco() {
    let (logs, ingestor) = pipeline(true, config_de_teste()).await;
    let porta = listener::spawn_udp(0, ingestor)
        .await
        .expect("listener UDP");
    assert_ne!(porta, 0, "o sistema tinha de atribuir uma porta efêmera");

    let cliente = UdpSocket::bind("127.0.0.1:0").await.expect("socket");
    cliente
        .send_to(
            b"<131>Aug 15 10:24:01 MikroTik-CCR system,error,critical login failure for admin",
            ("127.0.0.1", porta),
        )
        .await
        .expect("envio");

    assert_eq!(aguarda_linhas(&logs, 1).await, 1);
    let linha = device_logs::Entity::find()
        .one(&logs)
        .await
        .expect("consulta")
        .expect("linha");
    assert_eq!(linha.source_ip, "127.0.0.1");
    assert_eq!(linha.message, "login failure for admin");
    assert_eq!(linha.topics.as_deref(), Some("system,error,critical"));
    // A severidade sai dos tópicos, não do `<131>` — ver ADR 008.
    assert_eq!(linha.severity, Some(2));
    // Rede cadastrada sem dispositivo: fonte legítima, sem vínculo.
    assert_eq!(linha.device_id, None);
}

#[tokio::test]
#[serial]
async fn a_fonte_fora_de_qualquer_rede_cadastrada_nao_grava() {
    // Sem `networks`, o loopback não resolve para nada. É a regra que impede um
    // host solto de encher o disco.
    let (logs, ingestor) = pipeline(false, config_de_teste()).await;
    let metrics = Arc::clone(ingestor.metrics());
    let porta = listener::spawn_udp(0, ingestor)
        .await
        .expect("listener UDP");

    let cliente = UdpSocket::bind("127.0.0.1:0").await.expect("socket");
    cliente
        .send_to(
            b"<134>Aug 15 10:23:45 host app: intruso",
            ("127.0.0.1", porta),
        )
        .await
        .expect("envio");

    // Espera até o contador acusar o descarte — assim o teste não passa por
    // ainda não ter chegado nada.
    let prazo = tokio::time::Instant::now() + TIMEOUT;
    while metrics.snapshot().dropped_unknown_source == 0 {
        assert!(tokio::time::Instant::now() < prazo, "a linha nunca chegou");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        device_logs::Entity::find()
            .count(&logs)
            .await
            .expect("contagem"),
        0,
        "fonte desconhecida não pode gravar"
    );
}

#[tokio::test]
#[serial]
async fn o_tcp_le_as_duas_molduras_do_rfc6587_na_mesma_conexao() {
    let (logs, ingestor) = pipeline(true, config_de_teste()).await;
    let porta = listener::spawn_tcp(0, ingestor)
        .await
        .expect("listener TCP");

    let mut conexao = tokio::time::timeout(
        TIMEOUT,
        tokio::net::TcpStream::connect(("127.0.0.1", porta)),
    )
    .await
    .expect("prazo de conexão")
    .expect("conexão");

    // Delimitada por LF, depois contagem de octetos — a mesma conexão tem de
    // aguentar as duas, porque rsyslog e o resto do mundo não combinam entre si.
    conexao
        .write_all(b"<134>Aug 15 10:23:45 host app: primeira\n")
        .await
        .expect("escrita");
    conexao
        .write_all(b"38 <134>Aug 15 10:23:46 host app: segunda")
        .await
        .expect("escrita");
    conexao.flush().await.expect("flush");

    assert_eq!(aguarda_linhas(&logs, 2).await, 2);
    let mensagens: Vec<String> = device_logs::Entity::find()
        .all(&logs)
        .await
        .expect("consulta")
        .into_iter()
        .map(|linha| linha.message)
        .collect();
    assert!(mensagens.contains(&"primeira".to_string()), "{mensagens:?}");
    assert!(mensagens.contains(&"segunda".to_string()), "{mensagens:?}");
}

#[tokio::test]
#[serial]
async fn a_ultima_linha_sem_delimitador_e_gravada_no_fechamento() {
    // Cliente que abre a conexão, manda uma linha sem LF e fecha. Descartar
    // essa linha perderia justamente a mensagem que motivou a conexão.
    let (logs, ingestor) = pipeline(true, config_de_teste()).await;
    let porta = listener::spawn_tcp(0, ingestor)
        .await
        .expect("listener TCP");

    let mut conexao = tokio::time::timeout(
        TIMEOUT,
        tokio::net::TcpStream::connect(("127.0.0.1", porta)),
    )
    .await
    .expect("prazo de conexão")
    .expect("conexão");
    conexao
        .write_all(b"<134>Aug 15 10:23:45 host app: sem quebra")
        .await
        .expect("escrita");
    conexao.shutdown().await.expect("fechamento");

    assert_eq!(aguarda_linhas(&logs, 1).await, 1);
}

#[tokio::test]
#[serial]
async fn o_ip_de_origem_observado_e_o_do_remetente() {
    // O teste que a ADR 008 deixou em aberto para o Docker, feito aqui no que é
    // possível: em loopback, `source_ip` **tem** de ser o IP de quem enviou.
    // Se algum dia isto quebrar, a regra da fonte conhecida cai junto.
    let (logs, ingestor) = pipeline(true, config_de_teste()).await;
    let porta = listener::spawn_udp(0, ingestor)
        .await
        .expect("listener UDP");

    let cliente = UdpSocket::bind("127.0.0.1:0").await.expect("socket");
    let origem: IpAddr = cliente.local_addr().expect("endereço").ip();
    cliente
        .send_to(
            b"<134>Aug 15 10:23:45 host app: teste",
            ("127.0.0.1", porta),
        )
        .await
        .expect("envio");

    assert_eq!(aguarda_linhas(&logs, 1).await, 1);
    let linha = device_logs::Entity::find()
        .one(&logs)
        .await
        .expect("consulta")
        .expect("linha");
    assert_eq!(linha.source_ip, origem.to_string());
}
