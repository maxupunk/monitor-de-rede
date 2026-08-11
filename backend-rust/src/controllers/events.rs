//! Historico de alertas e stream SSE de eventos de dominio.

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
    models::alert_events,
    services::{
        events::{DomainEvent, EventBus},
        shared::errors::AppResult,
    },
};

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let events = alert_events::Entity::find()
        .order_by_desc(crate::models::_entities::alert_events::Column::CreatedAt)
        .all(&ctx.db)
        .await?;
    Ok(format::json(events)?)
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
        if sender.send(connected).await.is_err() {
            return;
        }
        loop {
            match updates.recv().await {
                Ok(event) => {
                    if sender.send(event).await.is_err() {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    let resync = DomainEvent {
                        event_type: "stream:resync".into(),
                        payload: serde_json::json!({ "skipped": skipped }),
                        occurred_at: Utc::now().to_rfc3339(),
                    };
                    if sender.send(resync).await.is_err() {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });
    let stream = ReceiverStream::new(receiver).map(|event| {
        Ok::<Event, Infallible>(
            Event::default().data(serde_json::to_string(&event).unwrap_or_else(|_| "{}".into())),
        )
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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/events")
        .add("/", get(index))
        .add("/stream", get(stream))
}
