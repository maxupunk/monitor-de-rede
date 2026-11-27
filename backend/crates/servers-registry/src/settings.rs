//! O que o administrador ajusta no registry: quantas versões guardar, a cota
//! (RF-15/17) e a partir de qual severidade uma vulnerabilidade vira alerta.
//! Mora em `system_settings` (`registry`), em JSON — campo novo entra com o
//! padrão (`serde(default)`), sem migration.

use sea_orm::{ConnectionTrait, EntityTrait, QuerySelect};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use netmonitor_core::{
    models::{registry_blobs, system_settings},
    shared::errors::{AppError, AppResult},
};

pub const STORAGE_KEY: &str = "registry";
const GIB: u64 = 1024 * 1024 * 1024;

/// A partir de qual severidade uma vulnerabilidade da imagem vira alerta em
/// `/alerts` (`scan::alerts`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "RegistryVulnerabilityAlerts"
)]
pub enum VulnerabilityAlerts {
    /// Nenhum alerta: a varredura continua, só não avisa.
    Off,
    /// Só as críticas.
    Critical,
    /// Críticas e altas (padrão).
    #[default]
    High,
}

impl VulnerabilityAlerts {
    /// Se a severidade (`critical`, `high`) vira alerta com este ajuste.
    #[must_use]
    pub const fn reports(self, critical: bool) -> bool {
        match self {
            Self::Off => false,
            Self::Critical => critical,
            Self::High => true,
        }
    }

    /// Como a auditoria e a tela descrevem o ajuste.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "desligados",
            Self::Critical => "só críticas",
            Self::High => "críticas e altas",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "RegistrySettings"
)]
pub struct Settings {
    /// Versões guardadas por repositório, além das que algum alvo usa.
    pub keep_versions: u32,
    /// Cota total, em GiB.
    pub quota_gib: u32,
    /// A partir de qual severidade a vulnerabilidade vira alerta.
    pub vulnerability_alerts: VulnerabilityAlerts,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keep_versions: 10,
            quota_gib: 50,
            vulnerability_alerts: VulnerabilityAlerts::default(),
        }
    }
}

impl Settings {
    #[must_use]
    pub const fn quota_bytes(&self) -> u64 {
        self.quota_gib as u64 * GIB
    }

    /// # Errors
    ///
    /// `Validation` fora das faixas.
    pub fn validate(&self) -> AppResult<()> {
        if !(1..=200).contains(&self.keep_versions) {
            return Err(AppError::validation(
                "Guarde de 1 a 200 versões por repositório",
            ));
        }
        if !(1..=10_000).contains(&self.quota_gib) {
            return Err(AppError::validation("A cota vai de 1 GiB a 10 TiB"));
        }
        Ok(())
    }
}

/// # Errors
///
/// Erro do banco.
pub async fn load<C: ConnectionTrait>(db: &C) -> AppResult<Settings> {
    Ok(system_settings::Model::get(db, STORAGE_KEY)
        .await?
        .and_then(|row| row.value)
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default())
}

/// # Errors
///
/// `Validation`; erro do banco.
pub async fn save<C: ConnectionTrait>(db: &C, settings: &Settings) -> AppResult<()> {
    settings.validate()?;
    let text =
        serde_json::to_string(settings).map_err(|error| AppError::validation(error.to_string()))?;
    system_settings::Model::set(db, STORAGE_KEY, Some(text)).await?;
    Ok(())
}

/// Bytes ocupados (cada blob uma vez).
///
/// # Errors
///
/// Erro do banco.
pub async fn usage<C: ConnectionTrait>(db: &C) -> AppResult<u64> {
    // `SUM(bigint)` vira `numeric` no PostgreSQL: o `CAST` mantém o mesmo
    // tipo nos dois bancos.
    let total: Option<i64> = registry_blobs::Entity::find()
        .select_only()
        .column_as(
            sea_orm::sea_query::Expr::cust("CAST(COALESCE(SUM(size), 0) AS BIGINT)"),
            "total",
        )
        .into_tuple()
        .one(db)
        .await?;
    Ok(total
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(0))
}

/// Quanto ainda cabe.
///
/// # Errors
///
/// Erro do banco.
pub async fn room<C: ConnectionTrait>(db: &C) -> AppResult<u64> {
    let settings = load(db).await?;
    Ok(settings.quota_bytes().saturating_sub(usage(db).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ajuste_gravado_antes_do_campo_novo_alerta_criticas_e_altas() {
        let old: Settings = serde_json::from_str(r#"{"keepVersions":5,"quotaGib":20}"#).unwrap();
        assert_eq!(old.keep_versions, 5);
        assert_eq!(old.vulnerability_alerts, VulnerabilityAlerts::High);
    }

    #[test]
    fn cada_ajuste_diz_quais_severidades_viram_alerta() {
        assert!(!VulnerabilityAlerts::Off.reports(true));
        assert!(VulnerabilityAlerts::Critical.reports(true));
        assert!(!VulnerabilityAlerts::Critical.reports(false));
        assert!(VulnerabilityAlerts::High.reports(false));
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(json["vulnerabilityAlerts"], "high");
    }
}
