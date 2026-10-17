//! As pontas de uma perna: um **equipamento** (pelo plugin ativo e instalado
//! que declara papéis de teste), a **central** (motores nativos) ou um
//! **agente** (só dirige). Cada uma sabe o que serve e o que dirige.

use std::net::IpAddr;

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use super::{
    model::{LinkTestEndpoint, LinkTestEndpointKind},
    negotiate::{Capabilities, Party},
};
use crate::{
    models::{device_credentials, devices, plugins, probes},
    services::{
        devices::system_device::NETMONITOR_DEFAULT_NAME,
        network_tools::throughput::ThroughputEngine,
        plugins::{credentials, manifest::ThroughputRoles, service},
        shared::errors::{AppError, AppResult},
    },
};

/// O plugin que faz os papéis de teste num equipamento.
#[derive(Debug, Clone)]
pub struct DevicePlugin {
    pub slug: String,
    pub name: String,
    pub roles: ThroughputRoles,
}

/// Quem está por trás de uma ponta.
#[derive(Debug, Clone)]
pub enum Role {
    Device {
        device: Box<devices::Model>,
        plugin: Option<DevicePlugin>,
    },
    Central {
        /// O dispositivo do sistema, quando cadastrado.
        device: Option<Box<devices::Model>>,
    },
    Agent {
        agent_id: i64,
    },
}

/// Uma ponta pronta para o plano e para a execução.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub view: LinkTestEndpoint,
    pub role: Role,
    pub caps: Capabilities,
    /// O que impede a ponta de participar (credencial, por exemplo).
    pub issues: Vec<String>,
}

impl Endpoint {
    /// Como a negociação a vê.
    #[must_use]
    pub fn party(&self) -> Party<'_> {
        Party {
            kind: self.view.kind,
            name: &self.view.name,
            caps: &self.caps,
        }
    }

    /// IP cadastrado, quando é um endereço válido.
    #[must_use]
    pub fn ip(&self) -> Option<IpAddr> {
        self.view
            .ip
            .as_deref()
            .and_then(|ip| ip.trim().parse().ok())
    }

    /// O equipamento por trás da ponta (o do sistema, na central).
    #[must_use]
    pub fn device(&self) -> Option<&devices::Model> {
        match &self.role {
            Role::Device { device, .. } => Some(device),
            Role::Central { device } => device.as_deref(),
            Role::Agent { .. } => None,
        }
    }

    /// O plugin de teste, numa ponta equipamento.
    #[must_use]
    pub fn plugin(&self) -> Option<&DevicePlugin> {
        match &self.role {
            Role::Device { plugin, .. } => plugin.as_ref(),
            _ => None,
        }
    }

    /// Id do equipamento, só para ponta equipamento (a central e o agente
    /// não entram na regra de "um teste por dispositivo").
    #[must_use]
    pub fn device_id(&self) -> Option<i64> {
        match &self.role {
            Role::Device { device, .. } => Some(device.id),
            _ => None,
        }
    }
}

/// Plataforma para a tela: o sistema escolhido, o observado ou o fabricante.
fn platform(device: &devices::Model) -> Option<String> {
    device
        .operating_system
        .clone()
        .or_else(|| device.observed_os.clone())
        .or_else(|| device.vendor.clone())
}

/// O plugin de teste do equipamento: o primeiro plugin ativo e instalado com
/// bloco `throughput` — o exclusivo do equipamento vale mais.
///
/// # Errors
///
/// Erro do banco ou manifesto gravado fora do formato.
pub async fn device_plugin<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
) -> AppResult<Option<(plugins::Model, DevicePlugin)>> {
    let mut found = Vec::new();
    for (_, plugin) in service::installed_active(db, Some(&[device_id])).await? {
        let manifest = service::package_of(&plugin)?.manifest;
        if let Some(roles) = manifest.throughput.clone() {
            found.push((
                plugin.device_id.is_none(),
                plugin.clone(),
                DevicePlugin {
                    slug: plugin.slug.clone(),
                    name: plugin.name.clone(),
                    roles,
                },
            ));
        }
    }
    // `installed_active` já vem por nome; a ordenação estável só sobe o
    // exclusivo do equipamento.
    found.sort_by_key(|(shared, _, _)| *shared);
    Ok(found
        .into_iter()
        .next()
        .map(|(_, plugin, device_plugin)| (plugin, device_plugin)))
}

/// Uma ponta equipamento.
///
/// # Errors
///
/// Erro do banco.
pub async fn device_endpoint<C: ConnectionTrait>(
    db: &C,
    device: devices::Model,
) -> AppResult<Endpoint> {
    let view = LinkTestEndpoint {
        kind: LinkTestEndpointKind::Device,
        device_id: Some(device.id),
        agent_id: None,
        name: device.name.clone(),
        ip: device.ip_address.clone().filter(|ip| !ip.trim().is_empty()),
        platform: platform(&device),
    };
    let mut issues = Vec::new();
    let found = device_plugin(db, device.id).await?;
    if let Some((plugin, _)) = &found {
        let transports = service::package_of(plugin)?.manifest.transports;
        if let Err(error) = credentials::resolve(db, device.id, &transports).await {
            match error {
                AppError::Internal(_) => return Err(error),
                other => issues.push(format!("{}: {other}", device.name)),
            }
        }
    }
    if view.ip.is_none() {
        issues.push(format!("{} não tem IP cadastrado", device.name));
    }
    let plugin = found.map(|(_, plugin)| plugin);
    let caps = plugin
        .as_ref()
        .map(|plugin| Capabilities::from_roles(&plugin.roles))
        .unwrap_or_default();
    Ok(Endpoint {
        view,
        role: Role::Device {
            device: Box::new(device),
            plugin,
        },
        caps,
        issues,
    })
}

/// A central — o dispositivo do sistema, quando cadastrado.
#[must_use]
pub fn central_endpoint(system: Option<devices::Model>) -> Endpoint {
    let view = LinkTestEndpoint {
        kind: LinkTestEndpointKind::Central,
        device_id: system.as_ref().map(|device| device.id),
        agent_id: None,
        name: system.as_ref().map_or_else(
            || NETMONITOR_DEFAULT_NAME.to_owned(),
            |device| device.name.clone(),
        ),
        ip: system
            .as_ref()
            .and_then(|device| device.ip_address.clone())
            .filter(|ip| !ip.trim().is_empty()),
        platform: Some("netmonitor".to_owned()),
    };
    Endpoint {
        view,
        role: Role::Central {
            device: system.map(Box::new),
        },
        caps: Capabilities::central(),
        issues: Vec::new(),
    }
}

/// Um agente — a linha de `probes`.
///
/// # Errors
///
/// `NotFound` se o agente não existe; erro do banco.
pub async fn agent_endpoint<C: ConnectionTrait>(db: &C, agent_id: i64) -> AppResult<Endpoint> {
    let probe = probes::Entity::find_by_id(agent_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Agente não encontrado"))?;
    let host = match probe.device_id {
        Some(id) => devices::Entity::find_by_id(id).one(db).await?,
        None => None,
    };
    Ok(Endpoint {
        view: LinkTestEndpoint {
            kind: LinkTestEndpointKind::Agent,
            device_id: probe.device_id,
            agent_id: Some(probe.id),
            name: format!("Agente {}", probe.name),
            ip: host.and_then(|device| device.ip_address),
            platform: Some("netmonitor-agent".to_owned()),
        },
        role: Role::Agent { agent_id: probe.id },
        caps: Capabilities::agent(),
        issues: Vec::new(),
    })
}

/// O agente por onde a central alcança o equipamento (credencial com
/// `via_probe_id`), quando houver.
///
/// # Errors
///
/// Erro do banco.
pub async fn agent_of<C: ConnectionTrait>(db: &C, device_id: i64) -> AppResult<Option<i64>> {
    Ok(device_credentials::Entity::find()
        .filter(device_credentials::Column::DeviceId.eq(device_id))
        .filter(device_credentials::Column::ViaProbeId.is_not_null())
        .all(db)
        .await?
        .into_iter()
        .find_map(|credential| credential.via_probe_id))
}

/// Pacotes que o `prepare` instala para o papel — o que a tela avisa antes
/// de rodar ("pode instalar").
#[must_use]
pub fn packages_for(engine: ThroughputEngine, serves: bool) -> Vec<String> {
    match (engine, serves) {
        (ThroughputEngine::Iperf3, _) => vec!["iperf3".to_owned()],
        (ThroughputEngine::Btest, false) => vec!["mikrotik-btest".to_owned()],
        (ThroughputEngine::Btest, true) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacotes_por_motor_e_papel() {
        assert_eq!(packages_for(ThroughputEngine::Iperf3, true), ["iperf3"]);
        assert_eq!(packages_for(ThroughputEngine::Iperf3, false), ["iperf3"]);
        assert_eq!(
            packages_for(ThroughputEngine::Btest, false),
            ["mikrotik-btest"]
        );
        assert!(packages_for(ThroughputEngine::Btest, true).is_empty());
    }

    #[test]
    fn central_sem_dispositivo_do_sistema_tem_nome_padrao() {
        let central = central_endpoint(None);
        assert_eq!(central.view.name, NETMONITOR_DEFAULT_NAME);
        assert_eq!(central.view.kind, LinkTestEndpointKind::Central);
        assert!(central.caps.serves(ThroughputEngine::Btest));
        assert!(central.caps.driver(ThroughputEngine::Iperf3).is_some());
        assert_eq!(central.device_id(), None, "a central não entra na trava");
    }
}
