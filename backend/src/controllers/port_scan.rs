//! Endpoint NDJSON da varredura de portas.

use std::{convert::Infallible, net::IpAddr};

use axum::{
    body::{Body, Bytes},
    http::{header, StatusCode},
    response::Response,
};
use loco_rs::prelude::*;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::{
    dtos::resources::PortScanInput,
    services::{
        network_tools::port_scanner::{
            self, PortProtocol, PortScanEvent, ScanProfile, ScanStrategy, MAX_PORTS_PER_SCAN,
        },
        shared::errors::{AppError, AppResult},
    },
};

async fn scan(Json(mut input): Json<PortScanInput>) -> AppResult<Response> {
    let host = input
        .host
        .trim()
        .parse::<IpAddr>()
        .map_err(|_| AppError::validation("Informe um endereço IP válido para a varredura"))?;
    let protocol = PortProtocol::parse(&input.protocol)
        .ok_or_else(|| AppError::validation("Protocolo deve ser tcp ou udp"))?;
    input.ports.sort_unstable();
    input.ports.dedup();
    if input.ports.is_empty() || input.ports.len() > MAX_PORTS_PER_SCAN || input.ports.contains(&0)
    {
        return Err(AppError::validation(
            "Informe portas únicas entre 1 e 65535",
        ));
    }
    let timeout_ms = input.timeout_ms.unwrap_or(1_500);
    if !(100..=5_000).contains(&timeout_ms) {
        return Err(AppError::validation(
            "timeoutMs deve estar entre 100 e 5000",
        ));
    }
    let profile = match input.profile.as_deref() {
        None => ScanProfile::Reliable,
        Some(value) => ScanProfile::parse(value)
            .ok_or_else(|| AppError::validation("profile deve ser fast, reliable ou complete"))?,
    };

    let guard = crate::services::monitoring::execution_guard::try_acquire_port_scan(host)
        .ok_or_else(|| {
            AppError::conflict(
                "Já existe uma varredura de portas em andamento para este endereço IP",
            )
        })?;

    let (sender, receiver) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    let scan_cancel = cancel.clone();
    tokio::spawn(async move {
        let _guard = guard;
        let results = port_scanner::scan(
            host,
            &input.ports,
            protocol,
            ScanStrategy::for_profile(profile, timeout_ms),
            sender.clone(),
            scan_cancel.clone(),
        )
        .await;
        if !scan_cancel.is_cancelled() {
            let _ = sender.send(PortScanEvent::Done).await;
            tracing::info!(target = %host, scanned = results.len(), "stream da varredura concluído");
        }
    });
    let output = ReceiverStream::new(receiver).map(|event| {
        let line = match event {
            PortScanEvent::Result(item) => serde_json::json!({ "type": "result", "port": item.port, "protocol": item.protocol, "status": item.status, "service": item.service, "latencyMs": item.latency_ms, "attempts": item.attempts, "error": item.error }),
            PortScanEvent::Done => serde_json::json!({ "type": "done" }),
        };
        Ok::<Bytes, Infallible>(Bytes::from(line.to_string() + "\n"))
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-ndjson")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(output))
        .map_err(|error| AppError::Internal(error.into()))
}

pub fn routes() -> Routes {
    Routes::new().add("/port-scan", post(scan))
}
