//! **SPIKE-04 — DNS wire** (§3.4 do roadmap).
//!
//! Pergunta: `hickory-proto` permite montar e ler o pacote DNS à mão, mantendo
//! o cronômetro **só** na etapa de resolução?
//!
//! Isso importa porque o `DnsLatencyCard` compara resolvedores. Se o
//! cronômetro englobar `connect`, resolução de nome do próprio servidor ou
//! criação de cliente, o ranking mede a máquina local, não o resolvedor. Usar
//! um cliente pronto (`hickory-resolver`) esconderia essas etapas dentro da
//! chamada; encodar/decodar à mão deixa o `Instant` exatamente em volta do
//! round-trip.
//!
//! Cobre os três transportes do §7.15: UDP, TCP e DoH.
//!
//! ```sh
//! cargo run --example spike_dns_wire
//! cargo run --example spike_dns_wire -- github.com 9.9.9.9
//! ```

use std::{
    str::FromStr,
    time::{Duration, Instant},
};

use hickory_proto::{
    op::{Message, MessageType, OpCode, Query},
    rr::{DNSClass, Name, RecordType},
    serialize::binary::BinDecodable,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

const TIMEOUT: Duration = Duration::from_secs(5);
const DOH_URL: &str = "https://cloudflare-dns.com/dns-query";

/// Monta a consulta. O `id` é sorteado: um resolvedor descarta resposta cujo id
/// não bate, e reusar id fixo abriria a porta para cache poisoning trivial.
fn consulta(hostname: &str, tipo: RecordType) -> Result<Message, Box<dyn std::error::Error>> {
    let nome = Name::from_str(hostname)?;
    let mut query = Query::query(nome, tipo);
    query.set_query_class(DNSClass::IN);

    let mut mensagem = Message::new();
    mensagem
        .set_id(rand::random())
        .set_message_type(MessageType::Query)
        .set_op_code(OpCode::Query)
        .set_recursion_desired(true)
        .add_query(query);

    Ok(mensagem)
}

fn descreva(resposta: &Message) -> String {
    if resposta.answers().is_empty() {
        return format!("sem resposta (rcode {:?})", resposta.response_code());
    }
    resposta
        .answers()
        .iter()
        .map(|r| r.data().map_or_else(String::new, ToString::to_string))
        .collect::<Vec<_>>()
        .join(", ")
}

/// UDP: o cronômetro cobre só `send` + `recv`. O `bind`/`connect` fica fora.
async fn via_udp(
    servidor: &str,
    hostname: &str,
) -> Result<(Duration, Message), Box<dyn std::error::Error>> {
    let pergunta = consulta(hostname, RecordType::A)?.to_vec()?;

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect((servidor, 53)).await?;

    let inicio = Instant::now();
    socket.send(&pergunta).await?;
    let mut buffer = [0u8; 4096];
    let lidos = tokio::time::timeout(TIMEOUT, socket.recv(&mut buffer)).await??;
    let decorrido = inicio.elapsed();

    Ok((decorrido, Message::from_bytes(&buffer[..lidos])?))
}

/// TCP: mesma consulta, mas o RFC 1035 §4.2.2 exige um prefixo de 2 bytes com
/// o tamanho da mensagem — tanto no envio quanto na leitura.
async fn via_tcp(
    servidor: &str,
    hostname: &str,
) -> Result<(Duration, Message), Box<dyn std::error::Error>> {
    let pergunta = consulta(hostname, RecordType::A)?.to_vec()?;
    let tamanho = u16::try_from(pergunta.len())?;

    let mut stream = tokio::time::timeout(TIMEOUT, TcpStream::connect((servidor, 53))).await??;

    let inicio = Instant::now();
    stream.write_all(&tamanho.to_be_bytes()).await?;
    stream.write_all(&pergunta).await?;

    let mut prefixo = [0u8; 2];
    stream.read_exact(&mut prefixo).await?;
    let mut resposta = vec![0u8; usize::from(u16::from_be_bytes(prefixo))];
    stream.read_exact(&mut resposta).await?;
    let decorrido = inicio.elapsed();

    Ok((decorrido, Message::from_bytes(&resposta)?))
}

/// DoH (RFC 8484): o **mesmo** pacote binário, num POST
/// `application/dns-message`. Nenhum encoder novo — é o argumento central para
/// codificar à mão em vez de usar um cliente por transporte.
async fn via_doh(
    url: &str,
    hostname: &str,
) -> Result<(Duration, Message), Box<dyn std::error::Error>> {
    let pergunta = consulta(hostname, RecordType::A)?.to_vec()?;
    let cliente = reqwest::Client::builder().timeout(TIMEOUT).build()?;

    let inicio = Instant::now();
    let resposta = cliente
        .post(url)
        .header("content-type", "application/dns-message")
        .header("accept", "application/dns-message")
        .body(pergunta)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let decorrido = inicio.elapsed();

    Ok((decorrido, Message::from_bytes(&resposta)?))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let hostname = args.next().unwrap_or_else(|| "example.com".to_string());
    let servidor = args.next().unwrap_or_else(|| "1.1.1.1".to_string());

    // Prova offline: encode → decode preserva id e pergunta, sem tocar a rede.
    let original = consulta(&hostname, RecordType::A)?;
    let bytes = original.to_vec()?;
    let relida = Message::from_bytes(&bytes)?;
    assert_eq!(relida.id(), original.id());
    assert_eq!(relida.queries()[0].name(), original.queries()[0].name());
    println!(
        "[ok] round-trip wire preserva id e pergunta ({} bytes)",
        bytes.len()
    );

    let mut falhas = 0;
    for (rotulo, resultado) in [
        (
            format!("UDP  {servidor}:53"),
            via_udp(&servidor, &hostname).await,
        ),
        (
            format!("TCP  {servidor}:53"),
            via_tcp(&servidor, &hostname).await,
        ),
        (format!("DoH  {DOH_URL}"), via_doh(DOH_URL, &hostname).await),
    ] {
        match resultado {
            Ok((decorrido, resposta)) => println!(
                "[ok] {rotulo}: {:.3} ms — {}",
                decorrido.as_secs_f64() * 1000.0,
                descreva(&resposta)
            ),
            Err(err) => {
                println!("[falha] {rotulo}: {err}");
                falhas += 1;
            }
        }
    }

    if falhas == 0 {
        println!("\nSPIKE-04: hickory-proto basta para os três transportes.");
        Ok(())
    } else {
        Err(format!("{falhas} transporte(s) falharam — verifique a saída de rede").into())
    }
}
