//! MikroTik Bandwidth Test (btest) em TCP — servidor e cliente nativos
//! (ADR 015 §1).
//!
//! * [`BtestServer`] — o servidor da **central**, aberto pelo
//!   `initializers::btest` em `BTEST_LISTEN` (padrão `0.0.0.0:2000`) e
//!   obtido com [`BtestServer::from_context`]. Só atende sessões armadas
//!   ([`BtestServer::arm`]) e responde "sem autenticação".
//! * [`run_client`] — o cliente do **agente**: sem autenticação ou MD5
//!   (RouterOS < 6.43); EC-SRP5 é recusado com erro próprio.
//!
//! Protocolo reconstruído pelo `btest-opensource` (Alex Samorukov, MIT) —
//! ver [`codec`]. **Limites conhecidos**, por não haver como validar contra
//! um RouterOS aqui: uma conexão TCP só (`tcp-connection-count=1`), sem UDP,
//! e as mensagens de estado intercaladas com os dados seguem o que o
//! `btest-opensource` e o `btest-rs` descrevem das capturas.

mod client;
mod codec;
mod duplex;
mod server;

use std::{fmt, net::SocketAddr, time::Duration};

pub use client::run_client;
pub use server::{ArmedSession, BtestServer, SessionOutcome};

use crate::services::shared::errors::AppError;

/// Porta padrão do `bandwidth-server`.
pub const DEFAULT_PORT: u16 = 2000;
/// Variável de ambiente com o endereço de escuta da central.
pub const LISTEN_ENV: &str = "BTEST_LISTEN";
/// Escuta padrão da central.
pub const DEFAULT_LISTEN: &str = "0.0.0.0:2000";
/// Validade padrão de uma sessão armada.
pub const DEFAULT_ARM_TTL: Duration = Duration::from_secs(90);
/// Erro de quem pede o servidor quando o processo não o abriu.
pub const UNAVAILABLE_MESSAGE: &str = "servidor btest indisponível";
/// O servidor exige EC-SRP5 (RouterOS ≥ 6.43 com autenticação).
pub const EC_SRP5_MESSAGE: &str = "O servidor btest exige autenticação EC-SRP5 (RouterOS 6.43 ou \
     mais novo), que o cliente nativo não fala: desligue a autenticação do bandwidth-server do \
     MikroTik durante a janela do teste";
/// O servidor pediu MD5 e não havia credencial.
pub const AUTH_REQUIRED_MESSAGE: &str =
    "O servidor btest exige autenticação e nenhuma credencial foi informada";
/// O servidor recusou a credencial MD5.
pub const AUTH_FAILED_MESSAGE: &str = "O servidor btest recusou o usuário ou a senha do teste";

/// Credencial do btest (MD5). Gerada pelo orquestrador a cada teste.
#[derive(Clone)]
pub struct BtestAuth {
    pub user: String,
    pub password: String,
}

impl fmt::Debug for BtestAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BtestAuth")
            .field("user", &self.user)
            .field("password", &"<oculta>")
            .finish()
    }
}

/// O erro é a recusa do EC-SRP5.
#[must_use]
pub fn is_ec_srp5_unsupported(error: &AppError) -> bool {
    matches!(error, AppError::BusinessRule(msg) if msg == EC_SRP5_MESSAGE)
}

/// Endereço de escuta a partir do valor de `BTEST_LISTEN`.
///
/// Ausente → [`DEFAULT_LISTEN`]; vazio → `None` (servidor desligado).
///
/// # Errors
///
/// Texto do problema quando o valor não é um `ip:porta`.
pub fn listen_addr(value: Option<&str>) -> Result<Option<SocketAddr>, String> {
    let value = value.map_or(DEFAULT_LISTEN, str::trim);
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|_| format!("{LISTEN_ENV}={value} não é um endereço ip:porta"))
}

/// [`listen_addr`] lido do ambiente.
///
/// # Errors
///
/// Ver [`listen_addr`].
pub fn listen_addr_from_env() -> Result<Option<SocketAddr>, String> {
    listen_addr(std::env::var(LISTEN_ENV).ok().as_deref())
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr},
        time::Duration,
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
        time::timeout,
    };
    use tokio_util::sync::CancellationToken;

    use super::{codec, *};
    use crate::services::network_tools::throughput::{
        is_cancelled, sample_channel, Flow, TestDirection, TestProtocol, ThroughputEngine,
        ThroughputReport, ThroughputSpec,
    };

    const LOCAL: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    fn spec(direction: TestDirection) -> ThroughputSpec {
        ThroughputSpec {
            engine: ThroughputEngine::Btest,
            direction,
            protocol: TestProtocol::Tcp,
            duration_s: 1,
            streams: 1,
            limit_mbps: None,
        }
    }

    async fn server() -> BtestServer {
        BtestServer::bind("127.0.0.1:0".parse().expect("addr"))
            .await
            .expect("bind")
    }

    async fn client(
        server: &BtestServer,
        spec: &ThroughputSpec,
        auth: Option<&BtestAuth>,
    ) -> Result<ThroughputReport, AppError> {
        let (sink, _rx) = sample_channel();
        timeout(
            Duration::from_secs(5),
            run_client(
                server.local_addr(),
                spec,
                auth,
                &sink,
                &CancellationToken::new(),
            ),
        )
        .await
        .expect("cliente dentro de 5 s")
    }

    async fn loopback(direction: TestDirection) -> (ThroughputReport, ThroughputReport, usize) {
        let server = server().await;
        let mut session = server.arm(LOCAL, Duration::from_secs(5)).expect("armado");
        let client_report = client(&server, &spec(direction), None)
            .await
            .expect("cliente ok");
        let server_report = timeout(Duration::from_secs(5), session.outcome.wait())
            .await
            .expect("relatório dentro de 5 s")
            .expect("servidor ok");
        let mut ao_vivo = 0;
        while session.samples.try_recv().is_ok() {
            ao_vivo += 1;
        }
        server.shutdown();
        (client_report, server_report, ao_vivo)
    }

    #[tokio::test]
    async fn cliente_envia_ao_servidor_armado() {
        let (cliente, servidor, ao_vivo) = loopback(TestDirection::Send).await;
        assert!(cliente.tx_bytes > 0 && cliente.tx_bps.is_some());
        assert_eq!((cliente.rx_bps, cliente.rx_bytes), (None, 0));
        // Visto do cliente nos dois relatórios: o servidor recebeu = tx.
        assert!(servidor.tx_bytes > 0 && servidor.tx_bps.is_some());
        assert!(servidor.tx_bytes <= cliente.tx_bytes);
        assert_eq!(servidor.rx_bps, None);
        assert!(servidor.samples.iter().all(|s| s.flow == Flow::Tx));
        assert_eq!(ao_vivo, servidor.samples.len());
        assert!(cliente.samples.iter().all(|s| s.flow == Flow::Tx));
        assert_eq!(cliente.engine, ThroughputEngine::Btest);
    }

    #[tokio::test]
    async fn cliente_recebe_do_servidor_armado() {
        let (cliente, servidor, _) = loopback(TestDirection::Receive).await;
        assert!(cliente.rx_bytes > 0 && cliente.rx_bps.is_some());
        assert_eq!(cliente.tx_bps, None);
        assert!(servidor.rx_bytes > 0, "servidor enviou = rx do cliente");
        assert_eq!(servidor.tx_bps, None);
        assert!(servidor.samples.iter().all(|s| s.flow == Flow::Rx));
    }

    #[tokio::test]
    async fn dois_sentidos_ao_mesmo_tempo() {
        let (cliente, servidor, _) = loopback(TestDirection::Both).await;
        assert!(cliente.tx_bytes > 0 && cliente.rx_bytes > 0);
        assert!(servidor.tx_bytes > 0 && servidor.rx_bytes > 0);
        let fluxos: Vec<Flow> = cliente.samples.iter().map(|s| s.flow).collect();
        assert!(fluxos.contains(&Flow::Tx) && fluxos.contains(&Flow::Rx));
    }

    #[tokio::test]
    async fn limite_de_banda_vale_nos_dois_lados() {
        let server = server().await;
        let mut session = server.arm(LOCAL, Duration::from_secs(5)).expect("armado");
        let s = ThroughputSpec {
            limit_mbps: Some(8),
            ..spec(TestDirection::Both)
        };
        let cliente = client(&server, &s, None).await.expect("ok");
        let servidor = session.outcome.wait().await.expect("ok");
        // 8 Mbit/s por ~1 s ≈ 1 MB; folga para o primeiro bloco e o arranque.
        assert!(cliente.tx_bytes < 1_600_000, "{}", cliente.tx_bytes);
        assert!(servidor.rx_bytes < 1_600_000, "{}", servidor.rx_bytes);
    }

    #[tokio::test]
    async fn sem_sessao_armada_fecha_na_hora() {
        let server = server().await;
        let err = client(&server, &spec(TestDirection::Send), None)
            .await
            .expect_err("fechado");
        assert!(matches!(err, AppError::ServiceUnavailable(_)), "{err:?}");
    }

    #[tokio::test]
    async fn sessao_e_de_uso_unico() {
        let server = server().await;
        let mut session = server.arm(LOCAL, Duration::from_secs(5)).expect("armado");
        client(&server, &spec(TestDirection::Send), None)
            .await
            .expect("primeira ok");
        session.outcome.wait().await.expect("relatório");
        assert!(
            session.outcome.wait().await.is_err(),
            "relatório já consumido"
        );
        let err = client(&server, &spec(TestDirection::Send), None)
            .await
            .expect_err("segunda");
        assert!(matches!(err, AppError::ServiceUnavailable(_)));
    }

    #[tokio::test]
    async fn sessao_de_outro_ip_nao_abre_a_porta() {
        let server = server().await;
        let mut session = server
            .arm(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
                Duration::from_millis(600),
            )
            .expect("armado");
        let err = client(&server, &spec(TestDirection::Send), None)
            .await
            .expect_err("outro IP");
        assert!(matches!(err, AppError::ServiceUnavailable(_)));
        let expirou = session.outcome.wait().await.expect_err("ninguém conectou");
        // A recusa continua; o diagnóstico aponta o IP que chegou (NAT?).
        assert!(
            matches!(&expirou, AppError::ServiceUnavailable(m)
                if m.contains("prazo") && m.contains("127.0.0.1") && m.contains("NAT")),
            "{expirou:?}"
        );
    }

    #[tokio::test]
    async fn sessao_expirada_recusa_e_informa() {
        let server = server().await;
        let mut session = server
            .arm(LOCAL, Duration::from_millis(150))
            .expect("armado");
        let err = session.outcome.wait().await.expect_err("expirou");
        assert!(
            matches!(&err, AppError::ServiceUnavailable(m)
                if m.contains("prazo") && m.contains("firewall") && !m.contains("NAT")),
            "{err:?}"
        );
        assert!(client(&server, &spec(TestDirection::Send), None)
            .await
            .is_err());
        // Expirada, a vaga do IP pode ser armada de novo.
        assert!(server.arm(LOCAL, Duration::from_secs(1)).is_ok());
    }

    #[tokio::test]
    async fn arm_duplicado_e_conflito_e_drop_desarma() {
        let server = server().await;
        let primeira = server.arm(LOCAL, Duration::from_secs(5)).expect("armado");
        let err = server
            .arm(LOCAL, Duration::from_secs(5))
            .err()
            .expect("duplicado");
        assert!(matches!(err, AppError::Conflict(_)));
        drop(primeira);
        assert!(client(&server, &spec(TestDirection::Send), None)
            .await
            .is_err());
        assert!(server.arm(LOCAL, Duration::from_secs(5)).is_ok());
    }

    #[tokio::test]
    async fn cancelar_a_sessao_armada() {
        let server = server().await;
        let mut session = server.arm(LOCAL, Duration::from_secs(5)).expect("armado");
        session.cancel();
        let err = session.outcome.wait().await.expect_err("cancelada");
        assert!(is_cancelled(&err), "{err:?}");
    }

    #[tokio::test]
    async fn cancelar_o_cliente_no_meio_do_teste() {
        let server = server().await;
        let _session = server.arm(LOCAL, Duration::from_secs(5)).expect("armado");
        let s = ThroughputSpec {
            duration_s: 30,
            ..spec(TestDirection::Send)
        };
        let cancel = CancellationToken::new();
        let gatilho = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            gatilho.cancel();
        });
        let (sink, _rx) = sample_channel();
        let err = timeout(
            Duration::from_secs(3),
            run_client(server.local_addr(), &s, None, &sink, &cancel),
        )
        .await
        .expect("cancelou rápido")
        .expect_err("cancelado");
        assert!(is_cancelled(&err));
    }

    #[tokio::test]
    async fn recusa_mais_de_um_fluxo() {
        let server = server().await;
        let s = ThroughputSpec {
            streams: 2,
            ..spec(TestDirection::Send)
        };
        assert!(matches!(
            client(&server, &s, None).await,
            Err(AppError::Validation(_))
        ));
    }

    /// Servidor falso que pede autenticação: devolve o que o cliente mandou.
    async fn fake_auth_server(
        challenge: &'static [u8],
        verdict: [u8; 4],
    ) -> (std::net::SocketAddr, oneshot::Receiver<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut cmd = [0u8; codec::COMMAND_LEN];
            if stream.write_all(&codec::HELLO).await.is_err()
                || stream.read_exact(&mut cmd).await.is_err()
                || stream.write_all(challenge).await.is_err()
            {
                return;
            }
            if challenge.len() == 4 {
                return; // EC-SRP5 ou lixo: o cliente desiste aqui.
            }
            let mut reply = vec![0u8; codec::AUTH_REPLY_LEN];
            if stream.read_exact(&mut reply).await.is_ok() {
                let _ = stream.write_all(&verdict).await;
                let _ = tx.send(reply);
            }
            // Fecha logo: o cliente encerra o teste ao ver o fim da conexão.
        });
        (addr, rx)
    }

    const MD5_CHALLENGE: &[u8] = &[
        0x02, 0x00, 0x00, 0x00, 0xad, 0x32, 0xd6, 0xf9, 0x4d, 0x28, 0x16, 0x16, 0x25, 0xf2, 0xf3,
        0x90, 0xbb, 0x89, 0x56, 0x37,
    ];

    async fn run_against(
        addr: std::net::SocketAddr,
        auth: Option<&BtestAuth>,
    ) -> Result<ThroughputReport, AppError> {
        let (sink, _rx) = sample_channel();
        timeout(
            Duration::from_secs(5),
            run_client(
                addr,
                &spec(TestDirection::Send),
                auth,
                &sink,
                &CancellationToken::new(),
            ),
        )
        .await
        .expect("dentro de 5 s")
    }

    #[tokio::test]
    async fn autenticacao_md5_manda_o_hash_do_vetor() {
        let (addr, enviado) = fake_auth_server(MD5_CHALLENGE, codec::AUTH_OK).await;
        let auth = BtestAuth {
            user: "test".into(),
            password: "test".into(),
        };
        run_against(addr, Some(&auth)).await.expect("autenticado");
        let reply = enviado.await.expect("resposta capturada");
        let esperado: [u8; 16] = [
            0x3c, 0x96, 0x85, 0x65, 0xbc, 0x03, 0x14, 0xf2, 0x81, 0xa6, 0xda, 0x15, 0x71, 0xcf,
            0x72, 0x55,
        ];
        assert_eq!(&reply[..16], &esperado);
        assert_eq!(&reply[16..20], b"test");
        assert!(reply[20..].iter().all(|b| *b == 0));
    }

    #[tokio::test]
    async fn autenticacao_md5_recusada() {
        let (addr, _) = fake_auth_server(MD5_CHALLENGE, codec::AUTH_FAILED).await;
        let auth = BtestAuth {
            user: "test".into(),
            password: "errada".into(),
        };
        let err = run_against(addr, Some(&auth)).await.expect_err("recusada");
        assert!(matches!(&err, AppError::BusinessRule(m) if m == AUTH_FAILED_MESSAGE));
    }

    #[tokio::test]
    async fn autenticacao_exigida_sem_credencial() {
        let (addr, _) = fake_auth_server(MD5_CHALLENGE, codec::AUTH_OK).await;
        let err = run_against(addr, None).await.expect_err("sem credencial");
        assert!(matches!(&err, AppError::BusinessRule(m) if m == AUTH_REQUIRED_MESSAGE));
    }

    #[tokio::test]
    async fn ec_srp5_e_recusado_com_erro_proprio() {
        let (addr, _) = fake_auth_server(&codec::AUTH_EC_SRP5, codec::AUTH_OK).await;
        let auth = BtestAuth {
            user: "admin".into(),
            password: "x".into(),
        };
        let err = run_against(addr, Some(&auth)).await.expect_err("EC-SRP5");
        assert!(is_ec_srp5_unsupported(&err), "{err:?}");
        assert!(!is_ec_srp5_unsupported(&AppError::business_rule("outro")));
    }

    #[tokio::test]
    async fn resposta_desconhecida_e_erro_de_protocolo() {
        let (addr, _) = fake_auth_server(&[0x09, 0x09, 0x09, 0x09], codec::AUTH_OK).await;
        let err = run_against(addr, None).await.expect_err("lixo");
        assert!(
            matches!(&err, AppError::BusinessRule(m) if m.contains("protocolo")),
            "{err:?}"
        );
    }

    #[test]
    fn endereco_de_escuta() {
        assert_eq!(
            listen_addr(None),
            Ok(Some("0.0.0.0:2000".parse().expect("addr")))
        );
        assert_eq!(listen_addr(Some("")), Ok(None));
        assert_eq!(listen_addr(Some("  ")), Ok(None));
        assert_eq!(
            listen_addr(Some("127.0.0.1:2100")),
            Ok(Some("127.0.0.1:2100".parse().expect("addr")))
        );
        assert_eq!(
            listen_addr(Some("[::]:2000")),
            Ok(Some("[::]:2000".parse().expect("addr")))
        );
        assert!(listen_addr(Some("porta-2000")).is_err());
    }

    #[test]
    fn credencial_nao_vaza_no_debug() {
        let auth = BtestAuth {
            user: "nm-test".into(),
            password: "segredo".into(),
        };
        let debug = format!("{auth:?}");
        assert!(debug.contains("nm-test") && !debug.contains("segredo"));
    }

    #[tokio::test]
    async fn porta_ocupada_e_indisponivel() {
        let ocupada = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = ocupada.local_addr().expect("addr");
        let err = BtestServer::bind(addr).await.expect_err("ocupada");
        assert!(matches!(&err, AppError::ServiceUnavailable(m) if m.contains(UNAVAILABLE_MESSAGE)));
    }
}
