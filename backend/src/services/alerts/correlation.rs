//! Correlação temporal de alertas em cascata (Fase 3 do roadmap).
//!
//! Além da inibição por `parent_id`, que suprime notificações de filhos quando o
//! pai já está em alerta, este módulo explica *por que* vários dispositivos
//! caíram juntos: ele olha para eventos abertos numa janela curta em torno de um
//! alerta e sugere o evento mais antigo que está num dispositivo de infraestrutura
//! (pai de outros alvos correlacionados) como causa raiz provável.

use std::collections::{HashMap, HashSet};

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events, devices},
    services::{alerts::contracts::AlertStatus, shared::errors::AppResult},
    views::alerts::{serialize_event, AlertRelations, SerializedAlertEvent},
};

/// Janela padrão em torno do `started_at` do evento alvo.
pub const DEFAULT_WINDOW_SECONDS: i64 = 60;

/// Resultado da análise de correlação para um evento.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertCorrelation {
    /// Largura da janela usada para buscar eventos correlacionados.
    pub window_seconds: i64,
    /// Evento mais provável de ser a causa raiz comum.
    pub primary_cause: Option<SerializedAlertEvent>,
    /// Demais eventos abertos na mesma janela, exceto o evento alvo e a causa
    /// raiz (quando esta é diferente do alvo).
    pub related_events: Vec<SerializedAlertEvent>,
    /// Site compartilhado pelos eventos correlacionados, quando houver.
    pub common_site_id: Option<i64>,
    /// Rede compartilhada pelos eventos correlacionados, quando houver.
    pub common_network_id: Option<i64>,
    /// Quantos eventos foram encontrados na janela (inclui a causa raiz, mas
    /// não o evento alvo).
    pub correlation_count: usize,
}

/// Analisa a correlação temporal de um evento de alerta.
///
/// Busca eventos abertos numa janela de `window_seconds` antes e depois do
/// `started_at` do evento alvo. Entre eles, prefere como causa raiz o evento
/// mais antigo cujo dispositivo é pai declarado de outros dispositivos da
/// correlação; se não houver, cai para o evento mais antigo da janela.
///
/// # Errors
///
/// Propaga erro do banco ou `AppError::not_found` se o evento não existe.
pub async fn analyze<C: ConnectionTrait>(
    db: &C,
    event_id: i64,
    window_seconds: Option<i64>,
) -> AppResult<AlertCorrelation> {
    let window = window_seconds.unwrap_or(DEFAULT_WINDOW_SECONDS).max(1);

    let event = alert_events::Entity::find_by_id(event_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            crate::services::shared::errors::AppError::not_found("Alerta não encontrado")
        })?;

    let target_device = if let Some(device_id) = event.device_id {
        devices::Entity::find_by_id(device_id).one(db).await?
    } else {
        None
    };

    let from = event.started_at.with_timezone(&Utc) - Duration::seconds(window);
    let to = event.started_at.with_timezone(&Utc) + Duration::seconds(window);

    let candidates: Vec<alert_events::Model> = alert_events::Entity::find()
        .filter(alert_events_entity::Column::Id.ne(event_id))
        .filter(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN))
        .filter(alert_events_entity::Column::StartedAt.gte(from))
        .filter(alert_events_entity::Column::StartedAt.lte(to))
        .all(db)
        .await?;

    if candidates.is_empty() {
        return Ok(AlertCorrelation {
            window_seconds: window,
            primary_cause: None,
            related_events: Vec::new(),
            common_site_id: target_device.as_ref().and_then(|d| d.site_id),
            common_network_id: target_device.as_ref().and_then(|d| d.network_id),
            correlation_count: 0,
        });
    }

    let device_ids: HashSet<i64> = candidates
        .iter()
        .filter_map(|event| event.device_id)
        .collect();

    let devices_map: HashMap<i64, devices::Model> = if device_ids.is_empty() {
        HashMap::new()
    } else {
        devices::Entity::find()
            .filter(devices::Column::Id.is_in(device_ids.iter().copied().collect::<Vec<_>>()))
            .all(db)
            .await?
            .into_iter()
            .map(|device| (device.id, device))
            .collect()
    };

    let (common_site_id, common_network_id) =
        common_scopes(target_device.as_ref(), &candidates, &devices_map);

    let primary = primary_cause(&candidates, &devices_map);

    let relations = AlertRelations::load(db, &candidates).await?;
    let mut serialized: Vec<SerializedAlertEvent> = candidates
        .iter()
        .map(|event| serialize_event(event, &relations))
        .collect();

    let primary_serialized = primary.map(|event| serialize_event(event, &relations));

    if let Some(ref primary) = primary_serialized {
        serialized.retain(|event| event.id != primary.id);
    }

    Ok(AlertCorrelation {
        window_seconds: window,
        primary_cause: primary_serialized,
        related_events: serialized,
        common_site_id,
        common_network_id,
        correlation_count: candidates.len(),
    })
}

/// Site/rede comuns entre o evento alvo e os candidatos.
fn common_scopes(
    target_device: Option<&devices::Model>,
    candidates: &[alert_events::Model],
    devices_map: &HashMap<i64, devices::Model>,
) -> (Option<i64>, Option<i64>) {
    let mut site_ids: HashSet<Option<i64>> = HashSet::new();
    let mut network_ids: HashSet<Option<i64>> = HashSet::new();

    site_ids.insert(target_device.and_then(|d| d.site_id));
    network_ids.insert(target_device.and_then(|d| d.network_id));

    for event in candidates {
        if let Some(device) = event.device_id.and_then(|id| devices_map.get(&id)) {
            site_ids.insert(device.site_id);
            network_ids.insert(device.network_id);
        }
    }

    let common_site = if site_ids.len() == 1 {
        site_ids.into_iter().next().flatten()
    } else {
        None
    };

    let common_network = if network_ids.len() == 1 {
        network_ids.into_iter().next().flatten()
    } else {
        None
    };

    (common_site, common_network)
}

/// Escolhe a causa raiz provável entre os candidatos.
///
/// O critério principal é ser um dispositivo pai de outros dispositivos
/// correlacionados (infraestrutura). Em caso de empate, vence o evento mais
/// antigo. Se nenhum candidato for pai, vence o evento mais antigo da janela.
fn primary_cause<'a>(
    candidates: &'a [alert_events::Model],
    devices_map: &HashMap<i64, devices::Model>,
) -> Option<&'a alert_events::Model> {
    let candidate_device_ids: HashSet<i64> = candidates
        .iter()
        .filter_map(|event| event.device_id)
        .collect();

    let mut parent_candidates: Vec<&alert_events::Model> = candidates
        .iter()
        .filter(|event| {
            event.device_id.map_or(false, |device_id| {
                candidate_device_ids.iter().any(|candidate_id| {
                    devices_map
                        .get(candidate_id)
                        .and_then(|device| device.parent_id)
                        == Some(device_id)
                })
            })
        })
        .collect();

    if parent_candidates.is_empty() {
        parent_candidates = candidates.iter().collect();
    }

    parent_candidates.sort_by(|a, b| {
        a.started_at
            .with_timezone(&Utc)
            .cmp(&b.started_at.with_timezone(&Utc))
            .then_with(|| a.id.cmp(&b.id))
    });

    parent_candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn device(
        id: i64,
        parent_id: Option<i64>,
        site_id: Option<i64>,
        network_id: Option<i64>,
    ) -> devices::Model {
        devices::Model {
            id,
            site_id,
            network_id,
            parent_id,
            ip_address: None,
            name: format!("device-{id}"),
            r#type: "router".into(),
            vendor: None,
            model: None,
            serial_number: None,
            description: None,
            is_monitored: true,
            snmp_enabled: false,
            snmp_community: None,
            snmp_version: None,
            snmp_poll_interval_seconds: 60,
            access_mode: None,
            operating_system: None,
            system_key: None,
            status: "up".into(),
            last_seen_at: None,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    fn event(id: i64, device_id: Option<i64>, seconds_ago: i64) -> alert_events::Model {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
        alert_events::Model {
            id,
            alert_rule_id: Some(1),
            device_id,
            monitor_id: None,
            scope_key: Some(format!("monitor:{id}")),
            status: AlertStatus::Active.as_str().into(),
            severity: "critical".into(),
            started_at: (now - Duration::seconds(seconds_ago)),
            resolved_at: None,
            message: Some("down".into()),
            data: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn causa_raiz_eh_o_pai_mais_antigo() {
        let candidates = vec![
            event(1, Some(20), 5), // pai (device 20 é parent de 10), mais antigo
            event(2, Some(10), 3), // filho
            event(3, Some(30), 0), // outro filho independente
        ];
        let mut devices_map = HashMap::new();
        devices_map.insert(10, device(10, Some(20), Some(1), Some(1)));
        devices_map.insert(20, device(20, None, Some(1), Some(1)));
        devices_map.insert(30, device(30, None, Some(1), Some(1)));

        let primary = primary_cause(&candidates, &devices_map);
        assert_eq!(primary.map(|e| e.id), Some(1));
    }

    #[test]
    fn sem_pai_cai_para_o_mais_antigo() {
        let candidates = vec![
            event(1, Some(10), 5),
            event(2, Some(20), 3),
            event(3, Some(30), 0),
        ];
        let mut devices_map = HashMap::new();
        devices_map.insert(10, device(10, None, Some(1), Some(1)));
        devices_map.insert(20, device(20, None, Some(1), Some(1)));
        devices_map.insert(30, device(30, None, Some(1), Some(1)));

        let primary = primary_cause(&candidates, &devices_map);
        assert_eq!(primary.map(|e| e.id), Some(1));
    }

    #[test]
    fn escopo_comum_aparece_quando_todos_iguais() {
        let target = device(100, None, Some(1), Some(2));
        let candidates = vec![event(1, Some(10), 0), event(2, Some(20), 0)];
        let mut devices_map = HashMap::new();
        devices_map.insert(10, device(10, None, Some(1), Some(2)));
        devices_map.insert(20, device(20, None, Some(1), Some(2)));

        let (site, network) = common_scopes(Some(&target), &candidates, &devices_map);
        assert_eq!(site, Some(1));
        assert_eq!(network, Some(2));
    }

    #[test]
    fn escopo_comum_some_quando_divergem() {
        let target = device(100, None, Some(1), Some(2));
        let candidates = vec![event(1, Some(10), 0), event(2, Some(20), 0)];
        let mut devices_map = HashMap::new();
        devices_map.insert(10, device(10, None, Some(1), Some(2)));
        devices_map.insert(20, device(20, None, Some(3), Some(2)));

        let (site, network) = common_scopes(Some(&target), &candidates, &devices_map);
        assert_eq!(site, None);
        assert_eq!(network, Some(2));
    }
}
