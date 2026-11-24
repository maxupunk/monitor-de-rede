//! Configurações globais do servidor MCP em `system_settings`.
//!
//! * `mcp.enabled` — padrão `true`;
//! * `mcp.allow_public_origin` — padrão `false` (apenas rede interna ou VPN).

use sea_orm::{ConnectionTrait, DatabaseConnection};

use netmonitor_core::{
    models::system_settings,
    shared::{errors::AppResult, transaction::begin_write},
};

pub const KEY_ENABLED: &str = "mcp.enabled";
pub const KEY_ALLOW_PUBLIC_ORIGIN: &str = "mcp.allow_public_origin";
pub const ENDPOINT_PATH: &str = "/mcp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpSettings {
    pub enabled: bool,
    pub allow_public_origin: bool,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_public_origin: false,
        }
    }
}

fn parse_bool(raw: Option<&str>, default: bool) -> bool {
    match raw.map(str::trim) {
        Some(v) if v.eq_ignore_ascii_case("true") || v == "1" => true,
        Some(v) if v.eq_ignore_ascii_case("false") || v == "0" => false,
        _ => default,
    }
}

/// Lê as configurações do servidor MCP; ausentes ou inválidas caem no padrão seguro.
///
/// # Errors
///
/// Propaga falha de leitura no banco.
pub async fn load<C: ConnectionTrait>(db: &C) -> AppResult<McpSettings> {
    let defaults = McpSettings::default();
    let enabled_row = system_settings::Model::get(db, KEY_ENABLED).await?;
    let public_row = system_settings::Model::get(db, KEY_ALLOW_PUBLIC_ORIGIN).await?;
    Ok(McpSettings {
        enabled: parse_bool(
            enabled_row.as_ref().and_then(|row| row.value.as_deref()),
            defaults.enabled,
        ),
        allow_public_origin: parse_bool(
            public_row.as_ref().and_then(|row| row.value.as_deref()),
            defaults.allow_public_origin,
        ),
    })
}

/// Persiste as configurações do servidor MCP atomicamente via `begin_write`.
///
/// # Errors
///
/// Propaga falha de transação ou escrita no banco.
pub async fn save(db: &DatabaseConnection, settings: &McpSettings) -> AppResult<McpSettings> {
    let tx = begin_write(db).await?;
    system_settings::Model::set(&tx, KEY_ENABLED, Some(settings.enabled.to_string())).await?;
    system_settings::Model::set(
        &tx,
        KEY_ALLOW_PUBLIC_ORIGIN,
        Some(settings.allow_public_origin.to_string()),
    )
    .await?;
    tx.commit().await?;
    Ok(*settings)
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectOptions, Database};

    use super::*;
    use crate::migrations::{MigratorTrait, TestMigrator};

    async fn memory_db() -> DatabaseConnection {
        let db = Database::connect(
            ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco em memória");
        TestMigrator::up(&db, None).await.expect("migrations");
        db
    }

    #[tokio::test]
    async fn padroes_seguros_e_persistencia() {
        let db = memory_db().await;
        let initial = load(&db).await.expect("load inicial");
        assert!(initial.enabled);
        assert!(!initial.allow_public_origin);

        let updated = save(
            &db,
            &McpSettings {
                enabled: false,
                allow_public_origin: true,
            },
        )
        .await
        .expect("save");
        assert!(!updated.enabled);
        assert!(updated.allow_public_origin);

        let reloaded = load(&db).await.expect("reload");
        assert_eq!(reloaded, updated);
    }
}
