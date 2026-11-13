//! As Configurações de Wi-Fi: de quanto em quanto tempo o mapa e a rotina
//! leem os APs, o ambiente da estimativa de distância (padrão e por AP) e o
//! histórico de roaming (retenção e ping-pong).
//!
//! Guardadas como um JSON só em `system_settings` (`wifi.settings`); valor
//! ausente ou antigo cai no padrão campo a campo.

use netmonitor_core::{
    models::system_settings,
    shared::{
        errors::{AppError, AppResult},
        transaction::begin_write,
    },
};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::Deserialize;

use crate::dtos::{WifiApEnvironment, WifiEnvironment, WifiSettings};

const STORAGE_KEY: &str = "wifi.settings";

/// Os limites de cada ajuste: o que a tela oferece e o servidor aceita.
pub const LIVE_INTERVAL_S: (u32, u32) = (5, 300);
pub const BACKGROUND_INTERVAL_S: (u32, u32) = (60, 900);
pub const RETENTION_DAYS: (u32, u32) = (1, 365);
pub const PING_PONG_COUNT: (u32, u32) = (2, 30);
pub const PING_PONG_WINDOW_MIN: (u32, u32) = (1, 120);

impl Default for WifiSettings {
    fn default() -> Self {
        Self {
            live_interval_s: 15,
            background_interval_s: 120,
            environment: WifiEnvironment::Office,
            ap_environments: Vec::new(),
            roaming_retention_days: 30,
            ping_pong_count: 4,
            ping_pong_window_min: 10,
        }
    }
}

impl WifiSettings {
    /// O ambiente de um AP: o dele, se houver, ou o padrão.
    #[must_use]
    pub fn environment_for(&self, device_id: i64) -> WifiEnvironment {
        self.ap_environments
            .iter()
            .find(|item| item.device_id == device_id)
            .map_or(self.environment, |item| item.environment)
    }
}

/// O JSON gravado, tolerante: campo ausente vem do padrão.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Stored {
    live_interval_s: u32,
    background_interval_s: u32,
    environment: WifiEnvironment,
    ap_environments: Vec<WifiApEnvironment>,
    roaming_retention_days: u32,
    ping_pong_count: u32,
    ping_pong_window_min: u32,
}

impl Default for Stored {
    fn default() -> Self {
        let WifiSettings {
            live_interval_s,
            background_interval_s,
            environment,
            ap_environments,
            roaming_retention_days,
            ping_pong_count,
            ping_pong_window_min,
        } = WifiSettings::default();
        Self {
            live_interval_s,
            background_interval_s,
            environment,
            ap_environments,
            roaming_retention_days,
            ping_pong_count,
            ping_pong_window_min,
        }
    }
}

impl From<Stored> for WifiSettings {
    fn from(stored: Stored) -> Self {
        Self {
            live_interval_s: stored.live_interval_s,
            background_interval_s: stored.background_interval_s,
            environment: stored.environment,
            ap_environments: stored.ap_environments,
            roaming_retention_days: stored.roaming_retention_days,
            ping_pong_count: stored.ping_pong_count,
            ping_pong_window_min: stored.ping_pong_window_min,
        }
    }
}

/// As configurações em vigor; gravação corrompida vale o padrão.
///
/// # Errors
///
/// Erro do banco.
pub async fn load<C: ConnectionTrait>(db: &C) -> AppResult<WifiSettings> {
    let stored = system_settings::Model::get(db, STORAGE_KEY)
        .await?
        .and_then(|row| row.value)
        .and_then(|value| serde_json::from_str::<Stored>(&value).ok());
    Ok(stored.map_or_else(WifiSettings::default, |stored| {
        clamped(WifiSettings::from(stored))
    }))
}

/// Grava as configurações depois de conferir cada limite.
///
/// # Errors
///
/// `422` com o primeiro ajuste fora do limite; erro do banco.
pub async fn save(db: &DatabaseConnection, input: WifiSettings) -> AppResult<WifiSettings> {
    let settings = validate(input)?;
    let value = serde_json::to_string(&settings)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    let txn = begin_write(db).await?;
    system_settings::Model::set(&txn, STORAGE_KEY, Some(value)).await?;
    txn.commit().await?;
    Ok(settings)
}

fn check(value: u32, (min, max): (u32, u32), what: &str) -> AppResult<()> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "{what} precisa ficar entre {min} e {max}."
        )))
    }
}

/// Os limites; ambiente por AP repetido fica com o último.
///
/// # Errors
///
/// `422` com o primeiro ajuste fora do limite.
pub fn validate(mut settings: WifiSettings) -> AppResult<WifiSettings> {
    check(
        settings.live_interval_s,
        LIVE_INTERVAL_S,
        "A atualização do mapa (segundos)",
    )?;
    check(
        settings.background_interval_s,
        BACKGROUND_INTERVAL_S,
        "A leitura em segundo plano (segundos)",
    )?;
    check(
        settings.roaming_retention_days,
        RETENTION_DAYS,
        "A retenção do histórico de roaming (dias)",
    )?;
    check(
        settings.ping_pong_count,
        PING_PONG_COUNT,
        "O número de trocas do ping-pong",
    )?;
    check(
        settings.ping_pong_window_min,
        PING_PONG_WINDOW_MIN,
        "A janela do ping-pong (minutos)",
    )?;
    let mut unique: Vec<WifiApEnvironment> = Vec::new();
    for item in settings.ap_environments.into_iter().rev() {
        if !unique.iter().any(|kept| kept.device_id == item.device_id) {
            unique.push(item);
        }
    }
    unique.reverse();
    // Igual ao padrão não é ajuste: não precisa ficar guardado.
    unique.retain(|item| item.environment != settings.environment);
    settings.ap_environments = unique;
    Ok(settings)
}

/// O gravado à mão fora do limite volta para dentro dele.
fn clamped(mut settings: WifiSettings) -> WifiSettings {
    let clamp = |value: u32, (min, max): (u32, u32)| value.clamp(min, max);
    settings.live_interval_s = clamp(settings.live_interval_s, LIVE_INTERVAL_S);
    settings.background_interval_s = clamp(settings.background_interval_s, BACKGROUND_INTERVAL_S);
    settings.roaming_retention_days = clamp(settings.roaming_retention_days, RETENTION_DAYS);
    settings.ping_pong_count = clamp(settings.ping_pong_count, PING_PONG_COUNT);
    settings.ping_pong_window_min = clamp(settings.ping_pong_window_min, PING_PONG_WINDOW_MIN);
    settings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_padrao_e_o_combinado() {
        let padrao = WifiSettings::default();
        assert_eq!(padrao.live_interval_s, 15);
        assert_eq!(padrao.background_interval_s, 120);
        assert_eq!(padrao.roaming_retention_days, 30);
        assert_eq!(
            (padrao.ping_pong_count, padrao.ping_pong_window_min),
            (4, 10)
        );
        assert_eq!(padrao.environment, WifiEnvironment::Office);
    }

    #[test]
    fn ambiente_do_ap_vale_mais_que_o_padrao() {
        let ajustes = WifiSettings {
            ap_environments: vec![WifiApEnvironment {
                device_id: 7,
                environment: WifiEnvironment::Walls,
            }],
            ..WifiSettings::default()
        };
        assert_eq!(ajustes.environment_for(7), WifiEnvironment::Walls);
        assert_eq!(ajustes.environment_for(8), WifiEnvironment::Office);
    }

    #[test]
    fn valida_os_limites_e_limpa_o_ambiente_por_ap() {
        assert!(validate(WifiSettings {
            live_interval_s: 2,
            ..WifiSettings::default()
        })
        .is_err());
        assert!(validate(WifiSettings {
            ping_pong_count: 1,
            ..WifiSettings::default()
        })
        .is_err());
        let limpo = validate(WifiSettings {
            ap_environments: vec![
                WifiApEnvironment {
                    device_id: 1,
                    environment: WifiEnvironment::Open,
                },
                WifiApEnvironment {
                    device_id: 1,
                    environment: WifiEnvironment::Walls,
                },
                WifiApEnvironment {
                    device_id: 2,
                    environment: WifiEnvironment::Office,
                },
            ],
            ..WifiSettings::default()
        })
        .unwrap();
        assert_eq!(
            limpo.ap_environments,
            vec![WifiApEnvironment {
                device_id: 1,
                environment: WifiEnvironment::Walls,
            }],
            "o último vale; igual ao padrão sai"
        );
    }

    #[test]
    fn gravacao_antiga_ou_fora_do_limite_cai_no_padrao() {
        let antiga: Stored = serde_json::from_str(r#"{"liveIntervalS": 1}"#).unwrap();
        let ajustes = clamped(WifiSettings::from(antiga));
        assert_eq!(ajustes.live_interval_s, 5, "volta para dentro do limite");
        assert_eq!(ajustes.background_interval_s, 120, "ausente vem do padrão");
    }
}
