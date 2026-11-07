//! Serviço de Teste de Velocidade WAN e LAN.
//!
//! Mede Ping, Jitter, Download e Upload via CDN da Cloudflare Speedtest
//! de forma assíncrona com streaming contínuo de progresso.

use std::time::{Duration, Instant};

use chrono::Utc;
use futures::StreamExt;
use reqwest::Client;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    dtos::diagnostics::{SpeedTestProgress, SpeedTestResult},
    services::network_tools::icmp_probe::{duration_to_ms, round_two},
};

const CLOUDFLARE_BASE: &str = "https://speed.cloudflare.com";
const CLIENT_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeedTestProfile {
    Simple,
    #[default]
    Medium,
    Complete,
}

impl SpeedTestProfile {
    pub fn from_opt_str(s: Option<&str>) -> Self {
        match s.map(|v| v.to_ascii_lowercase()).as_deref() {
            Some("simple") | Some("simples") | Some("quick") => Self::Simple,
            Some("complete") | Some("completo") | Some("full") => Self::Complete,
            _ => Self::Medium,
        }
    }

    pub fn ping_count(&self) -> usize {
        match self {
            Self::Simple => 3,
            Self::Medium => 5,
            Self::Complete => 10,
        }
    }

    pub fn download_bytes(&self) -> usize {
        match self {
            Self::Simple => 10_000_000,   // 10 MB
            Self::Medium => 25_000_000,   // 25 MB
            Self::Complete => 60_000_000, // 60 MB
        }
    }

    pub fn upload_bytes(&self) -> usize {
        match self {
            Self::Simple => 8_000_000,    // 8 MB
            Self::Medium => 20_000_000,   // 20 MB
            Self::Complete => 50_000_000, // 50 MB
        }
    }

    pub fn upload_chunks(&self) -> usize {
        match self {
            Self::Simple => 16,
            Self::Medium => 32,
            Self::Complete => 50,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SpeedTestEvent {
    Progress(SpeedTestProgress),
    Complete(SpeedTestResult),
    Error(String),
}

/// Executa o teste de velocidade WAN completo transmitindo o progresso em tempo real.
pub async fn execute_wan_speedtest(
    sender: mpsc::Sender<SpeedTestEvent>,
    cancel: CancellationToken,
    profile_str: Option<&str>,
) -> Option<SpeedTestResult> {
    let profile = SpeedTestProfile::from_opt_str(profile_str);
    let mut default_headers = reqwest::header::HeaderMap::new();
    default_headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        ),
    );
    default_headers.insert(
        reqwest::header::REFERER,
        reqwest::header::HeaderValue::from_static("https://speed.cloudflare.com/"),
    );
    default_headers.insert(
        reqwest::header::ORIGIN,
        reqwest::header::HeaderValue::from_static("https://speed.cloudflare.com"),
    );

    let client = match Client::builder()
        .default_headers(default_headers)
        .timeout(CLIENT_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            let _ = sender
                .send(SpeedTestEvent::Error(format!(
                    "Falha ao criar cliente HTTP: {err}"
                )))
                .await;
            return None;
        }
    };

    // 1. Fase de Ping & Jitter
    let (ping_ms, jitter_ms, server_location) =
        match measure_ping_jitter(&client, &sender, &cancel, profile.ping_count()).await {
            Some(res) => res,
            None => {
                if !cancel.is_cancelled() {
                    let _ = sender
                        .send(SpeedTestEvent::Error(
                            "Falha ao medir latência com o servidor".into(),
                        ))
                        .await;
                }
                return None;
            }
        };

    if cancel.is_cancelled() {
        return None;
    }

    // 2. Fase de Download
    let download_mbps = match measure_download(
        &client,
        &sender,
        &cancel,
        ping_ms,
        jitter_ms,
        &server_location,
        profile.download_bytes(),
    )
    .await
    {
        Some(mbps) => mbps,
        None => {
            if !cancel.is_cancelled() {
                let _ = sender
                    .send(SpeedTestEvent::Error(
                        "Falha durante o teste de download".into(),
                    ))
                    .await;
            }
            return None;
        }
    };

    if cancel.is_cancelled() {
        return None;
    }

    // 3. Fase de Upload
    let upload_mbps = match measure_upload(
        &client,
        &sender,
        &cancel,
        UploadOptions {
            ping_ms,
            jitter_ms,
            download_mbps,
            location: &server_location,
            bytes: profile.upload_bytes(),
            chunks: profile.upload_chunks(),
        },
    )
    .await
    {
        Some(mbps) => mbps,
        None => {
            if !cancel.is_cancelled() {
                let _ = sender
                    .send(SpeedTestEvent::Error(
                        "Falha durante o teste de upload".into(),
                    ))
                    .await;
            }
            return None;
        }
    };

    let result = SpeedTestResult {
        ping_ms: round_two(ping_ms),
        jitter_ms: round_two(jitter_ms),
        download_mbps: round_two(download_mbps),
        upload_mbps: round_two(upload_mbps),
        server_name: Some("Cloudflare Edge Network".into()),
        server_location: Some(server_location),
        timestamp: Utc::now().to_rfc3339(),
    };

    let _ = sender
        .send(SpeedTestEvent::Progress(SpeedTestProgress {
            phase: "complete".into(),
            progress_pct: 100.0,
            current_mbps: None,
            ping_ms: Some(result.ping_ms),
            jitter_ms: Some(result.jitter_ms),
            download_mbps: Some(result.download_mbps),
            upload_mbps: Some(result.upload_mbps),
            server_name: result.server_name.clone(),
            server_location: result.server_location.clone(),
        }))
        .await;

    let _ = sender.send(SpeedTestEvent::Complete(result.clone())).await;
    Some(result)
}

async fn measure_ping_jitter(
    client: &Client,
    sender: &mpsc::Sender<SpeedTestEvent>,
    cancel: &CancellationToken,
    ping_count: usize,
) -> Option<(f64, f64, String)> {
    let mut latencies: Vec<f64> = Vec::with_capacity(ping_count);
    let ping_url = format!("{CLOUDFLARE_BASE}/__down?bytes=0");

    let _ = sender
        .send(SpeedTestEvent::Progress(SpeedTestProgress {
            phase: "ping".into(),
            progress_pct: 5.0,
            current_mbps: None,
            ping_ms: None,
            jitter_ms: None,
            download_mbps: None,
            upload_mbps: None,
            server_name: Some("Cloudflare Edge".into()),
            server_location: None,
        }))
        .await;

    for i in 0..ping_count {
        if cancel.is_cancelled() {
            return None;
        }

        let start = Instant::now();
        let resp = client.get(&ping_url).send().await.ok()?;
        let elapsed = duration_to_ms(start.elapsed());

        if resp.status().is_success() {
            latencies.push(elapsed);
        }

        let pct = 5.0 + ((i + 1) as f64 / ping_count as f64) * 15.0;
        let current_ping = if latencies.is_empty() {
            None
        } else {
            Some(round_two(
                latencies.iter().sum::<f64>() / latencies.len() as f64,
            ))
        };

        let _ = sender
            .send(SpeedTestEvent::Progress(SpeedTestProgress {
                phase: "ping".into(),
                progress_pct: pct,
                current_mbps: None,
                ping_ms: current_ping,
                jitter_ms: None,
                download_mbps: None,
                upload_mbps: None,
                server_name: Some("Cloudflare Edge".into()),
                server_location: None,
            }))
            .await;

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    if latencies.is_empty() {
        return None;
    }

    let avg_ping = latencies.iter().sum::<f64>() / latencies.len() as f64;
    let jitter = calculate_jitter(&latencies);

    // Consulta PoP / cidade no trace
    let location = fetch_cloudflare_location(client).await;

    Some((avg_ping, jitter, location))
}

async fn fetch_cloudflare_location(client: &Client) -> String {
    let trace_url = "https://1.1.1.1/cdn-cgi/trace";
    if let Ok(resp) = client.get(trace_url).send().await {
        if let Ok(text) = resp.text().await {
            for line in text.lines() {
                if let Some(colo) = line.strip_prefix("colo=") {
                    return match_colo_location(colo.trim());
                }
            }
        }
    }
    "São Paulo, BR".into()
}

fn match_colo_location(colo: &str) -> String {
    match colo.to_ascii_uppercase().as_str() {
        "GRU" => "São Paulo (GRU), BR".into(),
        "GIG" => "Rio de Janeiro (GIG), BR".into(),
        "BSB" => "Brasília (BSB), BR".into(),
        "FOR" => "Fortaleza (FOR), BR".into(),
        "POA" => "Porto Alegre (POA), BR".into(),
        "CWB" => "Curitiba (CWB), BR".into(),
        "CNF" => "Belo Horizonte (CNF), BR".into(),
        "MIA" => "Miami (MIA), EUA".into(),
        other => format!("Edge ({other})"),
    }
}

async fn measure_download(
    client: &Client,
    sender: &mpsc::Sender<SpeedTestEvent>,
    cancel: &CancellationToken,
    ping_ms: f64,
    jitter_ms: f64,
    location: &str,
    download_bytes: usize,
) -> Option<f64> {
    let url = format!("{CLOUDFLARE_BASE}/__down?bytes={download_bytes}");
    let resp = client.get(&url).send().await.ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let mut stream = resp.bytes_stream();
    let mut received_bytes: usize = 0;
    let start = Instant::now();
    let mut last_emit = Instant::now();

    while let Some(chunk_res) = stream.next().await {
        if cancel.is_cancelled() {
            return None;
        }

        let chunk = chunk_res.ok()?;
        received_bytes += chunk.len();

        if last_emit.elapsed() >= Duration::from_millis(150) {
            let elapsed_sec = start.elapsed().as_secs_f64().max(0.001);
            let mbps = (received_bytes as f64 * 8.0) / (elapsed_sec * 1_000_000.0);
            let pct = 20.0 + (received_bytes as f64 / download_bytes as f64).min(1.0) * 45.0;

            let _ = sender
                .send(SpeedTestEvent::Progress(SpeedTestProgress {
                    phase: "download".into(),
                    progress_pct: round_two(pct),
                    current_mbps: Some(round_two(mbps)),
                    ping_ms: Some(round_two(ping_ms)),
                    jitter_ms: Some(round_two(jitter_ms)),
                    download_mbps: Some(round_two(mbps)),
                    upload_mbps: None,
                    server_name: Some("Cloudflare Edge Network".into()),
                    server_location: Some(location.to_string()),
                }))
                .await;

            last_emit = Instant::now();
        }
    }

    let total_secs = start.elapsed().as_secs_f64().max(0.001);
    let final_mbps = (received_bytes as f64 * 8.0) / (total_secs * 1_000_000.0);
    Some(final_mbps)
}

struct UploadOptions<'a> {
    ping_ms: f64,
    jitter_ms: f64,
    download_mbps: f64,
    location: &'a str,
    bytes: usize,
    chunks: usize,
}

async fn measure_upload(
    client: &Client,
    sender: &mpsc::Sender<SpeedTestEvent>,
    cancel: &CancellationToken,
    opts: UploadOptions<'_>,
) -> Option<f64> {
    let url = format!("{CLOUDFLARE_BASE}/__up");
    let chunk_size = (opts.bytes / opts.chunks).max(1);
    let payload = vec![0u8; chunk_size];
    let start = Instant::now();
    let mut sent_bytes: usize = 0;

    let _ = sender
        .send(SpeedTestEvent::Progress(SpeedTestProgress {
            phase: "upload".into(),
            progress_pct: 68.0,
            current_mbps: None,
            ping_ms: Some(round_two(opts.ping_ms)),
            jitter_ms: Some(round_two(opts.jitter_ms)),
            download_mbps: Some(round_two(opts.download_mbps)),
            upload_mbps: None,
            server_name: Some("Cloudflare Edge Network".into()),
            server_location: Some(opts.location.to_string()),
        }))
        .await;

    for i in 0..opts.chunks {
        if cancel.is_cancelled() {
            return None;
        }

        let req = client.post(&url).body(payload.clone());
        let resp = req.send().await.ok()?;

        if !resp.status().is_success() || cancel.is_cancelled() {
            return None;
        }

        sent_bytes += chunk_size;
        let elapsed_sec = start.elapsed().as_secs_f64().max(0.001);
        let current_mbps = (sent_bytes as f64 * 8.0) / (elapsed_sec * 1_000_000.0);
        let pct = 68.0 + ((i + 1) as f64 / opts.chunks as f64) * 28.0;

        let _ = sender
            .send(SpeedTestEvent::Progress(SpeedTestProgress {
                phase: "upload".into(),
                progress_pct: round_two(pct),
                current_mbps: Some(round_two(current_mbps)),
                ping_ms: Some(round_two(opts.ping_ms)),
                jitter_ms: Some(round_two(opts.jitter_ms)),
                download_mbps: Some(round_two(opts.download_mbps)),
                upload_mbps: Some(round_two(current_mbps)),
                server_name: Some("Cloudflare Edge Network".into()),
                server_location: Some(opts.location.to_string()),
            }))
            .await;
    }

    let elapsed_sec = start.elapsed().as_secs_f64().max(0.001);
    let final_mbps = (sent_bytes as f64 * 8.0) / (elapsed_sec * 1_000_000.0);
    Some(final_mbps)
}

/// Calcula o jitter delegando para a implementação canônica em `icmp_probe`.
pub fn calculate_jitter(latencies: &[f64]) -> f64 {
    crate::services::network_tools::icmp_probe::calculate_jitter(latencies).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculo_de_jitter_correto() {
        // [10, 12, 11, 15]:
        // diffs: |12-10| = 2, |11-12| = 1, |15-11| = 4 -> soma = 7 / 3 = 2.333...
        let latencies = vec![10.0, 12.0, 11.0, 15.0];
        let jitter = calculate_jitter(&latencies);
        assert_eq!(jitter, 2.33);

        // Amostra única ou vazia dá 0.0
        assert_eq!(calculate_jitter(&[10.0]), 0.0);
        assert_eq!(calculate_jitter(&[]), 0.0);
    }

    #[test]
    fn mapeamento_de_colo_location() {
        assert!(match_colo_location("GRU").contains("São Paulo"));
        assert!(match_colo_location("GIG").contains("Rio de Janeiro"));
        assert!(match_colo_location("MIA").contains("Miami"));
        assert_eq!(match_colo_location("XYZ"), "Edge (XYZ)");
    }
}
