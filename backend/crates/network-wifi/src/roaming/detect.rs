//! A detecção de roaming, pura: onde cada aparelho está numa rodada de
//! leitura e quem mudou de AP desde a anterior.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Duration, Utc};

use crate::{
    dtos::{WifiDeviceSnapshot, WifiReadStatus, WifiStation},
    station,
};

/// Sumiu por mais que isto e voltou noutro AP: é uma reconexão, não roaming
/// (o celular que saiu de manhã e voltou à tarde).
pub const ROAM_GAP: Duration = Duration::minutes(10);

/// Dois APs listam o mesmo aparelho e um deles o vê parado há este tanto a
/// mais: aquele é a sobra da associação antiga, não onde ele está.
const STALE_MARGIN_MS: u64 = 10_000;

/// Onde um aparelho está numa rodada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub device_id: i64,
    pub device_name: String,
    pub signal_dbm: i32,
    pub inactive_ms: Option<u64>,
    /// Como chamar o aparelho no alerta (nome do DHCP, fabricante ou MAC).
    pub label: String,
}

impl Location {
    fn of(snapshot: &WifiDeviceSnapshot, station: &WifiStation) -> Self {
        Self {
            device_id: snapshot.device_id,
            device_name: snapshot.device_name.clone(),
            signal_dbm: station.signal_dbm,
            inactive_ms: station.inactive_ms,
            label: station::label(station),
        }
    }

    /// Esta leitura do aparelho é mais confiável que `other`? A de atividade
    /// mais recente, com folga; empatadas, a de sinal melhor.
    fn beats(&self, other: &Self) -> bool {
        let (mine, theirs) = (
            self.inactive_ms.unwrap_or(0),
            other.inactive_ms.unwrap_or(0),
        );
        if mine.saturating_add(STALE_MARGIN_MS) < theirs {
            return true;
        }
        if theirs.saturating_add(STALE_MARGIN_MS) < mine {
            return false;
        }
        self.signal_dbm > other.signal_dbm
    }
}

/// Onde cada aparelho está, pelas leituras bem-sucedidas da rodada.
#[must_use]
pub fn locate(round: &[WifiDeviceSnapshot]) -> BTreeMap<String, Location> {
    let mut found: BTreeMap<String, Location> = BTreeMap::new();
    for snapshot in round
        .iter()
        .filter(|snapshot| snapshot.status == WifiReadStatus::Ok)
    {
        for station in &snapshot.stations {
            let candidate = Location::of(snapshot, station);
            let replace = found
                .get(&station.mac)
                .is_none_or(|current| candidate.beats(current));
            if replace {
                found.insert(station.mac.clone(), candidate);
            }
        }
    }
    found
}

/// Onde o aparelho foi visto por último, e quando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub location: Location,
    pub at: DateTime<Utc>,
}

/// Um aparelho que trocou de AP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roam {
    pub mac: String,
    pub from: Location,
    pub to: Location,
}

/// Compara a rodada com o que se sabia e devolve as trocas; atualiza `known`.
/// Aparelho que não aparece na rodada continua onde estava até passar
/// [`ROAM_GAP`] sem ser visto.
pub fn detect(
    known: &mut HashMap<String, Seen>,
    current: &BTreeMap<String, Location>,
    now: DateTime<Utc>,
) -> Vec<Roam> {
    let mut roams = Vec::new();
    for (mac, location) in current {
        if let Some(previous) = known.get(mac) {
            if previous.location.device_id != location.device_id && now - previous.at <= ROAM_GAP {
                roams.push(Roam {
                    mac: mac.clone(),
                    from: previous.location.clone(),
                    to: location.clone(),
                });
            }
        }
        known.insert(
            mac.clone(),
            Seen {
                location: location.clone(),
                at: now,
            },
        );
    }
    known.retain(|_, seen| now - seen.at <= ROAM_GAP);
    roams
}

/// Trocas demais na janela?
#[must_use]
pub const fn is_ping_pong(count: u64, threshold: u32) -> bool {
    count >= threshold as u64
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::dtos::{WifiGrade, WifiHealth};

    pub(crate) fn estacao(mac: &str, signal_dbm: i32, inactive_ms: u64) -> WifiStation {
        WifiStation {
            mac: mac.into(),
            randomized_mac: false,
            vendor: None,
            ip: None,
            hostname: Some(format!("host-{}", &mac[mac.len() - 2..])),
            ssid: Some("Casa".into()),
            bssid: None,
            radio: None,
            band: None,
            channel: None,
            channel_width_mhz: None,
            frequency_mhz: None,
            tx_power_dbm: None,
            signal_dbm,
            signal_avg_dbm: None,
            noise_dbm: None,
            snr_db: None,
            rx_rate_bps: None,
            tx_rate_bps: None,
            mcs: None,
            tx_retries_pct: None,
            inactive_ms: Some(inactive_ms),
            connected_s: None,
            signal_grade: WifiGrade::Good,
            snr_grade: None,
            retries_grade: None,
            health: WifiHealth::Healthy,
            health_reason: None,
            link_score: 1.0,
            distance: None,
        }
    }

    pub(crate) fn ap(device_id: i64, stations: Vec<WifiStation>) -> WifiDeviceSnapshot {
        WifiDeviceSnapshot {
            device_id,
            device_name: format!("AP {device_id}"),
            status: WifiReadStatus::Ok,
            read_at: None,
            error: None,
            stations,
            can_ping: false,
        }
    }

    const CELULAR: &str = "aa:bb:cc:dd:ee:01";

    #[test]
    fn localiza_pela_atividade_mais_recente_e_depois_pelo_sinal() {
        // A sobra da associação antiga (parada há 40 s) perde para o AP novo,
        // mesmo com sinal melhor.
        let rodada = vec![
            ap(1, vec![estacao(CELULAR, -50, 40_000)]),
            ap(2, vec![estacao(CELULAR, -70, 100)]),
        ];
        assert_eq!(locate(&rodada)[CELULAR].device_id, 2);
        // Ativos nos dois: o sinal melhor decide.
        let rodada = vec![
            ap(1, vec![estacao(CELULAR, -50, 200)]),
            ap(2, vec![estacao(CELULAR, -70, 100)]),
        ];
        assert_eq!(locate(&rodada)[CELULAR].device_id, 1);
        // Leitura que falhou não localiza ninguém.
        let mut falhou = ap(3, vec![estacao(CELULAR, -40, 0)]);
        falhou.status = WifiReadStatus::Failed;
        assert!(locate(&[falhou]).is_empty());
    }

    #[test]
    fn troca_de_ap_vira_roaming_com_o_sinal_dos_dois_lados() {
        let mut known = HashMap::new();
        let t0 = Utc::now();
        let primeira = locate(&[ap(1, vec![estacao(CELULAR, -78, 100)])]);
        assert!(detect(&mut known, &primeira, t0).is_empty(), "primeira vez");

        let mesma = locate(&[ap(1, vec![estacao(CELULAR, -80, 100)])]);
        assert!(detect(&mut known, &mesma, t0 + Duration::seconds(15)).is_empty());

        let outra = locate(&[ap(2, vec![estacao(CELULAR, -55, 100)])]);
        let trocas = detect(&mut known, &outra, t0 + Duration::seconds(30));
        assert_eq!(trocas.len(), 1);
        assert_eq!(trocas[0].mac, CELULAR);
        assert_eq!(trocas[0].from.device_id, 1);
        assert_eq!(
            trocas[0].from.signal_dbm, -80,
            "o último sinal no AP antigo"
        );
        assert_eq!(trocas[0].to.device_id, 2);
        assert_eq!(trocas[0].to.signal_dbm, -55);
        assert_eq!(trocas[0].to.label, "host-01");
    }

    #[test]
    fn rodada_sem_o_aparelho_nao_apaga_onde_ele_estava() {
        let mut known = HashMap::new();
        let t0 = Utc::now();
        detect(
            &mut known,
            &locate(&[ap(1, vec![estacao(CELULAR, -60, 100)])]),
            t0,
        );
        // Leitura de outro AP só (o "Ler agora" de um equipamento).
        detect(
            &mut known,
            &locate(&[ap(3, Vec::new())]),
            t0 + Duration::seconds(20),
        );
        let trocas = detect(
            &mut known,
            &locate(&[ap(2, vec![estacao(CELULAR, -60, 100)])]),
            t0 + Duration::seconds(40),
        );
        assert_eq!(trocas.len(), 1, "continuava no AP 1");
    }

    #[test]
    fn voltar_muito_depois_noutro_ap_e_reconexao() {
        let mut known = HashMap::new();
        let t0 = Utc::now();
        detect(
            &mut known,
            &locate(&[ap(1, vec![estacao(CELULAR, -60, 100)])]),
            t0,
        );
        let tarde = t0 + ROAM_GAP + Duration::seconds(1);
        detect(&mut known, &BTreeMap::new(), tarde);
        assert!(known.is_empty(), "esquece quem sumiu");
        let trocas = detect(
            &mut known,
            &locate(&[ap(2, vec![estacao(CELULAR, -60, 100)])]),
            tarde,
        );
        assert!(trocas.is_empty());
    }

    #[test]
    fn ping_pong_e_atingir_o_limite() {
        assert!(!is_ping_pong(3, 4));
        assert!(is_ping_pong(4, 4));
        assert!(is_ping_pong(9, 4));
    }
}
