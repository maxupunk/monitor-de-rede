//! O vigia dos aparelhos que deveriam estar sempre conectados.
//!
//! A cada ciclo (`WIFI_WATCH_INTERVAL_SECONDS`, só se houver vigiado) lê a
//! frota inteira — a mesma leitura da tela, que também sai pelo SSE — e
//! procura cada vigiado em todos os roteadores: trocar de AP (roaming) é estar
//! conectado. O que se viu vira fato para o motor de alertas; quanto tempo fora
//! é grave, quem decide é a regra.
//!
//! **Só a leitura completa prova ausência.** Roteador que não respondeu pode
//! estar com o aparelho: nesse ciclo ele fica sem fato, e a queda do roteador
//! já é alerta do monitor dele.

use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::EntityTrait;
use serde_json::{json, Map};

use super::{
    collector::{self, FleetReading},
    source::Requester,
    watchlist,
};
use crate::{
    dtos::wifi::{WifiDeviceSnapshot, WifiReadStatus, WifiStation},
    models::{devices, wifi_watched_clients},
    services::{
        alerts::{
            contracts::{AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey},
            datasets::wifi_client::{self, WifiClientFacts, WifiClientPresence},
            manager,
        },
        shared::errors::AppResult,
    },
};

/// O que a leitura disse de um vigiado.
#[derive(Debug, Clone, PartialEq)]
struct Observation {
    /// O roteador onde está (conectado) ou onde esteve por último (fora).
    device_id: Option<i64>,
    presence: WifiClientPresence,
}

/// Um ciclo do vigia; devolve quantos vigiados tiveram fato. Dois ciclos não
/// se sobrepõem: o segundo desiste enquanto o primeiro lê.
///
/// # Errors
///
/// Erro do banco.
pub async fn run(ctx: &AppContext) -> AppResult<usize> {
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::AcqRel) {
        return Ok(0);
    }
    let outcome = cycle(ctx).await;
    RUNNING.store(false, Ordering::Release);
    outcome
}

async fn cycle(ctx: &AppContext) -> AppResult<usize> {
    let watched = wifi_watched_clients::Entity::find().all(&ctx.db).await?;
    if watched.is_empty() {
        return Ok(0);
    }
    let reading = collector::read_fleet(ctx, Requester::watch()).await?;
    let now = Utc::now();
    let mut observed = 0;
    for client in &watched {
        let Some(observation) = observe(client, &reading, now) else {
            continue;
        };
        let connected_on = matches!(observation.presence, WifiClientPresence::Connected { .. })
            .then_some(observation.device_id)
            .flatten();
        watchlist::record(&ctx.db, client, connected_on, now).await?;
        evaluate(ctx, client, &observation).await?;
        observed += 1;
    }
    watchlist::publish(ctx).await;
    Ok(observed)
}

/// Onde o aparelho está; `None` quando a leitura não basta para dizer.
fn observe(
    client: &wifi_watched_clients::Model,
    reading: &FleetReading,
    now: DateTime<Utc>,
) -> Option<Observation> {
    if let Some((snapshot, station)) = find_station(&reading.snapshots, &client.mac) {
        return Some(Observation {
            device_id: Some(snapshot.device_id),
            presence: WifiClientPresence::Connected {
                device_name: snapshot.device_name.clone(),
                signal_dbm: station.signal_dbm,
                health: station.health,
                health_reason: station.health_reason.clone(),
            },
        });
    }
    if !reading.complete {
        return None;
    }
    let since = client.last_seen_at.unwrap_or(client.created_at);
    Some(Observation {
        device_id: client.last_device_id,
        presence: WifiClientPresence::Offline {
            seconds: (now - since.with_timezone(&Utc)).num_seconds().max(0),
        },
    })
}

/// A estação com o MAC nas leituras bem-sucedidas. Visto por dois APs no
/// meio de um roaming, vale o de sinal melhor.
pub(super) fn find_station<'a>(
    snapshots: &'a [WifiDeviceSnapshot],
    mac: &str,
) -> Option<(&'a WifiDeviceSnapshot, &'a WifiStation)> {
    snapshots
        .iter()
        .filter(|snapshot| snapshot.status == WifiReadStatus::Ok)
        .flat_map(|snapshot| {
            snapshot
                .stations
                .iter()
                .filter(|station| station.mac == mac)
                .map(move |station| (snapshot, station))
        })
        .max_by_key(|(_, station)| station.signal_dbm)
}

async fn evaluate(
    ctx: &AppContext,
    client: &wifi_watched_clients::Model,
    observation: &Observation,
) -> AppResult<()> {
    let device = match observation.device_id {
        Some(device_id) => devices::Entity::find_by_id(device_id).one(&ctx.db).await?,
        None => None,
    };
    let facts = WifiClientFacts {
        name: &client.name,
        presence: &observation.presence,
    };
    let dataset = wifi_client::build(&facts);
    let mut data = Map::new();
    data.insert("eventType".into(), json!("wifi_client_state"));
    data.insert("wifiWatchedId".into(), json!(client.id));
    data.insert("mac".into(), json!(client.mac));
    data.extend(dataset.clone());

    manager::evaluate(
        ctx,
        &AlertEvaluationContext {
            scope: AlertEvaluationScope {
                site_id: device.as_ref().and_then(|device| device.site_id),
                device_id: observation.device_id,
                monitor_id: None,
            },
            scope_key: AlertScopeKey::wifi_client(client.id),
            target_label: client.name.clone(),
            dataset,
            message: Some(wifi_client::describe(&facts)),
            data,
            recovered: wifi_client::is_recovery(&observation.presence),
            degraded: false,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;
    use crate::dtos::wifi::{WifiGrade, WifiHealth};

    const MAC: &str = "3c:84:6a:00:00:01";

    fn estacao(signal_dbm: i32) -> WifiStation {
        WifiStation {
            mac: MAC.into(),
            randomized_mac: false,
            vendor: None,
            ip: None,
            hostname: None,
            ssid: None,
            bssid: None,
            radio: None,
            band: None,
            channel: None,
            channel_width_mhz: None,
            signal_dbm,
            signal_avg_dbm: None,
            noise_dbm: None,
            snr_db: None,
            rx_rate_bps: None,
            tx_rate_bps: None,
            mcs: None,
            tx_retries_pct: None,
            inactive_ms: None,
            connected_s: None,
            signal_grade: WifiGrade::Good,
            snr_grade: None,
            retries_grade: None,
            health: WifiHealth::Healthy,
            health_reason: None,
        }
    }

    fn ap(
        device_id: i64,
        status: WifiReadStatus,
        stations: Vec<WifiStation>,
    ) -> WifiDeviceSnapshot {
        WifiDeviceSnapshot {
            device_id,
            device_name: format!("AP {device_id}"),
            status,
            read_at: None,
            error: None,
            stations,
            can_ping: false,
        }
    }

    fn vigiado(
        last_seen_minutes_ago: Option<i64>,
        now: DateTime<Utc>,
    ) -> wifi_watched_clients::Model {
        wifi_watched_clients::Model {
            id: 1,
            mac: MAC.into(),
            name: "Câmera".into(),
            last_state: None,
            last_seen_at: last_seen_minutes_ago
                .map(|minutes| (now - Duration::minutes(minutes)).into()),
            last_device_id: Some(7),
            created_by: None,
            created_at: (now - Duration::hours(2)).into(),
            updated_at: now.into(),
        }
    }

    #[test]
    fn no_roaming_vale_o_ap_de_sinal_melhor() {
        let now = Utc::now();
        let leitura = FleetReading {
            snapshots: vec![
                ap(1, WifiReadStatus::Ok, vec![estacao(-80)]),
                ap(2, WifiReadStatus::Ok, vec![estacao(-55)]),
            ],
            complete: true,
        };
        let visto = observe(&vigiado(Some(5), now), &leitura, now).expect("conectado");
        assert_eq!(visto.device_id, Some(2));
        assert!(matches!(
            visto.presence,
            WifiClientPresence::Connected {
                signal_dbm: -55,
                ..
            }
        ));
    }

    #[test]
    fn leitura_incompleta_nao_prova_ausencia() {
        let now = Utc::now();
        let leitura = FleetReading {
            snapshots: vec![ap(1, WifiReadStatus::Failed, Vec::new())],
            complete: false,
        };
        assert_eq!(observe(&vigiado(Some(5), now), &leitura, now), None);
    }

    #[test]
    fn ausente_conta_desde_a_ultima_vez_visto_ou_desde_o_cadastro() {
        let now = Utc::now();
        let leitura = FleetReading {
            snapshots: vec![ap(1, WifiReadStatus::Ok, Vec::new())],
            complete: true,
        };
        let fora = observe(&vigiado(Some(12), now), &leitura, now).expect("fora");
        assert_eq!(fora.device_id, Some(7), "o último AP onde esteve");
        assert_eq!(fora.presence, WifiClientPresence::Offline { seconds: 720 });

        let nunca_visto = observe(&vigiado(None, now), &leitura, now).expect("fora");
        assert_eq!(
            nunca_visto.presence,
            WifiClientPresence::Offline { seconds: 7_200 }
        );
    }

    #[test]
    fn estacoes_de_leitura_em_andamento_nao_contam() {
        let snapshots = vec![ap(1, WifiReadStatus::Reading, vec![estacao(-50)])];
        assert!(find_station(&snapshots, MAC).is_none());
    }
}
