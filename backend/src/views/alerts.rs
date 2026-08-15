//! Serialização das regras e dos eventos de alerta (§7.11).
//!
//! O frontend (`stores/alerts.ts`) lê `AlertRule` e `AlertEvent` com estes
//! nomes exatos. Duas derivações não vêm do banco e precisam ser calculadas
//! aqui: `isEnabled`, espelho de `enabled` que a tela consome sob esse nome, e
//! `title`, que a Central de Alertas exibe como cabeçalho de cada linha.

use std::collections::HashMap;

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use serde_json::Value;

use crate::{
    models::{
        _entities::{alert_events as alert_events_entity, devices as devices_entity},
        alert_events, alert_rules, devices, monitors,
    },
    services::shared::errors::AppResult,
};

/// Regra como o frontend a consome.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuleResponse {
    pub id: i64,
    pub site_id: Option<i64>,
    pub device_id: Option<i64>,
    pub monitor_id: Option<i64>,
    pub name: String,
    #[serde(rename = "type")]
    pub rule_type: String,
    pub template_key: Option<String>,
    pub condition: Value,
    pub severity: String,
    pub duration_seconds: i32,
    /// Janela de estabilidade antes de resolver, em segundos (0 = resolve na
    /// primeira checagem ok). A tela de regras edita este campo.
    pub recovery_window_seconds: i32,
    /// Recaídas dentro da janela de flap que declaram o alvo oscilando
    /// (0 = detecção desligada). Fase 3.
    pub flap_threshold: i32,
    /// Largura da janela deslizante da detecção de flapping, em segundos.
    pub flap_window_seconds: i32,
    /// Intervalo mínimo entre notificações de problema do par (regra, alvo),
    /// mesmo quando o evento fecha e reabre (0 = sem cooldown). Fase 4.
    pub notification_cooldown_seconds: i32,
    /// Suprimir a notificação quando o pai declarado do dispositivo já está em
    /// alerta. Fase 4.
    pub inhibit_when_parent_down: bool,
    pub enabled: bool,
    /// Espelho de `enabled` (§6.1) — o `AlertRule` do frontend lê os dois.
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<alert_rules::Model> for AlertRuleResponse {
    fn from(row: alert_rules::Model) -> Self {
        Self {
            id: row.id,
            site_id: row.site_id,
            device_id: row.device_id,
            monitor_id: row.monitor_id,
            name: row.name,
            rule_type: row.r#type,
            template_key: row.template_key,
            condition: row.condition,
            severity: row.severity,
            duration_seconds: row.duration_seconds,
            recovery_window_seconds: row.recovery_window_seconds,
            flap_threshold: row.flap_threshold,
            flap_window_seconds: row.flap_window_seconds,
            notification_cooldown_seconds: row.notification_cooldown_seconds,
            inhibit_when_parent_down: row.inhibit_when_parent_down,
            enabled: row.enabled,
            is_enabled: row.enabled,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

/// Payload enxuto dos eventos SSE de regra (`alert_rule:created|updated|deleted`).
#[must_use]
pub fn rule_event_payload(rule: &alert_rules::Model) -> Value {
    serde_json::json!({
        "id": rule.id,
        "name": rule.name,
        "type": rule.r#type,
        "templateKey": rule.template_key,
        "condition": rule.condition,
        "severity": rule.severity,
        "durationSeconds": rule.duration_seconds,
        "recoveryWindowSeconds": rule.recovery_window_seconds,
        "flapThreshold": rule.flap_threshold,
        "flapWindowSeconds": rule.flap_window_seconds,
        "notificationCooldownSeconds": rule.notification_cooldown_seconds,
        "inhibitWhenParentDown": rule.inhibit_when_parent_down,
        "enabled": rule.enabled,
        "isEnabled": rule.enabled,
    })
}

/// Referência enxuta a uma entidade relacionada no payload de alertas.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedSummary {
    pub id: i64,
    pub name: String,
}

/// Contrato de resposta de `GET /api/alerts` (§7.11).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedAlertEvent {
    pub id: i64,
    pub alert_rule_id: Option<i64>,
    pub device_id: Option<i64>,
    pub monitor_id: Option<i64>,
    pub scope_key: Option<String>,
    pub status: String,
    pub severity: String,
    pub message: Option<String>,
    pub data: Option<Value>,
    pub started_at: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Derivado: nome da regra + alvo, com fallback para o título gravado no
    /// evento. Nunca vazio — a tela usa isto como rótulo da linha.
    pub title: String,
    pub alert_rule: Option<RelatedSummary>,
    pub device: Option<RelatedSummary>,
    pub monitor: Option<RelatedSummary>,
    pub silenced_until: Option<String>,
}

/// Nomes das entidades relacionadas, carregados de uma vez.
///
/// Uma consulta por tabela para o conjunto inteiro da página, em vez de três
/// por linha — é o que evita o N+1 na Central de Alertas.
#[derive(Debug, Default)]
pub struct AlertRelations {
    rules: HashMap<i64, String>,
    devices: HashMap<i64, String>,
    monitors: HashMap<i64, String>,
}

impl AlertRelations {
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn load<C: ConnectionTrait>(
        db: &C,
        events: &[alert_events::Model],
    ) -> AppResult<Self> {
        let ids = |pick: fn(&alert_events::Model) -> Option<i64>| -> Vec<i64> {
            let mut values: Vec<i64> = events.iter().filter_map(pick).collect();
            values.sort_unstable();
            values.dedup();
            values
        };

        let rule_ids = ids(|event| event.alert_rule_id);
        let device_ids = ids(|event| event.device_id);
        let monitor_ids = ids(|event| event.monitor_id);

        let mut relations = Self::default();
        if !rule_ids.is_empty() {
            for row in alert_rules::Entity::find()
                .filter(alert_rules::Column::Id.is_in(rule_ids))
                .all(db)
                .await?
            {
                relations.rules.insert(row.id, row.name);
            }
        }
        if !device_ids.is_empty() {
            for row in devices::Entity::find()
                .filter(devices_entity::Column::Id.is_in(device_ids))
                .all(db)
                .await?
            {
                relations.devices.insert(row.id, row.name);
            }
        }
        if !monitor_ids.is_empty() {
            for row in monitors::Entity::find()
                .filter(monitors::Column::Id.is_in(monitor_ids))
                .all(db)
                .await?
            {
                relations.monitors.insert(row.id, row.name);
            }
        }
        Ok(relations)
    }

    fn summary(map: &HashMap<i64, String>, id: Option<i64>) -> Option<RelatedSummary> {
        let id = id?;
        map.get(&id).map(|name| RelatedSummary {
            id,
            name: name.clone(),
        })
    }
}

/// Achata as relações e deriva o título exibido na Central de Alertas.
#[must_use]
pub fn serialize_event(
    event: &alert_events::Model,
    relations: &AlertRelations,
) -> SerializedAlertEvent {
    let data_string = |key: &str| -> Option<String> {
        event
            .data
            .as_ref()?
            .get(key)?
            .as_str()
            .map(ToString::to_string)
    };

    let alert_rule = AlertRelations::summary(&relations.rules, event.alert_rule_id);
    let device = AlertRelations::summary(&relations.devices, event.device_id);
    let monitor = AlertRelations::summary(&relations.monitors, event.monitor_id);

    // Precedência do backend anterior: título gravado no disparo, senão
    // "<regra> — <alvo>", senão o rótulo genérico. O nome da regra pode vir do
    // `data` quando a regra já foi apagada — é o que preserva o histórico.
    let title = data_string("title").unwrap_or_else(|| {
        let rule_name = alert_rule
            .as_ref()
            .map(|item| item.name.clone())
            .or_else(|| data_string("ruleName"));
        let target = device
            .as_ref()
            .or(monitor.as_ref())
            .map(|item| item.name.clone());
        let parts: Vec<String> = [rule_name, target].into_iter().flatten().collect();
        if parts.is_empty() {
            "Alerta do sistema".to_string()
        } else {
            parts.join(" — ")
        }
    });

    SerializedAlertEvent {
        id: event.id,
        alert_rule_id: event.alert_rule_id,
        device_id: event.device_id,
        monitor_id: event.monitor_id,
        scope_key: event.scope_key.clone(),
        status: event.status.clone(),
        severity: event.severity.clone(),
        message: event.message.clone(),
        data: event.data.clone(),
        started_at: Some(event.started_at.to_rfc3339()),
        resolved_at: event.resolved_at.map(|value| value.to_rfc3339()),
        created_at: Some(event.created_at.to_rfc3339()),
        updated_at: Some(event.updated_at.to_rfc3339()),
        title,
        alert_rule,
        device,
        monitor,
        silenced_until: data_string("silencedUntil"),
    }
}

/// Serializa uma lista inteira, carregando as relações de uma vez.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn serialize_events<C: ConnectionTrait>(
    db: &C,
    events: Vec<alert_events::Model>,
) -> AppResult<Vec<SerializedAlertEvent>> {
    let relations = AlertRelations::load(db, &events).await?;
    Ok(events
        .iter()
        .map(|event| serialize_event(event, &relations))
        .collect())
}

/// Coluna de ordenação usada por `GET /api/alerts` e `/api/events`.
#[must_use]
pub const fn newest_first() -> alert_events_entity::Column {
    alert_events_entity::Column::Id
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    fn evento(data: Option<Value>) -> alert_events::Model {
        let now = Utc::now().into();
        alert_events::Model {
            id: 7,
            alert_rule_id: Some(1),
            device_id: Some(2),
            monitor_id: Some(3),
            scope_key: Some("monitor:3".into()),
            status: "active".into(),
            severity: "critical".into(),
            started_at: now,
            resolved_at: None,
            message: Some("Host inacessível".into()),
            data,
            created_at: now,
            updated_at: now,
        }
    }

    fn relacoes() -> AlertRelations {
        let mut relations = AlertRelations::default();
        relations.rules.insert(1, "Dispositivo sem resposta".into());
        relations.devices.insert(2, "Roteador Matriz".into());
        relations.monitors.insert(3, "Ping Roteador".into());
        relations
    }

    #[test]
    fn titulo_gravado_no_disparo_tem_precedencia() {
        let serialized = serialize_event(
            &evento(Some(json!({ "title": "Título gravado" }))),
            &relacoes(),
        );
        assert_eq!(serialized.title, "Título gravado");
    }

    #[test]
    fn sem_titulo_deriva_regra_e_alvo() {
        let serialized = serialize_event(&evento(None), &relacoes());
        assert_eq!(
            serialized.title,
            "Dispositivo sem resposta — Roteador Matriz"
        );
    }

    #[test]
    fn sem_dispositivo_o_alvo_e_o_monitor() {
        let mut relations = relacoes();
        relations.devices.clear();
        let serialized = serialize_event(&evento(None), &relations);
        assert_eq!(serialized.title, "Dispositivo sem resposta — Ping Roteador");
    }

    #[test]
    fn regra_apagada_ainda_rende_titulo_pelo_data() {
        let mut relations = relacoes();
        relations.rules.clear();
        let serialized = serialize_event(
            &evento(Some(json!({ "ruleName": "Regra removida" }))),
            &relations,
        );
        assert_eq!(serialized.title, "Regra removida — Roteador Matriz");
        assert!(serialized.alert_rule.is_none());
    }

    #[test]
    fn sem_nada_cai_no_rotulo_generico() {
        let serialized = serialize_event(&evento(None), &AlertRelations::default());
        assert_eq!(serialized.title, "Alerta do sistema");
    }

    #[test]
    fn silenced_until_sai_do_data_para_o_topo() {
        let serialized = serialize_event(
            &evento(Some(json!({ "silencedUntil": "2026-08-11T12:00:00Z" }))),
            &relacoes(),
        );
        assert_eq!(
            serialized.silenced_until.as_deref(),
            Some("2026-08-11T12:00:00Z")
        );
    }

    #[test]
    fn regra_serializa_com_is_enabled() {
        let now = Utc::now().into();
        let response = AlertRuleResponse::from(alert_rules::Model {
            id: 1,
            site_id: None,
            device_id: None,
            monitor_id: None,
            name: "Regra".into(),
            r#type: "custom".into(),
            template_key: Some("device_offline".into()),
            condition: json!({ "field": "status", "operator": "eq", "value": "down" }),
            severity: "critical".into(),
            duration_seconds: 0,
            recovery_window_seconds: 120,
            flap_threshold: 5,
            flap_window_seconds: 900,
            notification_cooldown_seconds: 900,
            inhibit_when_parent_down: true,
            enabled: true,
            created_at: now,
            updated_at: now,
        });
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["isEnabled"], true);
        assert_eq!(json["durationSeconds"], 0);
        assert_eq!(json["recoveryWindowSeconds"], 120);
        assert_eq!(json["flapThreshold"], 5);
        assert_eq!(json["flapWindowSeconds"], 900);
        assert_eq!(json["notificationCooldownSeconds"], 900);
        assert_eq!(json["inhibitWhenParentDown"], true);
        assert_eq!(json["templateKey"], "device_offline");
        assert_eq!(json["type"], "custom");
    }
}
