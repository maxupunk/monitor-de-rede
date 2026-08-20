//! Serviço de status de onboarding e primeiro acesso (§7).
//!
//! Controla se o assistente de primeiro acesso foi concluído e fornece
//! os dados preliminares (contagem de sites, redes, servidores DNS, IP LAN, IP público)
//! para permitir o preenchimento inteligente do diálogo no primeiro acesso.

use chrono::Utc;
use sea_orm::{DatabaseConnection, EntityTrait, PaginatorTrait};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::{dns_servers, networks, sites, system_settings},
    services::{
        server_addresses,
        shared::errors::{AppError, AppResult},
        syslog::nat::NatDetector,
        vpn::{preflight, server_service},
    },
};

/// Chave em `system_settings`.
pub const STORAGE_KEY: &str = "onboarding_status";

/// Documento persistido em `system_settings`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredOnboarding {
    pub completed: bool,
    pub completed_at: Option<String>,
}

/// Resposta de status para o assistente de configuração.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OnboardingStatus {
    pub completed: bool,
    pub completed_at: Option<String>,
    pub needs_onboarding: bool,
    #[ts(type = "number")]
    pub sites_count: u64,
    #[ts(type = "number")]
    pub networks_count: u64,
    #[ts(type = "number")]
    pub dns_servers_count: u64,
    pub vpn_configured: bool,
    pub detected_lan_ip: Option<String>,
    pub detected_public_ip: Option<String>,
}

/// Consulta o status atual do onboarding.
pub async fn get_status(
    db: &DatabaseConnection,
    detector: &NatDetector,
) -> AppResult<OnboardingStatus> {
    let stored: StoredOnboarding = system_settings::Model::get(db, STORAGE_KEY)
        .await?
        .and_then(|linha| linha.value)
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default();

    let sites_count = sites::Entity::find().count(db).await?;
    let networks_count = networks::Entity::find().count(db).await?;
    let dns_servers_count = dns_servers::Entity::find().count(db).await?;
    let vpn_server = server_service::find(db).await?;
    let vpn_configured = vpn_server.is_some();

    // Endereços detectados
    let lista_enderecos = server_addresses::list(db, detector)
        .await
        .unwrap_or_default();
    let detected_lan_ip = lista_enderecos
        .iter()
        .find(|e| e.kind == server_addresses::AddressKind::Lan)
        .and_then(|e| e.value.clone().or_else(|| e.detected.clone()));

    let detected_public_ip = match lista_enderecos
        .iter()
        .find(|e| e.kind == server_addresses::AddressKind::Public)
        .and_then(|e| e.value.clone().or_else(|| e.detected.clone()))
    {
        Some(ip) => Some(ip),
        None => preflight::detect_public_ip().await.map(|ip| ip.to_string()),
    };

    let needs_onboarding = !stored.completed && (sites_count == 0 || networks_count == 0);

    Ok(OnboardingStatus {
        completed: stored.completed,
        completed_at: stored.completed_at,
        needs_onboarding,
        sites_count,
        networks_count,
        dns_servers_count,
        vpn_configured,
        detected_lan_ip,
        detected_public_ip,
    })
}

/// Marca o assistente de onboarding como concluído.
pub async fn mark_completed(db: &DatabaseConnection) -> AppResult<StoredOnboarding> {
    let doc = StoredOnboarding {
        completed: true,
        completed_at: Some(Utc::now().to_rfc3339()),
    };
    let texto = serde_json::to_string(&doc)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(db, STORAGE_KEY, Some(texto)).await?;
    Ok(doc)
}

/// Redefine o status do assistente (para testes ou reexecução manual).
pub async fn reset(db: &DatabaseConnection) -> AppResult<()> {
    system_settings::Model::set(db, STORAGE_KEY, None).await?;
    Ok(())
}
