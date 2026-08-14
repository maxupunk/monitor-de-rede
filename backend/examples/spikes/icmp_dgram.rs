//! **SPIKE-03 — ICMP sem privilégio** (§3.4 do roadmap).
//!
//! Pergunta: `surge-ping` com socket `SOCK_DGRAM` funciona no container base
//! escolhido, sem `CAP_NET_RAW`?
//!
//! O que este protótipo prova, na ordem:
//!
//! 1. que o socket ICMP DGRAM **abre** (é aqui que falta de privilégio
//!    aparece — `EACCES`/`EPERM` — e não na hora de enviar);
//! 2. que um `Client` só é aberto **uma vez** e multiplexa várias medições por
//!    `PingIdentifier`, como exige a §3.2.1;
//! 3. que a latência medida bate com a do `ping` do sistema.
//!
//! Uso:
//!
//! ```sh
//! # dentro do container (ver docker-compose.icmp-spike.yml)
//! cargo run --example spike_icmp_dgram -- 1.1.1.1 8.8.8.8
//! ```
//!
//! Sem argumentos, mede `127.0.0.1`. O código de saída é 0 só se todos os
//! alvos responderem.

use std::{net::IpAddr, time::Duration};

use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};

/// Igual ao default do `PingChecker` atual (§3.2).
const PACKET_COUNT: u16 = 3;
const PAYLOAD_LEN: usize = 56;
const TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let alvos: Vec<String> = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.is_empty() {
            vec!["127.0.0.1".to_string()]
        } else {
            args
        }
    };

    // (1) `sock_type_hint(DGRAM)` é a decisão sob teste: com ele o kernel usa
    // o socket ICMP não privilegiado, liberado por
    // `sysctl net.ipv4.ping_group_range="0 2147483647"`. Sem ele, seria raw
    // socket e exigiria CAP_NET_RAW.
    let config = Config::builder()
        .kind(ICMP::V4)
        .sock_type_hint(socket2::Type::DGRAM)
        .build();

    let client = match Client::new(&config) {
        Ok(client) => {
            println!("[ok] socket ICMP DGRAM aberto sem privilégio adicional");
            client
        }
        Err(err) => {
            eprintln!("[FALHA] não foi possível abrir o socket ICMP DGRAM: {err}");
            eprintln!(
                "        Verifique `sysctl net.ipv4.ping_group_range` no container \
                 (ou rode com CAP_NET_RAW e sem sock_type_hint)."
            );
            return Err(err.into());
        }
    };

    let mut todos_responderam = true;

    for alvo in &alvos {
        let ip: IpAddr = match alvo.parse() {
            Ok(ip) => ip,
            Err(err) => {
                eprintln!("[FALHA] `{alvo}` não é um IP: {err}");
                todos_responderam = false;
                continue;
            }
        };

        // (2) Um identificador por alvo; o `Client` casa a resposta com o
        // pinger certo. É isso que permite um socket só para o monitor inteiro
        // e para o sweep do discovery.
        let mut pinger = client.pinger(ip, PingIdentifier(rand::random())).await;
        pinger.timeout(TIMEOUT);

        let payload = vec![0u8; PAYLOAD_LEN];
        let mut latencias = Vec::new();

        for sequencia in 0..PACKET_COUNT {
            match pinger.ping(PingSequence(sequencia), &payload).await {
                Ok((_pacote, rtt)) => latencias.push(rtt),
                Err(err) => println!("  seq {sequencia}: sem resposta ({err})"),
            }
        }

        let perda =
            100.0 * (f64::from(PACKET_COUNT) - latencias.len() as f64) / f64::from(PACKET_COUNT);

        if latencias.is_empty() {
            println!("[{alvo}] 100% de perda — status `down`");
            todos_responderam = false;
        } else {
            let media: Duration =
                latencias.iter().sum::<Duration>() / u32::try_from(latencias.len()).unwrap_or(1);
            // (3) Compare com `ping -c 3 <alvo>`: a §3.2 aceita ±10%.
            println!(
                "[{alvo}] latência média {:.2} ms, perda {perda:.0}% — status `{}`",
                media.as_secs_f64() * 1000.0,
                if perda > 0.0 { "warning" } else { "up" }
            );
        }
    }

    if todos_responderam {
        println!("\nSPIKE-03: ICMP DGRAM operacional.");
        Ok(())
    } else {
        Err("ao menos um alvo não respondeu".into())
    }
}
