//! O que o submódulo Wi-Fi contribui com a IA (ADR 017): a seção `wifi` do
//! `get_platform_status` — cada roteador/AP, as estações conectadas (as que
//! precisam de atenção primeiro) e os aparelhos vigiados.
//!
//! Lê o mesmo estado que a tela `/wifi` mostra (o snapshot guardado no
//! barramento). Sem nenhuma leitura desde o boot, lê a frota uma vez, como o
//! vigia faz — pelos plugins ativos, então sem pedir aprovação.

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use netmonitor_core::{
    ai::platform::PlatformSection,
    events::{DomainEvent, EventBus},
    shared::errors::AppResult,
};
use netmonitor_network::plugins::wifi_source::Requester;
use serde_json::{json, Value};
use tokio::{sync::broadcast, time::Instant};

use crate::{
    collector,
    dtos::{WifiBand, WifiDeviceSnapshot, WifiHealth, WifiReadStatus, WifiStation},
    rounds, watchlist,
};

/// Quanto a IA espera pela primeira leitura da frota.
const FIRST_READ_TIMEOUT: Duration = Duration::from_secs(25);

/// A seção `wifi` do estado da plataforma.
pub struct WifiStatus;

/// Da pior para a melhor — quem precisa de atenção vem primeiro.
const fn rank(health: WifiHealth) -> u8 {
    match health {
        WifiHealth::Critical => 0,
        WifiHealth::HighRetries => 1,
        WifiHealth::Fair => 2,
        WifiHealth::PowerSave => 3,
        WifiHealth::Healthy => 4,
    }
}

const fn needs_attention(health: WifiHealth) -> bool {
    matches!(
        health,
        WifiHealth::Critical | WifiHealth::HighRetries | WifiHealth::Fair
    )
}

const fn band(band: Option<WifiBand>) -> Option<&'static str> {
    match band {
        Some(WifiBand::Band2g) => Some("2.4GHz"),
        Some(WifiBand::Band5g) => Some("5GHz"),
        Some(WifiBand::Band6g) => Some("6GHz"),
        None => None,
    }
}

fn station_row(device: &str, station: &WifiStation) -> Value {
    json!({
        "device": device,
        "name": station.hostname,
        "mac": station.mac,
        "ip": station.ip,
        "vendor": station.vendor,
        "ssid": station.ssid,
        "band": band(station.band),
        "signal_dbm": station.signal_dbm,
        "snr_db": station.snr_db,
        "tx_retries_pct": station.tx_retries_pct,
        "health": station.health,
        "reason": station.health_reason,
        "connected_min": station.connected_s.map(|seconds| seconds / 60),
        // Faixa aproximada pelo sinal (o mapa); nunca uma medida.
        "distance_m": station.distance.map(|distance| format!(
            "~{:.0}–{:.0}",
            distance.min_m, distance.max_m
        )),
    })
}

/// O relatório a partir dos snapshots e dos vigiados — puro, testável.
#[must_use]
pub fn report(snapshots: &[WifiDeviceSnapshot], watched: Value, limit: usize) -> Value {
    let access_points: Vec<Value> = snapshots
        .iter()
        .map(|snapshot| {
            json!({
                "device_id": snapshot.device_id,
                "device": snapshot.device_name,
                "status": snapshot.status,
                "read_at": snapshot.read_at,
                "error": snapshot.error,
                "stations": snapshot.stations.len(),
                "stations_attention": snapshot
                    .stations
                    .iter()
                    .filter(|station| needs_attention(station.health))
                    .count(),
            })
        })
        .collect();

    let mut stations: Vec<(&str, &WifiStation)> = snapshots
        .iter()
        .flat_map(|snapshot| {
            snapshot
                .stations
                .iter()
                .map(|station| (snapshot.device_name.as_str(), station))
        })
        .collect();
    stations.sort_by_key(|(_, station)| (rank(station.health), station.signal_dbm));
    let total = stations.len();
    let attention = stations
        .iter()
        .filter(|(_, station)| needs_attention(station.health))
        .count();

    json!({
        "configured": true,
        "access_points": access_points,
        "stations_total": total,
        "stations_attention": attention,
        "stations": stations
            .iter()
            .take(limit)
            .map(|(device, station)| station_row(device, station))
            .collect::<Vec<_>>(),
        "omitted": total.saturating_sub(limit),
        "watched": watched,
    })
}

/// Equipamento que está na primeira leitura: em `Reading` e sem leitura
/// anterior, ou seja, sem estação nenhuma para mostrar ainda.
fn first_read_pending(snapshot: &WifiDeviceSnapshot) -> bool {
    snapshot.status == WifiReadStatus::Reading && snapshot.read_at.is_none()
}

fn settled(snapshots: &[WifiDeviceSnapshot]) -> bool {
    !snapshots.is_empty() && !snapshots.iter().any(first_read_pending)
}

/// Espera a rodada em curso (de outro pedido) terminar a primeira leitura,
/// acordando a cada `wifi:snapshot` do barramento — sem consultar nada em
/// laço. Ao fim do prazo, devolve o que houver.
async fn wait_settled(
    ctx: &AppContext,
    events: Option<broadcast::Receiver<DomainEvent>>,
    deadline: Instant,
) -> Vec<WifiDeviceSnapshot> {
    if let Some(mut events) = events {
        loop {
            let current = collector::current_snapshots(ctx);
            if settled(&current) {
                return current;
            }
            match tokio::time::timeout_at(deadline, events.recv()).await {
                Ok(Ok(_) | Err(broadcast::error::RecvError::Lagged(_))) => {}
                Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => break,
            }
        }
    }
    collector::current_snapshots(ctx)
}

async fn snapshots(ctx: &AppContext) -> AppResult<Vec<WifiDeviceSnapshot>> {
    // Assina antes de olhar o estado: o snapshot que termina a leitura não
    // pode passar entre a conferência e a espera.
    let events = EventBus::from_context(ctx).ok().map(|bus| bus.subscribe());
    let deadline = Instant::now() + FIRST_READ_TIMEOUT;
    let current = collector::current_snapshots(ctx);
    if settled(&current) {
        return Ok(current);
    }
    if !current.is_empty() {
        // Uma rodada já está lendo a frota pela primeira vez.
        return Ok(wait_settled(ctx, events, deadline).await);
    }
    // Executa em tarefa destacada passando por `rounds::run` (respeita
    // `RunGuard` e vigia): se o prazo de 25 s estourar, a leitura continua até
    // o fim em segundo plano e nunca deixa snapshots presos em `reading`.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let task_ctx = ctx.clone();
    tokio::spawn(async move {
        let result = rounds::run(&task_ctx, Requester::ai(None)).await;
        let _ = tx.send(result);
    });
    match tokio::time::timeout_at(deadline, rx).await {
        Ok(Ok(Ok(Some(outcome)))) => Ok(outcome.reading.snapshots),
        // Outra rodada estava em andamento: espera ela, dentro do mesmo prazo.
        Ok(Ok(Ok(None))) => Ok(wait_settled(ctx, events, deadline).await),
        Ok(Err(_)) | Err(_) => Ok(collector::current_snapshots(ctx)),
        Ok(Ok(Err(error))) => Err(error),
    }
}

#[async_trait]
impl PlatformSection for WifiStatus {
    fn key(&self) -> &'static str {
        "wifi"
    }

    async fn report(
        &self,
        ctx: &AppContext,
        _since: DateTime<Utc>,
        limit: usize,
    ) -> AppResult<Value> {
        let snapshots = snapshots(ctx).await?;
        if snapshots.is_empty() {
            return Ok(json!({
                "configured": false,
                "hint": "Nenhum equipamento com plugin Wi-Fi ativo (ou a leitura ainda não terminou).",
            }));
        }
        let watched: Vec<Value> = watchlist::list(&ctx.db)
            .await?
            .into_iter()
            .map(|client| {
                json!({
                    "name": client.name,
                    "mac": client.mac,
                    "state": client.state,
                    "device": client.device_name,
                    "last_seen_at": client.last_seen_at,
                })
            })
            .collect();
        Ok(report(&snapshots, Value::Array(watched), limit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtos::{WifiGrade, WifiReadStatus};

    fn estacao(mac: &str, signal: i32, health: WifiHealth) -> WifiStation {
        WifiStation {
            mac: mac.into(),
            randomized_mac: false,
            vendor: None,
            ip: None,
            hostname: Some(format!("host-{mac}")),
            ssid: Some("casa".into()),
            bssid: None,
            radio: None,
            band: Some(WifiBand::Band5g),
            channel: None,
            channel_width_mhz: None,
            frequency_mhz: None,
            tx_power_dbm: None,
            signal_dbm: signal,
            signal_avg_dbm: None,
            noise_dbm: None,
            snr_db: None,
            rx_rate_bps: None,
            tx_rate_bps: None,
            mcs: None,
            tx_retries_pct: None,
            inactive_ms: None,
            connected_s: Some(600),
            signal_grade: WifiGrade::Good,
            snr_grade: None,
            retries_grade: None,
            health,
            health_reason: None,
            link_score: 1.0,
            distance: None,
        }
    }

    fn ap(id: i64, nome: &str, stations: Vec<WifiStation>) -> WifiDeviceSnapshot {
        WifiDeviceSnapshot {
            device_id: id,
            device_name: nome.into(),
            status: WifiReadStatus::Ok,
            read_at: None,
            error: None,
            stations,
            can_ping: false,
        }
    }

    #[test]
    fn estacoes_com_problema_vem_primeiro_entre_todos_os_aps() {
        let snapshots = [
            ap(1, "ap-sala", vec![estacao("a", -50, WifiHealth::Healthy)]),
            ap(
                2,
                "ap-quarto",
                vec![
                    estacao("b", -60, WifiHealth::Fair),
                    estacao("c", -82, WifiHealth::Critical),
                ],
            ),
        ];
        let out = report(&snapshots, json!([]), 2);
        assert_eq!(out["stations_total"], 3);
        assert_eq!(out["stations_attention"], 2);
        assert_eq!(out["stations"][0]["mac"], "c");
        assert_eq!(out["stations"][0]["device"], "ap-quarto");
        assert_eq!(out["stations"][0]["band"], "5GHz");
        assert_eq!(out["stations"][0]["connected_min"], 10);
        assert_eq!(out["stations"][1]["mac"], "b");
        assert_eq!(out["omitted"], 1);
        assert_eq!(out["access_points"][1]["stations_attention"], 2);
    }

    #[test]
    fn primeira_leitura_em_curso_nao_e_estado_pronto() {
        let mut lendo = ap(1, "ap-sala", Vec::new());
        lendo.status = WifiReadStatus::Reading;
        assert!(!settled(&[]), "sem leitura nenhuma não há o que entregar");
        assert!(!settled(&[ap(2, "ap-quarto", Vec::new()), lendo.clone()]));

        // Relendo com a leitura anterior em mãos: as estações já servem.
        lendo.read_at = Some("2026-11-25T10:00:00Z".into());
        assert!(settled(&[lendo]));
    }
}
