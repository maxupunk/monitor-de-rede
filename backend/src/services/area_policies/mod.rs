//! O que cada área opcional sabe sobre si mesma (ADR 017): evidência de uso,
//! consequências de desligar e bloqueios.
//!
//! Moram aqui só enquanto as áreas não têm crate próprio; cada política vai
//! junto com a sua área quando ela for extraída (roadmap, Fases 6, 7 e 9).

mod network;
mod servers;
mod vpn;

use netmonitor_core::{
    areas::{AreaPolicy, Consequence},
    shared::text::count_noun,
};

/// As políticas registradas — o ponto de montagem as entrega ao núcleo.
#[must_use]
pub fn registry() -> Vec<Box<dyn AreaPolicy>> {
    vec![
        Box::new(network::NetworkPolicy),
        Box::new(servers::ServersPolicy),
        Box::new(vpn::VpnPolicy),
    ]
}

/// Monitores nunca param pela área — a confirmação diz quantos seguem rodando.
fn monitors_keep_running(count: u64, singular: &str, plural_form: &str) -> Option<Consequence> {
    (count > 0).then(|| {
        Consequence::new(format!(
            "{} continua{} rodando e alertando: monitor nunca para pela área.",
            count_noun(count, singular, plural_form),
            if count == 1 { "" } else { "m" }
        ))
    })
}

#[cfg(test)]
mod tests {
    use netmonitor_core::areas::Area;

    use super::*;

    #[test]
    fn todas_as_areas_opcionais_tem_politica() {
        let areas: Vec<Area> = registry().iter().map(|policy| policy.area()).collect();
        for area in Area::OPTIONAL {
            assert!(areas.contains(&area), "{area} sem política");
        }
    }
}
