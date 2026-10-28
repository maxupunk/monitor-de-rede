//! Regras aplicadas **uma vez na vida** da instalação.
//!
//! `ensure_defaults` pergunta "esta instalação é nova?"; aqui a pergunta é
//! "já apliquei este conjunto alguma vez?" — e só um marcador em
//! `system_settings` a responde: se o operador apagar de propósito uma dessas
//! regras, ela não pode ressuscitar (ver `health_defaults`).

use sea_orm::ConnectionTrait;

use crate::{
    models::system_settings::Model as SystemSetting,
    services::{
        alerts::catalog::{
            service::{apply_scoped, CatalogApplicationResult, TemplateScope},
            templates,
        },
        shared::errors::AppResult,
    },
};

const WIFI_WATCH_MARKER: &str = "alerts.wifi_watch_defaults_applied";

/// Aplica os templates no escopo se o marcador ainda não existe.
///
/// Idempotente por duas vias: o marcador impede que uma regra removida pelo
/// usuário reapareça, e a idempotência com escopo do catálogo impede duplicata
/// caso o marcador se perca numa restauração de backup.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn apply_once<C: ConnectionTrait>(
    db: &C,
    marker: &str,
    keys: &[&str],
    scope: TemplateScope,
) -> AppResult<CatalogApplicationResult> {
    if SystemSetting::get(db, marker).await?.is_some() {
        return Ok(CatalogApplicationResult::default());
    }
    let keys: Vec<String> = keys.iter().map(|key| (*key).to_string()).collect();
    let result = apply_scoped(db, &keys, scope).await?;
    // O marcador é gravado depois da aplicação: se a criação falhar, a próxima
    // chamada tenta de novo em vez de desistir para sempre.
    SystemSetting::set(db, marker, Some("1".to_string())).await?;
    Ok(result)
}

/// As regras dos clientes Wi-Fi vigiados, quando o primeiro aparelho é vigiado.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn ensure_wifi_watch<C: ConnectionTrait>(db: &C) -> AppResult<CatalogApplicationResult> {
    apply_once(
        db,
        WIFI_WATCH_MARKER,
        &templates::WIFI_WATCH_KEYS,
        TemplateScope::global(),
    )
    .await
}
