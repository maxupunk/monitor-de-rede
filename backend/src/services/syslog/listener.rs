//! Os dois listeners: UDP e TCP.
//!
//! **Porta 5514, nunca 514.** O processo roda com `CapEff` zerado
//! (`setpriv --inh-caps=-all` no entrypoint) e não abre porta privilegiada;
//! quem faz a ponte é o `docker-compose.yml`, publicando `514:5514`. Pedir
//! `CAP_NET_BIND` para a API atravessaria a fronteira do §7 do `AGENTS.md`, que
//! nem o WireGuard atravessa.
//!
//! O TCP entende as **duas** molduras da RFC 6587 — contagem de octetos
//! (`123 <134>…`) e delimitação por LF. Não é preciosismo: o rsyslog manda a
//! primeira, quase todo o resto manda a segunda, e escolher uma só significaria
//! ler lixo do outro lado.

use std::{net::IpAddr, sync::Arc, time::Duration};

use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, UdpSocket},
    sync::Semaphore,
};

use super::ingest::Ingestor;
use crate::services::shared::errors::{AppError, AppResult};

/// Buffer do socket UDP.
///
/// Roteador que reiniciou despeja o buffer de log de uma vez, e o padrão do
/// sistema (~208 KiB) transborda numa rajada dessas — com o agravante de que a
/// perda aconteceria no kernel, fora dos nossos contadores, invisível. Falha ao
/// ajustar não é fatal: o padrão cobre a carga alvo.
const RECV_BUFFER_BYTES: usize = 4 * 1024 * 1024;

/// Teto de conexões TCP simultâneas. Trinta a quarenta dispositivos não chegam
/// perto; o número existe para um cliente defeituoso não abrir mil sockets.
const MAX_TCP_CONNECTIONS: usize = 128;

/// Conexão TCP sem tráfego por este tempo é encerrada.
const TCP_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Sobe o listener UDP e devolve a porta efetiva.
///
/// Porta `0` pede uma efêmera ao sistema — é o que os testes usam para não
/// colidir entre si.
///
/// # Errors
///
/// Falha ao abrir o socket.
pub async fn spawn_udp(porta: u16, ingestor: Ingestor) -> AppResult<u16> {
    let socket = UdpSocket::bind(("0.0.0.0", porta))
        .await
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;

    if let Err(error) = socket2::SockRef::from(&socket).set_recv_buffer_size(RECV_BUFFER_BYTES) {
        tracing::debug!(%error, "buffer de recepção UDP mantido no padrão do sistema");
    }

    let porta = socket
        .local_addr()
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?
        .port();
    tracing::info!(porta, "syslog UDP escutando");

    tokio::spawn(async move {
        // Uma folga sobre o teto de mensagem para reconhecer o datagrama que
        // veio grande demais — sem ela, `recv_from` truncaria em silêncio e o
        // contador de corte nunca subiria.
        let mut buffer = vec![0_u8; ingestor.max_msg_bytes() + 1024];
        loop {
            match socket.recv_from(&mut buffer).await {
                Ok((tamanho, origem)) => {
                    ingestor.handle(&buffer[..tamanho], origem.ip()).await;
                }
                Err(error) => {
                    tracing::warn!(%error, "falha ao ler do socket UDP de syslog");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    });

    Ok(porta)
}

/// Sobe o listener TCP e devolve a porta efetiva.
///
/// # Errors
///
/// Falha ao abrir o socket.
pub async fn spawn_tcp(porta: u16, ingestor: Ingestor) -> AppResult<u16> {
    let listener = TcpListener::bind(("0.0.0.0", porta))
        .await
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    let porta = listener
        .local_addr()
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?
        .port();
    tracing::info!(porta, "syslog TCP escutando");

    tokio::spawn(async move {
        let vagas = Arc::new(Semaphore::new(MAX_TCP_CONNECTIONS));
        loop {
            match listener.accept().await {
                Ok((stream, origem)) => {
                    let Ok(vaga) = Arc::clone(&vagas).acquire_owned().await else {
                        return;
                    };
                    let ingestor = ingestor.clone();
                    tokio::spawn(async move {
                        atende(stream, origem.ip(), ingestor).await;
                        drop(vaga);
                    });
                }
                Err(error) => {
                    tracing::warn!(%error, "falha ao aceitar conexão TCP de syslog");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    });

    Ok(porta)
}

/// Uma conexão TCP, do primeiro byte ao fechamento.
async fn atende(mut stream: tokio::net::TcpStream, origem: IpAddr, ingestor: Ingestor) {
    let maximo = ingestor.max_msg_bytes();
    let mut acumulado: Vec<u8> = Vec::with_capacity(maximo);
    let mut pedaco = vec![0_u8; 8192];

    loop {
        let lido = match tokio::time::timeout(TCP_IDLE_TIMEOUT, stream.read(&mut pedaco)).await {
            Ok(Ok(0)) => {
                // Fim de fluxo: o que sobrou no buffer é uma última linha sem
                // delimitador. Descartá-la perderia a mensagem que motivou a
                // conexão em conexões de uma linha só.
                if !acumulado.is_empty() {
                    ingestor.handle(&acumulado, origem).await;
                }
                return;
            }
            Ok(Ok(lido)) => lido,
            Ok(Err(error)) => {
                tracing::debug!(%error, %origem, "conexão TCP de syslog encerrada com erro");
                return;
            }
            Err(_) => {
                tracing::debug!(%origem, "conexão TCP de syslog ociosa encerrada");
                return;
            }
        };

        acumulado.extend_from_slice(&pedaco[..lido]);

        loop {
            match proxima_moldura(&mut acumulado, maximo) {
                Framing::Message(linha) => {
                    if !linha.is_empty() {
                        ingestor.handle(&linha, origem).await;
                    }
                }
                Framing::Incomplete => break,
                Framing::Invalid => {
                    // Contagem de octetos anunciando mais do que o teto: não há
                    // como reencontrar o começo da próxima mensagem sem confiar
                    // no número que já se mostrou absurdo. Fechar é a única
                    // saída que não vira leitura de lixo.
                    tracing::warn!(%origem, "moldura TCP inválida; conexão encerrada");
                    return;
                }
            }
        }
    }
}

/// O que saiu do buffer.
#[derive(Debug, PartialEq, Eq)]
enum Framing {
    Message(Vec<u8>),
    Incomplete,
    Invalid,
}

/// Extrai a próxima mensagem, consumindo-a do buffer.
///
/// Reconhece a moldura pelo primeiro byte útil: dígito seguido de espaço é
/// contagem de octetos (RFC 6587 §3.4.1); qualquer outra coisa é delimitação
/// por LF (§3.4.2).
fn proxima_moldura(buffer: &mut Vec<u8>, maximo: usize) -> Framing {
    // Delimitadores acumulados entre mensagens.
    let inicio = buffer
        .iter()
        .position(|byte| !matches!(byte, b'\n' | b'\r' | b'\0'))
        .unwrap_or(buffer.len());
    if inicio > 0 {
        buffer.drain(..inicio);
    }
    if buffer.is_empty() {
        return Framing::Incomplete;
    }

    // Contagem de octetos é **só dígitos** seguidos de um espaço. Exigir isso,
    // e não "um espaço nos primeiros bytes", é o que impede `2026-08-15 algo`
    // de ser lido como um cabeçalho: ali os dígitos param no `-`, não no
    // espaço. Sem essa distinção, uma linha que começa com data derrubaria a
    // conexão inteira.
    let digitos = buffer
        .iter()
        .take(11)
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digitos > 0 && digitos <= 10 && buffer.get(digitos) == Some(&b' ') {
        let Ok(texto) = std::str::from_utf8(&buffer[..digitos]) else {
            return Framing::Invalid;
        };
        let Ok(tamanho) = texto.parse::<usize>() else {
            return Framing::Invalid;
        };
        // Anúncio acima do teto: não há como reencontrar o começo da próxima
        // mensagem confiando num número que já se mostrou absurdo.
        if tamanho > maximo {
            return Framing::Invalid;
        }
        let total = digitos + 1 + tamanho;
        if buffer.len() < total {
            return Framing::Incomplete;
        }
        let linha = buffer[digitos + 1..total].to_vec();
        buffer.drain(..total);
        return Framing::Message(linha);
    }

    if let Some(quebra) = buffer.iter().position(|byte| *byte == b'\n') {
        let mut linha = buffer[..quebra].to_vec();
        if linha.last() == Some(&b'\r') {
            linha.pop();
        }
        buffer.drain(..=quebra);
        linha.truncate(maximo);
        return Framing::Message(linha);
    }

    // Remetente que nunca manda LF não pode fazer o buffer crescer sem fim.
    if buffer.len() > maximo {
        let linha = buffer[..maximo].to_vec();
        buffer.drain(..maximo);
        return Framing::Message(linha);
    }

    Framing::Incomplete
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(texto: &str) -> Vec<u8> {
        texto.as_bytes().to_vec()
    }

    #[test]
    fn a_moldura_por_lf_entrega_uma_linha_por_vez() {
        let mut dados = buffer("<134>primeira\n<134>segunda\n");
        assert_eq!(
            proxima_moldura(&mut dados, 8192),
            Framing::Message(buffer("<134>primeira"))
        );
        assert_eq!(
            proxima_moldura(&mut dados, 8192),
            Framing::Message(buffer("<134>segunda"))
        );
        assert_eq!(proxima_moldura(&mut dados, 8192), Framing::Incomplete);
    }

    #[test]
    fn o_crlf_do_windows_nao_entra_na_mensagem() {
        let mut dados = buffer("<134>linha\r\n");
        assert_eq!(
            proxima_moldura(&mut dados, 8192),
            Framing::Message(buffer("<134>linha"))
        );
    }

    #[test]
    fn a_contagem_de_octetos_do_rfc6587_e_entendida() {
        // `13` é o tamanho exato de `<134>mensagem`.
        let mut dados = buffer("13 <134>mensagem13 <134>mensagem");
        assert_eq!(
            proxima_moldura(&mut dados, 8192),
            Framing::Message(buffer("<134>mensagem"))
        );
        assert_eq!(
            proxima_moldura(&mut dados, 8192),
            Framing::Message(buffer("<134>mensagem"))
        );
        assert!(dados.is_empty());
    }

    #[test]
    fn a_contagem_incompleta_espera_o_resto_do_fluxo() {
        let mut dados = buffer("13 <134>men");
        assert_eq!(proxima_moldura(&mut dados, 8192), Framing::Incomplete);
        // O TCP fragmenta onde quiser; o resto chega depois.
        dados.extend_from_slice(b"sagem");
        assert_eq!(
            proxima_moldura(&mut dados, 8192),
            Framing::Message(buffer("<134>mensagem"))
        );
    }

    #[test]
    fn contagem_maior_que_o_teto_e_recusada() {
        // Não há como reencontrar o começo da próxima mensagem confiando num
        // número absurdo: a conexão é encerrada.
        let mut dados = buffer("999999 <134>oi");
        assert_eq!(proxima_moldura(&mut dados, 8192), Framing::Invalid);
    }

    #[test]
    fn linha_que_comeca_com_digito_ainda_e_lida_por_lf() {
        // `2026-08-15 ...` começa com dígito mas não é contagem de octetos.
        let mut dados = buffer("2026-08-15 aconteceu algo\n");
        assert_eq!(
            proxima_moldura(&mut dados, 8192),
            Framing::Message(buffer("2026-08-15 aconteceu algo"))
        );
    }

    #[test]
    fn remetente_sem_lf_nao_faz_o_buffer_crescer_sem_fim() {
        let mut dados = buffer(&"x".repeat(100));
        assert_eq!(
            proxima_moldura(&mut dados, 32),
            Framing::Message(buffer(&"x".repeat(32)))
        );
        assert_eq!(dados.len(), 68);
    }

    #[test]
    fn linha_maior_que_o_teto_e_truncada_e_nao_perdida() {
        let mut dados = buffer(&format!("{}\n", "y".repeat(100)));
        let Framing::Message(linha) = proxima_moldura(&mut dados, 32) else {
            panic!("esperava mensagem");
        };
        assert_eq!(linha.len(), 32);
        assert!(dados.is_empty(), "o resto da linha é consumido junto");
    }

    #[test]
    fn delimitadores_acumulados_entre_mensagens_sao_ignorados() {
        let mut dados = buffer("\n\n\0<134>linha\n");
        assert_eq!(
            proxima_moldura(&mut dados, 8192),
            Framing::Message(buffer("<134>linha"))
        );
    }
}
