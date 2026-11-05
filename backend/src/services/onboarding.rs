//! Serviço de status de onboarding e primeiro acesso (§7).
//!
//! Controla se o assistente de primeiro acesso foi concluído e fornece
//! os dados preliminares (contagem de sites, redes, servidores DNS, IP LAN, IP público)
//! para permitir o preenchimento inteligente do diálogo no primeiro acesso.

use chrono::Utc;
use sea_orm::{DatabaseConnection, EntityTrait, PaginatorTrait};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use netmonitor_core::areas::{self, Area, AreaPolicy, AreaService, AreasSnapshot};

use crate::{
    dtos::areas::OnboardingIntentInput,
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
    /// A primeira etapa ("O que você vai gerenciar?") já foi respondida.
    #[serde(default)]
    pub intent_chosen: bool,
}

/// Resposta de status para o assistente de configuração.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OnboardingStatus {
    pub completed: bool,
    pub completed_at: Option<String>,
    pub needs_onboarding: bool,
    /// A escolha das áreas (primeira etapa) já foi feita.
    pub intent_chosen: bool,
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
    let stored = load(db).await?;

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

    // Só cobra site e rede de quem vai gerenciar rede (ADR 017): quem escolheu
    // só Servidores conclui sem cadastrar faixa nenhuma.
    let network_on = areas::is_enabled(db, Area::Network).await?;
    let needs_onboarding = !stored.completed
        && (!stored.intent_chosen || (network_on && (sites_count == 0 || networks_count == 0)));

    Ok(OnboardingStatus {
        completed: stored.completed,
        completed_at: stored.completed_at,
        needs_onboarding,
        intent_chosen: stored.intent_chosen,
        sites_count,
        networks_count,
        dns_servers_count,
        vpn_configured,
        detected_lan_ip,
        detected_public_ip,
    })
}

async fn load(db: &DatabaseConnection) -> AppResult<StoredOnboarding> {
    Ok(system_settings::Model::get(db, STORAGE_KEY)
        .await?
        .and_then(|linha| linha.value)
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default())
}

async fn store(db: &DatabaseConnection, doc: &StoredOnboarding) -> AppResult<()> {
    let texto = serde_json::to_string(doc)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?;
    system_settings::Model::set(db, STORAGE_KEY, Some(texto)).await?;
    Ok(())
}

/// Marca o assistente de onboarding como concluído.
pub async fn mark_completed(db: &DatabaseConnection) -> AppResult<StoredOnboarding> {
    let doc = StoredOnboarding {
        completed: true,
        completed_at: Some(Utc::now().to_rfc3339()),
        ..load(db).await?
    };
    store(db, &doc).await?;
    Ok(doc)
}

/// Uma área que a escolha do assistente mudou — para a auditoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaChange {
    pub area: Area,
    pub before: bool,
    pub enabled: bool,
}

/// O resultado de aplicar a escolha: o estado novo e o que mudou.
#[derive(Debug, Clone)]
pub struct IntentOutcome {
    pub snapshot: AreasSnapshot,
    pub changes: Vec<AreaChange>,
}

/// Aplica a primeira etapa do assistente ("O que você vai gerenciar?"):
/// liga e desliga as áreas escolhidas e registra que a escolha foi feita.
///
/// # Errors
///
/// Nem Rede nem Servidores escolhidos (422); desligamento bloqueado pela área
/// (409, a VPN com servidor ativo); erro do banco.
pub async fn apply_intent(
    db: &DatabaseConnection,
    policies: &[Box<dyn AreaPolicy>],
    intent: OnboardingIntentInput,
) -> AppResult<IntentOutcome> {
    if !intent.network && !intent.servers {
        return Err(AppError::validation(
            "Escolha ao menos Rede ou Servidores: é o que o NetMonitor vai acompanhar.",
        ));
    }
    let service = AreaService::new(db, policies);
    let mut changes = Vec::new();
    let mut snapshot = service.snapshot().await?;
    for (area, enabled) in [
        (Area::Network, intent.network),
        (Area::Servers, intent.servers),
        (Area::Vpn, intent.vpn),
    ] {
        let before = snapshot.is_enabled(area);
        if before != enabled {
            snapshot = service.set(area, enabled).await?;
            changes.push(AreaChange {
                area,
                before,
                enabled,
            });
        }
    }
    store(
        db,
        &StoredOnboarding {
            intent_chosen: true,
            ..load(db).await?
        },
    )
    .await?;
    Ok(IntentOutcome { snapshot, changes })
}

/// Redefine o status do assistente (para testes ou reexecução manual).
pub async fn reset(db: &DatabaseConnection) -> AppResult<()> {
    system_settings::Model::set(db, STORAGE_KEY, None).await?;
    Ok(())
}
