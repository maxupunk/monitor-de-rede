//! Historico de alertas e stream SSE de eventos de dominio.

use axum::extract::Query;
use axum::http::{header, HeaderValue};
use axum::response::{
    sse::{Event, KeepAlive, Sse},
    IntoResponse,
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{EntityTrait, QueryOrder};
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

use crate::{
    dtos::{
        alerts::{
            HourlyDistributionBin, HourlyDistributionQuery, HourlyDistributionResponse,
            HourlyDistributionTotals,
        },
        resources::PaginationQuery,
    },
    models::alert_events,
    services::{
        events::{DomainEvent, EventBus},
        shared::{
            errors::AppResult,
            pagination::{paginate, PaginatedResponse},
        },
    },
    views::alerts::serialize_events,
};

/// `GET /api/events` — histórico paginado no envelope `{ data, meta }`, com
/// `device` e `monitor` achatados como na Central de Alertas.
async fn index(
    State(ctx): State<AppContext>,
    Query(query): Query<PaginationQuery>,
) -> AppResult<Response> {
    let page = paginate(
        &ctx.db,
        alert_events::Entity::find()
            .order_by_desc(crate::models::_entities::alert_events::Column::CreatedAt),
        query.page.unwrap_or(1),
        query.limit.unwrap_or(20),
        |row| row,
    )
    .await?;
    Ok(format::json(PaginatedResponse {
        data: serialize_events(&ctx.db, page.data).await?,
        meta: page.meta,
    })?)
}

async fn stream(State(ctx): State<AppContext>) -> AppResult<Response> {
    let bus = EventBus::from_context(&ctx)?;
    let mut updates = bus.subscribe();
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(async move {
        let connected = DomainEvent {
            event_type: "stream:connected".into(),
            payload: serde_json::json!({}),
            occurred_at: Utc::now().to_rfc3339(),
        };
        // `retry` vai junto do primeiro quadro (§11.1): o `EventSource` do
        // navegador tem backoff próprio, e sem esta linha uma queda de rede
        // pode deixar o painel mudo por muito mais do que os 3 s combinados.
        if sender.send((connected, true)).await.is_err() {
            return;
        }
        loop {
            match updates.recv().await {
                Ok(event) => {
                    if sender.send((event, false)).await.is_err() {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    let resync = DomainEvent {
                        event_type: "stream:resync".into(),
                        payload: serde_json::json!({ "skipped": skipped }),
                        occurred_at: Utc::now().to_rfc3339(),
                    };
                    if sender.send((resync, false)).await.is_err() {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });
    let stream = ReceiverStream::new(receiver).map(|(event, first)| {
        let frame =
            Event::default().data(serde_json::to_string(&event).unwrap_or_else(|_| "{}".into()));
        Ok::<Event, Infallible>(if first {
            frame.retry(std::time::Duration::from_millis(3_000))
        } else {
            frame
        })
    });
    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(std::time::Duration::from_secs(25))
                .text("keep-alive"),
        )
        .into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}

/// `GET /api/events/hourly-distribution` — agregação de eventos por hora (padrão últimas 6h).
async fn hourly_distribution(
    State(ctx): State<AppContext>,
    Query(query): Query<HourlyDistributionQuery>,
) -> AppResult<Response> {
    use chrono::{Duration, Timelike};
    use sea_orm::{ColumnTrait, QueryFilter};

    let hours_count = query.hours.unwrap_or(6).clamp(1, 48);
    let now = Utc::now();

    // 1. Inicializa os baldes de horas ordenados cronologicamente
    let mut bins: Vec<HourlyDistributionBin> = Vec::with_capacity(hours_count as usize);
    for i in 0..hours_count {
        let bin_time = now - Duration::hours(hours_count - 1 - i);
        let hour = bin_time.hour();
        let label = format!("{:02}:00", hour);
        bins.push(HourlyDistributionBin {
            label,
            hour,
            timestamp: bin_time.to_rfc3339(),
            critical: 0,
            warning: 0,
            info: 0,
        });
    }

    // 2. Consulta eventos criados desde o início do período
    let cutoff = now - Duration::hours(hours_count);
    let mut rows = alert_events::Entity::find()
        .filter(crate::models::_entities::alert_events::Column::CreatedAt.gte(cutoff))
        .filter(crate::models::_entities::alert_events::Column::CreatedAt.lte(now))
        .stream(&ctx.db)
        .await?;

    let mut totals = HourlyDistributionTotals {
        critical: 0,
        warning: 0,
        info: 0,
        total: 0,
    };

    // 3. Distribui os eventos nos baldes
    while let Some(row) = futures::TryStreamExt::try_next(&mut rows).await? {
        let row_time = row.created_at.with_timezone(&Utc);
        let hours_ago = (now - row_time).num_hours();

        if hours_ago >= 0 && hours_ago < hours_count {
            let bin_idx = (hours_count - 1 - hours_ago) as usize;
            if let Some(bin) = bins.get_mut(bin_idx) {
                match row.severity.to_lowercase().as_str() {
                    "critical" | "error" => {
                        bin.critical += 1;
                        totals.critical += 1;
                    }
                    "warning" => {
                        bin.warning += 1;
                        totals.warning += 1;
                    }
                    _ => {
                        bin.info += 1;
                        totals.info += 1;
                    }
                }
                totals.total += 1;
            }
        }
    }

    Ok(format::json(HourlyDistributionResponse {
        hours: hours_count,
        bins,
        totals,
    })?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/events")
        .add("/", get(index))
        .add("/stream", get(stream))
        .add("/hourly-distribution", get(hourly_distribution))
}
