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
            self, PortProtocol, PortScanEvent, ScanStrategy, MAX_PORTS_PER_SCAN,
        },
        shared::errors::{AppError, AppResult},
    },
};

async fn scan(Json(input): Json<PortScanInput>) -> AppResult<Response> {
    let host = input
        .host
        .trim()
        .parse::<IpAddr>()
        .map_err(|_| AppError::validation("Informe um endereço IP válido para a varredura"))?;
    let protocol = PortProtocol::parse(&input.protocol)
        .ok_or_else(|| AppError::validation("Protocolo deve ser tcp ou udp"))?;
    if input.ports.is_empty() || input.ports.len() > MAX_PORTS_PER_SCAN {
        return Err(AppError::validation("Informe entre 1 e 1024 portas"));
    }
    let timeout_ms = input.timeout_ms.unwrap_or(1_500);
    if !(100..=5_000).contains(&timeout_ms) {
        return Err(AppError::validation(
            "timeoutMs deve estar entre 100 e 5000",
        ));
    }

    let (sender, receiver) = mpsc::channel(32);
    let cancel = CancellationToken::new();
    let scan_cancel = cancel.clone();
    tokio::spawn(async move {
        port_scanner::scan(
            host,
            &input.ports,
            protocol,
            ScanStrategy::with_timeout(timeout_ms),
            sender.clone(),
            scan_cancel.clone(),
        )
        .await;
        if !scan_cancel.is_cancelled() {
            let _ = sender.send(PortScanEvent::Done).await;
        }
    });
    let output = ReceiverStream::new(receiver).map(|event| {
        let line = match event {
            PortScanEvent::Result(item) => serde_json::json!({ "type": "result", "port": item.port, "protocol": item.protocol, "status": item.status, "service": item.service, "latencyMs": item.latency_ms }),
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
