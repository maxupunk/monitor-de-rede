//! Os registros que o ponto de montagem preenche (ADR 017): cada área
//! entrega as suas peças e o núcleo só consulta o registro. É o único lugar,
//! fora de `app.rs`, que conhece todas as áreas.

use std::sync::Arc;

use netmonitor_core::{areas::AreaPolicy, monitoring::MonitorCheckers};

use crate::services::{
    area_policies::{NetworkPolicy, VpnPolicy},
    docker::source::DockerEngine,
};

/// As políticas das áreas opcionais.
#[must_use]
pub fn area_policies() -> Vec<Box<dyn AreaPolicy>> {
    vec![
        Box::new(NetworkPolicy),
        Box::new(netmonitor_servers::ServersPolicy),
        Box::new(VpnPolicy),
    ]
}

/// Os tipos de monitor das áreas. A Docker Engine só existe no agente
/// (ADR 014): a central passa `None`.
#[must_use]
pub fn monitor_checkers(docker: Option<Arc<dyn DockerEngine>>) -> MonitorCheckers {
    MonitorCheckers::new(netmonitor_servers::monitor_checkers(docker))
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
