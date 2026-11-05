//! Os registros que o ponto de montagem preenche (ADR 017): cada área
//! entrega as suas peças e o núcleo só consulta o registro. É o único lugar,
//! fora de `app.rs`, que conhece todas as áreas.

use std::sync::Arc;

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_core::{areas::AreaPolicy, models::devices, monitoring::MonitorCheckers};
use netmonitor_vpn::central::{Central, CentralServices};
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::{
    controllers::devices::present_for_vpn,
    services::{
        alerts::vpn_peer_watcher, area_policies::NetworkPolicy, docker::source::DockerEngine,
        server_addresses, shared::errors::AppResult,
    },
};

/// As políticas das áreas opcionais.
#[must_use]
pub fn area_policies() -> Vec<Box<dyn AreaPolicy>> {
    vec![
        Box::new(NetworkPolicy),
        Box::new(netmonitor_servers::ServersPolicy),
        Box::new(netmonitor_vpn::VpnPolicy),
    ]
}

/// Os tipos de monitor das áreas. A Docker Engine só existe no agente
/// (ADR 014): a central passa `None`.
#[must_use]
pub fn monitor_checkers(docker: Option<Arc<dyn DockerEngine>>) -> MonitorCheckers {
    MonitorCheckers::new(netmonitor_servers::monitor_checkers(docker))
}

/// O que a área VPN pede ao resto da central: o motor de alertas, a
/// apresentação de dispositivo e a lista de endereços da central.
struct VpnCentral;

#[async_trait]
impl CentralServices for VpnCentral {
    async fn evaluate_peer_states(&self, ctx: &AppContext, vpn_server_id: i64) -> AppResult<usize> {
        vpn_peer_watcher::evaluate_server_peers(ctx, vpn_server_id).await
    }

    async fn present_device(
        &self,
        db: &DatabaseConnection,
        device: devices::Model,
    ) -> AppResult<Option<Value>> {
        Ok(serde_json::to_value(present_for_vpn(db, device).await?).ok())
    }

    async fn forget_address_override(
        &self,
        db: &DatabaseConnection,
        kind: &str,
        detected: &str,
    ) -> AppResult<bool> {
        server_addresses::forget_override_if(db, kind, detected).await
    }
}

/// Os serviços da central para a VPN, para o `shared_store`.
#[must_use]
pub fn vpn_central() -> Central {
    Central::new(Arc::new(VpnCentral))
}

#[cfg(test)]
mod tests {
    use netmonitor_core::areas::Area;

    use super::*;

    #[test]
    fn todas_as_areas_opcionais_tem_politica() {
        let areas: Vec<Area> = area_policies().iter().map(|policy| policy.area()).collect();
        for area in Area::OPTIONAL {
            assert!(areas.contains(&area), "{area} sem política");
        }
    }

    #[test]
    fn o_monitor_de_container_esta_registrado() {
        assert!(monitor_checkers(None).find("container").is_some());
    }
}
