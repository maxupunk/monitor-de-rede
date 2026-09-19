//! Endpoints de diagnóstico de rede: traceroute, speedtest e playbooks com streaming NDJSON.

use std::{convert::Infallible, net::IpAddr, time::Instant};

use axum::{
    body::{Body, Bytes},
    extract::Query,
    http::{header, StatusCode},
    response::Response,
};
use futures::StreamExt;
use loco_rs::prelude::*;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::{
    dtos::diagnostics::{PlaybookRunInput, TracerouteInput},
    models::devices,
    services::{
        monitoring::execution_guard::{try_acquire_speedtest, try_acquire_traceroute},
        network_tools::{
            playbook::{
                run_device_reachability_playbook, run_internet_health_playbook, PlaybookEvent,
            },
            speedtest::{execute_wan_speedtest, SpeedTestEvent},
            traceroute::{execute_traceroute, TracerouteEvent, TracerouteOptions},
        },
        shared::errors::{AppError, AppResult},
    },
};

const DEFAULT_LAN_DOWNLOAD_BYTES: usize = 25_000_000; // 25 MB
const MAX_LAN_DOWNLOAD_BYTES: usize = 100_000_000; // 100 MB
const LAN_CHUNK_SIZE: usize = 65_536; // 64 KB

#[derive(Debug, Deserialize)]
pub struct LanDownloadQuery {
    pub bytes: Option<usize>,
}

async fn traceroute(Json(input): Json<TracerouteInput>) -> AppResult<Response> {
    let host_str = input.host.trim();
    if host_str.is_empty() {
        return Err(AppError::validation(
            "Informe um host ou endereço IP válido",
        ));
    }

    let ip: IpAddr = if let Ok(ip) = host_str.parse() {
        ip
    } else {
        match tokio::net::lookup_host((host_str, 0)).await {
            Ok(mut addrs) => addrs.next().map(|a| a.ip()).ok_or_else(|| {
                AppError::validation("Nenhum IP encontrado para o host informado")
            })?,
            Err(err) => {
                return Err(AppError::validation(format!(
                    "Falha ao resolver DNS do host {host_str}: {err}"
                )))
            }
        }
    };

    let guard = try_acquire_traceroute(ip).ok_or_else(|| {
        AppError::conflict("Já existe um traceroute em andamento para este endereço IP no momento")
    })?;

    let options = TracerouteOptions::new(input.max_hops, input.timeout_ms, input.probes_per_hop);
    let (sender, receiver) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();

    tokio::spawn(async move {
        let _guard = guard;
        let hops = execute_traceroute(ip, options, sender.clone(), task_cancel.clone()).await;
        if !task_cancel.is_cancelled() {
            let _ = sender.send(TracerouteEvent::Done).await;
            tracing::info!(target = %ip, hops = hops.len(), "stream de traceroute concluído");
        }
    });

    let output = ReceiverStream::new(receiver).map(|event| {
        let line = match event {
            TracerouteEvent::Hop(h) => serde_json::json!({
                "type": "hop",
                "hop": h.hop,
                "ip": h.ip,
                "hostname": h.hostname,
                "rttMs": h.rtt_ms,
                "avgRttMs": h.avg_rtt_ms,
                "status": h.status
            }),
            TracerouteEvent::Done => serde_json::json!({ "type": "done" }),
        };
        Ok::<Bytes, Infallible>(Bytes::from(line.to_string() + "\n"))
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-ndjson")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(output))
        .map_err(|err| AppError::Internal(err.into()))
}

async fn speedtest() -> AppResult<Response> {
    let guard = try_acquire_speedtest().ok_or_else(|| {
        AppError::conflict("Já existe um teste de velocidade em andamento no servidor")
    })?;

    let (sender, receiver) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();

    tokio::spawn(async move {
        let _guard = guard;
        let _res = execute_wan_speedtest(sender.clone(), task_cancel.clone()).await;
        if !task_cancel.is_cancelled() {
            let _ = sender
                .send(SpeedTestEvent::Progress(
                    crate::dtos::diagnostics::SpeedTestProgress {
                        phase: "done".into(),
                        progress_pct: 100.0,
                        current_mbps: None,
                        ping_ms: None,
                        jitter_ms: None,
                        download_mbps: None,
                        upload_mbps: None,
                        server_name: None,
                        server_location: None,
                    },
                ))
                .await;
        }
    });

    let output = ReceiverStream::new(receiver).map(|event| {
        let line = match event {
            SpeedTestEvent::Progress(p) => serde_json::json!({
                "type": "progress",
                "phase": p.phase,
                "progressPct": p.progress_pct,
                "currentMbps": p.current_mbps,
                "pingMs": p.ping_ms,
                "jitterMs": p.jitter_ms,
                "downloadMbps": p.download_mbps,
                "uploadMbps": p.upload_mbps,
                "serverName": p.server_name,
                "serverLocation": p.server_location
            }),
            SpeedTestEvent::Complete(c) => serde_json::json!({
                "type": "complete",
                "result": c
            }),
            SpeedTestEvent::Error(err) => serde_json::json!({
                "type": "error",
                "message": err
            }),
        };
        Ok::<Bytes, Infallible>(Bytes::from(line.to_string() + "\n"))
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-ndjson")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(output))
        .map_err(|err| AppError::Internal(err.into()))
}

async fn lan_download(Query(query): Query<LanDownloadQuery>) -> AppResult<Response> {
    let total_bytes = query
        .bytes
        .unwrap_or(DEFAULT_LAN_DOWNLOAD_BYTES)
        .clamp(0, MAX_LAN_DOWNLOAD_BYTES);

    if total_bytes == 0 {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(header::CONTENT_LENGTH, "0")
            .header(header::CACHE_CONTROL, "no-cache, no-store")
            .body(Body::empty())
            .map_err(|err| AppError::Internal(err.into()));
    }

    let stream = futures::stream::unfold((0usize, total_bytes), |(mut sent, total)| async move {
        if sent >= total {
            None
        } else {
            let to_send = (total - sent).min(LAN_CHUNK_SIZE);
            sent += to_send;
            let chunk = vec![0x55u8; to_send];
            Some((Ok::<Bytes, Infallible>(Bytes::from(chunk)), (sent, total)))
        }
    })
    .fuse();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, total_bytes.to_string())
        .header(header::CACHE_CONTROL, "no-cache, no-store")
        .body(Body::from_stream(stream))
        .map_err(|err| AppError::Internal(err.into()))
}

async fn lan_upload(body: Body) -> AppResult<Response> {
    let start = Instant::now();
    let mut stream = body.into_data_stream();
    let mut received_bytes: usize = 0;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|err| AppError::Internal(err.into()))?;
        received_bytes += chunk.len();
    }

    let elapsed_sec = start.elapsed().as_secs_f64().max(0.001);
    let mbps = (received_bytes as f64 * 8.0) / (elapsed_sec * 1_000_000.0);

    let res = serde_json::json!({
        "bytesReceived": received_bytes,
        "durationMs": (elapsed_sec * 1_000.0).round(),
        "mbps": (mbps * 100.0).round() / 100.0
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(res.to_string()))
        .map_err(|err| AppError::Internal(err.into()))
}

async fn run_playbook(
    State(ctx): State<AppContext>,
    Json(input): Json<PlaybookRunInput>,
) -> AppResult<Response> {
    let (sender, receiver) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();

    match input.playbook_type.as_str() {
        "internet_health" => {
            tokio::spawn(async move {
                let _ = run_internet_health_playbook(sender, task_cancel).await;
            });
        }
        "device_reachability" => {
            let (target_ip, device_name) = if let Some(dev_id) = input.device_id {
                let dev = devices::Entity::find_by_id(dev_id)
                    .one(&ctx.db)
                    .await?
                    .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
                let ip_str = dev.ip_address.ok_or_else(|| {
                    AppError::validation("O dispositivo selecionado não possui endereço IP")
                })?;
                let ip: IpAddr = ip_str
                    .parse()
                    .map_err(|_| AppError::validation("Endereço IP do dispositivo inválido"))?;
                (ip, Some(dev.name))
            } else if let Some(target_str) = input.target {
                let ip: IpAddr = target_str.trim().parse().map_err(|_| {
                    AppError::validation("Informe um endereço IP válido para o dispositivo")
                })?;
                (ip, None)
            } else {
                return Err(AppError::validation(
                    "Informe deviceId ou target para o playbook de dispositivo",
                ));
            };

            tokio::spawn(async move {
                let _ =
                    run_device_reachability_playbook(target_ip, device_name, sender, task_cancel)
                        .await;
            });
        }
        _ => {
            return Err(AppError::validation(
                "Tipo de playbook inválido. Use internet_health ou device_reachability",
            ));
        }
    }

    let output = ReceiverStream::new(receiver).map(|event| {
        let line = match event {
            PlaybookEvent::Step(s) => serde_json::json!({
                "type": "step",
                "step": s
            }),
            PlaybookEvent::Summary(sum) => serde_json::json!({
                "type": "summary",
                "summary": sum
            }),
            PlaybookEvent::Done => serde_json::json!({ "type": "done" }),
        };
        Ok::<Bytes, Infallible>(Bytes::from(line.to_string() + "\n"))
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-ndjson")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(output))
        .map_err(|err| AppError::Internal(err.into()))
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/diagnostics/traceroute", post(traceroute))
        .add("/diagnostics/speedtest", post(speedtest))
        .add("/diagnostics/speedtest/lan/download", get(lan_download))
        .add(
            "/diagnostics/speedtest/lan/upload",
            post(lan_upload).layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .add("/diagnostics/playbook/run", post(run_playbook))
}
