//! **SPIKE — estimar a banda de um enlace só com ping** (avaliação do "teste
//! rápido" para equipamento desconhecido, sem SSH nem plugin).
//!
//! Método (dispersão por tamanho): o RTT de um eco grande é o de um pequeno
//! mais o tempo de serializar a diferença de bytes nos dois sentidos. Com os
//! **mínimos** (o menor ruído de fila), a capacidade do gargalo sai de
//!
//! ```text
//! capacidade ≈ 2 · (grande − pequeno) · 8 bits / (RTTmin_grande − RTTmin_pequeno)
//! ```
//!
//! A pergunta do spike é empírica: em enlaces reais, a estimativa chega perto
//! da banda medida por btest/iperf3? Onde ela quebra?
//!
//! Uso (leve: ~2 pacotes a cada 20 ms):
//!
//! ```sh
//! cargo run --example spike_ping_capacity -- 10.0.0.242 10.0.0.2
//! ```

use std::{net::IpAddr, time::Duration};

use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};

const PAIRS: u16 = 80;
const SMALL: usize = 56;
const LARGE: usize = 1_472;
const GAP: Duration = Duration::from_millis(20);
const TIMEOUT: Duration = Duration::from_secs(2);

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let index = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let targets: Vec<IpAddr> = std::env::args()
        .skip(1)
        .map(|arg| arg.parse())
        .collect::<Result<_, _>>()?;
    let client = Client::new(&Config::builder().kind(ICMP::V4).build())?;
    for target in targets {
        let mut pinger = client.pinger(target, PingIdentifier(rand::random())).await;
        pinger.timeout(TIMEOUT);
        let (small, large) = (vec![0u8; SMALL], vec![0u8; LARGE]);
        let (mut rtt_small, mut rtt_large) = (Vec::new(), Vec::new());
        let mut lost = 0;
        for seq in 0..PAIRS {
            // Intercalados: a mesma condição de fila vale para os dois tamanhos.
            for (payload, bucket, offset) in
                [(&small, &mut rtt_small, 0), (&large, &mut rtt_large, 1)]
            {
                match pinger.ping(PingSequence(seq * 2 + offset), payload).await {
                    Ok((_, rtt)) => bucket.push(rtt.as_secs_f64() * 1_000.0),
                    Err(_) => lost += 1,
                }
                tokio::time::sleep(GAP).await;
            }
        }
        rtt_small.sort_by(f64::total_cmp);
        rtt_large.sort_by(f64::total_cmp);
        if rtt_small.is_empty() || rtt_large.is_empty() {
            println!("{target}: sem resposta");
            continue;
        }
        let (min_s, min_l) = (rtt_small[0], rtt_large[0]);
        let delta_ms = min_l - min_s;
        let bits = 2.0 * (LARGE - SMALL) as f64 * 8.0;
        let estimate = if delta_ms > 0.0 {
            format!("{:.1} Mbit/s", bits / (delta_ms / 1_000.0) / 1e6)
        } else {
            "indeterminada (diferença ≤ 0)".to_owned()
        };
        println!(
            "{target}: pequeno min {min_s:.3} p50 {:.3} p95 {:.3} ms · grande min {min_l:.3} p50 {:.3} p95 {:.3} ms · Δmin {delta_ms:.3} ms · perda {lost}/{} · capacidade estimada {estimate}",
            percentile(&rtt_small, 0.5),
            percentile(&rtt_small, 0.95),
            percentile(&rtt_large, 0.5),
            percentile(&rtt_large, 0.95),
            PAIRS * 2,
        );
    }
    Ok(())
}
