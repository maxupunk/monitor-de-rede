//! A topologia como superfície de plugin (ADR 015, §7): as ações que cada
//! plugin ativo instalado oferece no nó do equipamento (`topology.nodeActions`).

use std::collections::BTreeMap;

use sea_orm::ConnectionTrait;

use super::{
    manifest::{PluginManifest, Surface},
    service,
};
use crate::{
    dtos::plugins::TopologyNodeAction, models::plugins, services::shared::errors::AppResult,
};

/// As ações de nó de um plugin, na ordem do manifesto. Vazio quando ele não
/// aparece na topologia.
#[must_use]
pub fn node_actions_of(
    plugin: &plugins::Model,
    manifest: &PluginManifest,
) -> Vec<TopologyNodeAction> {
    let Some(topology) = manifest
        .topology
        .as_ref()
        .filter(|_| manifest.shows_on(Surface::Topology))
    else {
        return Vec::new();
    };
    topology
        .node_actions
        .iter()
        .filter_map(|id| manifest.action(id))
        .map(|action| TopologyNodeAction {
            plugin_id: plugin.id,
            plugin_slug: plugin.slug.clone(),
            plugin_name: plugin.name.clone(),
            action: action.id.clone(),
            title: action.title.clone(),
            effect: action.effect,
            has_params: action.params.as_ref().is_some_and(|schema| {
                schema
                    .get("properties")
                    .and_then(serde_json::Value::as_object)
                    .is_some_and(|properties| !properties.is_empty())
            }),
            definition: action.clone(),
        })
        .collect()
}

/// Por equipamento (só os de `device_ids`, quando há lista), as ações que os
/// plugins ativos instalados nele oferecem no nó. Equipamento sem nenhuma
/// fica de fora.
///
/// # Errors
///
/// Erro do banco ou manifesto gravado fora do formato.
pub async fn node_actions<C: ConnectionTrait>(
    db: &C,
    device_ids: Option<&[i64]>,
) -> AppResult<BTreeMap<i64, Vec<TopologyNodeAction>>> {
    let mut by_device: BTreeMap<i64, Vec<TopologyNodeAction>> = BTreeMap::new();
    for (device_id, plugin) in service::installed_active(db, device_ids).await? {
        let manifest = service::package_of(&plugin)?.manifest;
        let actions = node_actions_of(&plugin, &manifest);
        if !actions.is_empty() {
            by_device.entry(device_id).or_default().extend(actions);
        }
    }
    Ok(by_device)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plugin() -> plugins::Model {
        plugins::Model {
            id: 7,
            slug: "mikrotik".into(),
            name: "MikroTik".into(),
            version: "1.0.0".into(),
            description: None,
            scope: "model".into(),
            device_id: None,
            source: "builtin".into(),
            status: "active".into(),
            manifest: json!({}),
            script: String::new(),
            usage: String::new(),
            compatibility: json!([]),
            tests: json!({}),
            review: None,
            checksum: String::new(),
            last_test_at: None,
            last_test_ok: None,
            auto_enable: false,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    fn manifest(surfaces: &[&str]) -> PluginManifest {
        serde_json::from_value(json!({
            "slug": "mikrotik", "name": "MikroTik", "version": "1.0.0", "transports": ["ssh"],
            "surfaces": surfaces,
            "actions": [
                { "id": "detect", "title": "Detectar", "effect": "read" },
                { "id": "ping_from", "title": "Ping a partir daqui", "effect": "read",
                  "params": { "type": "object", "properties": { "target": { "type": "string" } } } }
            ],
            "topology": { "nodeActions": ["ping_from", "detect"] }
        }))
        .unwrap()
    }

    #[test]
    fn acoes_de_no_seguem_a_ordem_do_manifesto() {
        let actions = node_actions_of(&plugin(), &manifest(&["device", "topology"]));
        let ids: Vec<_> = actions
            .iter()
            .map(|action| action.action.as_str())
            .collect();
        assert_eq!(ids, ["ping_from", "detect"]);
        assert!(actions[0].has_params);
        assert!(!actions[1].has_params);
        assert_eq!(actions[0].plugin_slug, "mikrotik");
        assert_eq!(actions[0].definition.title, "Ping a partir daqui");
    }

    #[test]
    fn sem_a_superficie_nada_aparece() {
        assert!(node_actions_of(&plugin(), &manifest(&["device"])).is_empty());
    }
}
