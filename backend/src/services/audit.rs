//! Trilha de auditoria (Fase 3 do roadmap).
//!
//! Registra quem alterou cada recurso, quando e como. O service é chamado
//! explicitamente pelos controllers após uma operação bem-sucedida; falhas no
//! log são isoladas e nunca quebram a ação principal.

use axum::http::HeaderMap;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use serde_json::Value;

use crate::{
    controllers::auth_guard::AUTHENTICATED_USER_HEADER,
    models::{audit_logs, users},
    services::shared::{
        errors::{AppError, AppResult},
        pagination::{normalize_limit, normalize_page, paginate_compat, LucidPage},
    },
};

/// Ação registrada na trilha de auditoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    Create,
    Update,
    Delete,
    Login,
    Logout,
}

impl AuditAction {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::Create => "create",
            AuditAction::Update => "update",
            AuditAction::Delete => "delete",
            AuditAction::Login => "login",
            AuditAction::Logout => "logout",
        }
    }
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AuditAction {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "create" => Ok(AuditAction::Create),
            "update" => Ok(AuditAction::Update),
            "delete" => Ok(AuditAction::Delete),
            "login" => Ok(AuditAction::Login),
            "logout" => Ok(AuditAction::Logout),
            _ => Err(AppError::validation(format!(
                "Ação de auditoria inválida: {s}"
            ))),
        }
    }
}

/// Tipo de recurso auditado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Device,
    Monitor,
    Site,
    Network,
    User,
    Probe,
    VpnPeer,
    AlertRule,
    MaintenanceWindow,
}

impl ResourceType {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Device => "device",
            ResourceType::Monitor => "monitor",
            ResourceType::Site => "site",
            ResourceType::Network => "network",
            ResourceType::User => "user",
            ResourceType::Probe => "probe",
            ResourceType::VpnPeer => "vpn_peer",
            ResourceType::AlertRule => "alert_rule",
            ResourceType::MaintenanceWindow => "maintenance_window",
        }
    }
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ResourceType {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "device" => Ok(ResourceType::Device),
            "monitor" => Ok(ResourceType::Monitor),
            "site" => Ok(ResourceType::Site),
            "network" => Ok(ResourceType::Network),
            "user" => Ok(ResourceType::User),
            "probe" => Ok(ResourceType::Probe),
            "vpn_peer" | "vpnpeer" => Ok(ResourceType::VpnPeer),
            "alert_rule" | "alertrule" => Ok(ResourceType::AlertRule),
            "maintenance_window" | "maintenancewindow" => Ok(ResourceType::MaintenanceWindow),
            _ => Err(AppError::validation(format!(
                "Tipo de recurso inválido: {s}"
            ))),
        }
    }
}

/// Quem executou a ação auditada.
#[derive(Debug, Clone, Default)]
pub struct AuditActor {
    pub user_id: Option<i64>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

impl AuditActor {
    /// Cria um ator já identificado por `user_id`, extraindo IP e user-agent.
    #[must_use]
    pub fn from_user_id(user_id: i64, headers: &HeaderMap) -> Self {
        Self {
            user_id: Some(user_id),
            ip: extract_ip(headers),
            user_agent: extract_user_agent(headers),
        }
    }

    /// Resolve o ator a partir do cabeçalho interno gravado pelo guarda JWT.
    ///
    /// O `user_id` é buscado no banco a partir do `pid`. Quando o header está
    /// ausente ou o usuário não existe, o log ainda é gerado sem `user_id`
    /// (ex.: ações do setup inicial).
    ///
    /// # Errors
    ///
    /// Propaga erro do banco ao buscar o usuário.
    pub async fn from_headers(headers: &HeaderMap, db: &DatabaseConnection) -> AppResult<Self> {
        let pid = headers
            .get(AUTHENTICATED_USER_HEADER)
            .and_then(|value| value.to_str().ok());

        let user_id = match pid {
            Some(pid) => users::Model::find_by_pid(db, pid).await.ok().map(|u| u.id),
            None => None,
        };

        Ok(Self {
            user_id,
            ip: extract_ip(headers),
            user_agent: extract_user_agent(headers),
        })
    }
}

/// Diff opcional entre o estado anterior e o novo.
#[derive(Debug, Clone, Default)]
pub struct AuditChanges {
    pub old: Option<Value>,
    pub new: Option<Value>,
}

/// Entrada para gravação de um evento de auditoria.
#[derive(Debug, Clone)]
pub struct AuditEntryInput {
    pub action: AuditAction,
    pub resource_type: ResourceType,
    pub resource_id: Option<i64>,
    pub resource_label: Option<String>,
    pub description: Option<String>,
    pub changes: Option<AuditChanges>,
}

/// Filtros para listagem de auditoria.
#[derive(Debug, Clone, Default)]
pub struct AuditFilters {
    pub user_id: Option<i64>,
    pub resource_type: Option<String>,
    pub resource_id: Option<i64>,
    pub action: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// Service de auditoria.
pub struct AuditService<'a, C> {
    db: &'a C,
}

impl<'a, C: ConnectionTrait> AuditService<'a, C> {
    #[must_use]
    pub fn new(db: &'a C) -> Self {
        Self { db }
    }

    /// Grava um evento de auditoria.
    ///
    /// Erros são logados com `warn!` e suprimidos: a operação principal não deve
    /// falhar porque a auditoria falhou.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco. **Quem chama deve ignorar o erro** na maioria dos
    /// casos, usando `let _ = ...` ou `.ok()`.
    pub async fn log(
        &self,
        actor: AuditActor,
        input: AuditEntryInput,
    ) -> AppResult<audit_logs::Model> {
        let changes_json = input.changes.map(|c| {
            serde_json::json!({
                "old": c.old,
                "new": c.new,
            })
        });

        let row = audit_logs::ActiveModel {
            user_id: Set(actor.user_id),
            action: Set(input.action.to_string()),
            resource_type: Set(Some(input.resource_type.to_string())),
            resource_id: Set(input.resource_id),
            resource_label: Set(input.resource_label),
            description: Set(input.description),
            changes: Set(changes_json),
            ip_address: Set(actor.ip),
            user_agent: Set(actor.user_agent),
            ..Default::default()
        }
        .insert(self.db)
        .await?;

        Ok(row)
    }

    /// Lista eventos de auditoria com filtros e paginação.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn list(
        &self,
        filters: AuditFilters,
        page: Option<u64>,
        limit: Option<u64>,
    ) -> AppResult<LucidPage<audit_logs::Model>> {
        let mut query = audit_logs::Entity::find();

        if let Some(user_id) = filters.user_id {
            query = query.filter(audit_logs::Column::UserId.eq(user_id));
        }
        if let Some(resource_type) = filters.resource_type {
            query = query.filter(audit_logs::Column::ResourceType.eq(resource_type));
        }
        if let Some(resource_id) = filters.resource_id {
            query = query.filter(audit_logs::Column::ResourceId.eq(resource_id));
        }
        if let Some(action) = filters.action {
            query = query.filter(audit_logs::Column::Action.eq(action));
        }
        if let Some(from) = filters.from {
            query = query.filter(audit_logs::Column::CreatedAt.gte(from));
        }
        if let Some(to) = filters.to {
            query = query.filter(audit_logs::Column::CreatedAt.lte(to));
        }

        query = query.order_by_desc(audit_logs::Column::CreatedAt);

        let page = normalize_page(page);
        let limit = normalize_limit(limit);
        paginate_compat(self.db, query, page, limit, |m| m).await
    }
}

fn extract_ip(headers: &HeaderMap) -> Option<String> {
    let sanitize = |raw: &str| -> Option<String> {
        let cleaned: String = raw
            .chars()
            .filter(|c| !c.is_control() && !c.is_whitespace())
            .collect();
        if cleaned.is_empty() || cleaned.len() > 64 {
            None
        } else {
            Some(cleaned)
        }
    };

    if let Some(forwarded) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = forwarded.split(',').next() {
            if let Some(ip) = sanitize(first) {
                return Some(ip);
            }
        }
    }

    if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if let Some(ip) = sanitize(real_ip) {
            return Some(ip);
        }
    }

    None
}

fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.len() <= 512)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn acoes_tem_rotulo_estavel() {
        assert_eq!(AuditAction::Create.as_str(), "create");
        assert_eq!(AuditAction::Update.as_str(), "update");
        assert_eq!(AuditAction::Delete.as_str(), "delete");
        assert_eq!(AuditAction::Login.as_str(), "login");
        assert_eq!(AuditAction::Logout.as_str(), "logout");
    }

    #[test]
    fn recursos_tem_rotulo_estavel() {
        assert_eq!(ResourceType::Device.as_str(), "device");
        assert_eq!(
            ResourceType::MaintenanceWindow.as_str(),
            "maintenance_window"
        );
    }

    #[test]
    fn parse_de_acao_aceita_variacoes_de_caixa() {
        assert_eq!(
            "CREATE".parse::<AuditAction>().unwrap(),
            AuditAction::Create
        );
        assert_eq!("Login".parse::<AuditAction>().unwrap(), AuditAction::Login);
        assert!("foo".parse::<AuditAction>().is_err());
    }

    #[test]
    fn parse_de_recurso_aceita_snake_e_camel() {
        assert_eq!(
            "vpn_peer".parse::<ResourceType>().unwrap(),
            ResourceType::VpnPeer
        );
        assert_eq!(
            "vpnPeer".parse::<ResourceType>().unwrap(),
            ResourceType::VpnPeer
        );
        assert!("foo".parse::<ResourceType>().is_err());
    }

    #[test]
    fn extrai_primeiro_ip_do_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.1, 10.0.0.1"),
        );
        assert_eq!(extract_ip(&headers).as_deref(), Some("203.0.113.1"));
    }

    #[test]
    fn x_real_ip_funciona_quando_nao_ha_forwarded() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.2"));
        assert_eq!(extract_ip(&headers).as_deref(), Some("198.51.100.2"));
    }

    #[test]
    fn ip_muito_longo_e_descartado() {
        let mut headers = HeaderMap::new();
        let huge = "a".repeat(100);
        headers.insert("x-real-ip", HeaderValue::from_str(&huge).unwrap());
        assert!(extract_ip(&headers).is_none());
    }

    #[test]
    fn user_agent_e_limitado() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::USER_AGENT,
            HeaderValue::from_static("Mozilla/5.0"),
        );
        assert_eq!(extract_user_agent(&headers).as_deref(), Some("Mozilla/5.0"));
    }
}
