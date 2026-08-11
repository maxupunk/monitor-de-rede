//! Telemetria dos túneis (§8.10.2).
//!
//! O container WireGuard publica periodicamente a saída de `wg show <iface>
//! dump` no volume compartilhado (`<iface>.status`). O servidor apenas **lê**
//! esse arquivo — assim continua sem `NET_ADMIN` e sem acesso ao socket do
//! Docker.

use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
};

use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};

use crate::{
    models::vpn_peers,
    services::{
        shared::errors::AppResult,
        vpn::config_writer::{resolve_config_dir, VpnConfigSink},
    },
};

const NONE: &str = "(none)";
const OFF: &str = "off";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgPeerStatus {
    pub public_key: String,
    pub preshared_key: Option<String>,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    pub latest_handshake_at: Option<DateTime<Utc>>,
    pub bytes_rx: i64,
    pub bytes_tx: i64,
    pub persistent_keepalive: i32,
}

/// Parser de `wg show <iface> dump`.
///
/// A primeira linha descreve a interface (4 campos) e as seguintes descrevem os
/// peers (8 campos, separados por TAB). Linhas com menos de 8 colunas são
/// descartadas — é assim que a linha da interface se elimina sozinha, sem
/// depender de ela ser a primeira.
#[must_use]
pub fn parse_wg_dump(dump: &str) -> Vec<WgPeerStatus> {
    dump.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let columns: Vec<&str> = line.split('\t').collect();
            if columns.len() < 8 {
                return None;
            }
            let handshake_seconds: i64 = columns[4].parse().unwrap_or(0);
            Some(WgPeerStatus {
                public_key: columns[0].to_string(),
                preshared_key: optional(columns[1]),
                endpoint: optional(columns[2]),
                allowed_ips: if columns[3] == NONE {
                    Vec::new()
                } else {
                    columns[3]
                        .split(',')
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                        .collect()
                },
                latest_handshake_at: (handshake_seconds > 0)
                    .then(|| DateTime::from_timestamp(handshake_seconds, 0))
                    .flatten(),
                bytes_rx: columns[5].parse().unwrap_or(0),
                bytes_tx: columns[6].parse().unwrap_or(0),
                persistent_keepalive: if columns[7] == OFF {
                    0
                } else {
                    columns[7].parse().unwrap_or(0)
                },
            })
        })
        .collect()
}

fn optional(value: &str) -> Option<String> {
    (value != NONE).then(|| value.to_string())
}

/// Interfaces cujo dump já foi reportado como ausente.
///
/// Um `<iface>.status` ilegível é indistinguível de "nada mudou": o sink devolve
/// `None` e a sincronização vira um no-op. Sem este aviso, um processo sem o
/// volume compartilhado publica telemetria congelada indefinidamente e em
/// silêncio. Guarda o estado para avisar na **transição**, não a cada ciclo.
fn missing_dump_warned() -> &'static Mutex<HashSet<String>> {
    static WARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    WARNED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Lê e interpreta o dump publicado pelo container WireGuard.
pub async fn read_status(sink: &dyn VpnConfigSink, interface_name: &str) -> Vec<WgPeerStatus> {
    let Some(dump) = sink.read(&format!("{interface_name}.status")).await else {
        if let Ok(mut warned) = missing_dump_warned().lock() {
            if warned.insert(interface_name.to_string()) {
                tracing::warn!(
                    interface = interface_name,
                    dir = %resolve_config_dir().display(),
                    "o arquivo de status do WireGuard não pôde ser lido; a telemetria dos \
                     túneis não será atualizada por este processo — confira se o volume \
                     `wg-config` está montado neste container"
                );
            }
        }
        return Vec::new();
    };

    if let Ok(mut warned) = missing_dump_warned().lock() {
        if warned.remove(interface_name) {
            tracing::info!(
                interface = interface_name,
                "status do WireGuard voltou a ser legível"
            );
        }
    }
    parse_wg_dump(&dump)
}

/// Atualiza handshake, contadores de tráfego e sinal de vida dos peers.
///
/// Devolve quantas linhas mudaram.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn sync_peers<C: ConnectionTrait>(
    db: &C,
    sink: &dyn VpnConfigSink,
    interface_name: &str,
    vpn_server_id: i64,
) -> AppResult<u64> {
    let statuses = read_status(sink, interface_name).await;
    if statuses.is_empty() {
        return Ok(0);
    }
    let by_public_key: HashMap<&str, &WgPeerStatus> = statuses
        .iter()
        .map(|status| (status.public_key.as_str(), status))
        .collect();

    let peers = vpn_peers::Entity::find()
        .filter(vpn_peers::Column::VpnServerId.eq(vpn_server_id))
        .all(db)
        .await?;
    let now = Utc::now();
    let mut updated = 0;

    for peer in peers {
        let Some(status) = by_public_key.get(peer.public_key.as_str()) else {
            continue;
        };
        let Some(changes) = compute_changes(&peer, status, now) else {
            continue;
        };

        let mut active: vpn_peers::ActiveModel = peer.into();
        active.bytes_rx = Set(changes.bytes_rx);
        active.bytes_tx = Set(changes.bytes_tx);
        active.last_handshake_at = Set(changes.last_handshake_at.map(Into::into));
        if let Some(last_seen_at) = changes.last_seen_at {
            active.last_seen_at = Set(Some(last_seen_at.into()));
        }
        active.update(db).await?;
        updated += 1;
    }

    Ok(updated)
}

/// O que muda numa linha a partir de uma leitura do dump, ou `None` quando nada
/// mudou — evita um `UPDATE` por peer por ciclo num túnel parado.
struct PeerChanges {
    bytes_rx: i64,
    bytes_tx: i64,
    last_handshake_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
}

fn compute_changes(
    peer: &vpn_peers::Model,
    status: &WgPeerStatus,
    now: DateTime<Utc>,
) -> Option<PeerChanges> {
    let previous_handshake = peer.last_handshake_at.map(|value| value.to_utc());
    // Handshake ausente no dump não apaga o que já se sabia: o `wg` esquece o
    // último handshake quando a interface reinicia, e zerar aqui faria um peer
    // saudável parecer "aguardando primeira conexão".
    let last_handshake_at = status.latest_handshake_at.or(previous_handshake);

    // Contador de RX subiu desde a leitura anterior: chegou pelo menos um
    // keepalive, então o túnel está vivo **agora** — independente de o handshake
    // ser antigo. Queda de contador significa interface reiniciada, não vida.
    let received_new_bytes = status.bytes_rx > peer.bytes_rx;
    let renegotiated = match (last_handshake_at, previous_handshake) {
        (Some(_), None) => true,
        (Some(current), Some(previous)) => current > previous,
        _ => false,
    };
    let last_seen_at = (received_new_bytes || renegotiated).then_some(now);

    let changed = status.bytes_rx != peer.bytes_rx
        || status.bytes_tx != peer.bytes_tx
        || last_handshake_at != previous_handshake
        || last_seen_at.is_some();

    changed.then_some(PeerChanges {
        bytes_rx: status.bytes_rx,
        bytes_tx: status.bytes_tx,
        last_handshake_at,
        last_seen_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    const DUMP: &str = "\
CHAVE-PRIVADA-SERVIDOR\tCHAVE-PUBLICA-SERVIDOR\t51820\toff
PUB-A\t(none)\t203.0.113.9:51820\t10.8.0.11/32\t1700000000\t2048\t4096\t25
PUB-B\tPSK-B\t(none)\t(none)\t0\t0\t0\toff";

    #[test]
    fn a_linha_da_interface_e_descartada() {
        let peers = parse_wg_dump(DUMP);
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].public_key, "PUB-A");
    }

    #[test]
    fn interpreta_os_sentinelas_do_wg() {
        let peers = parse_wg_dump(DUMP);
        assert_eq!(peers[0].preshared_key, None);
        assert_eq!(peers[0].endpoint.as_deref(), Some("203.0.113.9:51820"));
        assert_eq!(peers[0].allowed_ips, vec!["10.8.0.11/32".to_string()]);
        assert_eq!(peers[0].persistent_keepalive, 25);

        assert_eq!(peers[1].preshared_key.as_deref(), Some("PSK-B"));
        assert_eq!(peers[1].endpoint, None);
        assert!(peers[1].allowed_ips.is_empty());
        assert_eq!(peers[1].persistent_keepalive, 0);
    }

    #[test]
    fn handshake_zero_significa_nunca_conectou() {
        let peers = parse_wg_dump(DUMP);
        assert!(peers[0].latest_handshake_at.is_some());
        assert_eq!(peers[1].latest_handshake_at, None);
    }

    #[test]
    fn dump_vazio_ou_ilegivel_nao_produz_peers() {
        assert!(parse_wg_dump("").is_empty());
        assert!(parse_wg_dump("   \n\n").is_empty());
        assert!(parse_wg_dump("lixo sem tabs").is_empty());
    }

    fn peer(bytes_rx: i64, handshake_ago: Option<i64>) -> vpn_peers::Model {
        let now = Utc::now();
        vpn_peers::Model {
            id: 1,
            vpn_server_id: 1,
            device_id: 1,
            public_key: "PUB-A".into(),
            preshared_key_encrypted: None,
            device_profile: "linux".into(),
            persistent_keepalive: 25,
            last_handshake_at: handshake_ago.map(|s| (now - Duration::seconds(s)).into()),
            last_seen_at: None,
            bytes_rx,
            bytes_tx: 0,
            enabled: true,
            last_connection_status: None,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    fn status(bytes_rx: i64, handshake: Option<DateTime<Utc>>) -> WgPeerStatus {
        WgPeerStatus {
            public_key: "PUB-A".into(),
            preshared_key: None,
            endpoint: None,
            allowed_ips: Vec::new(),
            latest_handshake_at: handshake,
            bytes_rx,
            bytes_tx: 0,
            persistent_keepalive: 25,
        }
    }

    #[test]
    fn rx_crescente_e_sinal_de_vida_mesmo_com_handshake_antigo() {
        let now = Utc::now();
        let changes = compute_changes(&peer(100, Some(3_600)), &status(200, None), now)
            .expect("houve mudança");
        assert_eq!(changes.last_seen_at, Some(now));
        assert_eq!(changes.bytes_rx, 200);
    }

    #[test]
    fn queda_de_contador_e_reinicio_e_nao_vida() {
        let now = Utc::now();
        let changes =
            compute_changes(&peer(500, Some(60)), &status(10, None), now).expect("houve mudança");
        // Os bytes são atualizados, mas isso **não** conta como keepalive.
        assert_eq!(changes.bytes_rx, 10);
        assert_eq!(changes.last_seen_at, None);
    }

    #[test]
    fn handshake_mais_novo_conta_como_vida() {
        let now = Utc::now();
        let changes = compute_changes(&peer(100, Some(3_600)), &status(100, Some(now)), now)
            .expect("houve mudança");
        assert_eq!(changes.last_seen_at, Some(now));
    }

    #[test]
    fn tunel_parado_nao_gera_escrita() {
        let mut linha = peer(100, Some(60));
        let handshake = linha.last_handshake_at.unwrap().to_utc();
        linha.bytes_tx = 0;
        assert!(compute_changes(&linha, &status(100, Some(handshake)), Utc::now()).is_none());
    }

    #[test]
    fn dump_sem_handshake_nao_apaga_o_que_ja_se_sabia() {
        let linha = peer(100, Some(120));
        let anterior = linha.last_handshake_at.unwrap().to_utc();
        let changes = compute_changes(&linha, &status(200, None), Utc::now()).expect("mudou");
        assert_eq!(changes.last_handshake_at, Some(anterior));
    }
}
