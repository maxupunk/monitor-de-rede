//! Grafo da topologia (dispositivos, enlaces e hierarquia) no contrato do Vue.
//!
//! As regras puras ficam nos módulos vizinhos — estado do enlace em
//! [`edge_state`], inferência em [`inference`], menor caminho em [`path`] — e
//! aqui só se carrega o banco e monta a resposta.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use sea_orm::{
    sea_query::Query, ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use ts_rs::TS;

use crate::{
    dtos::resources::UnmanagedSwitchInput,
    models::{
        _entities::{
            device_interfaces as device_interfaces_entity, device_links as device_links_entity,
        },
        device_interfaces, device_links, devices, monitors, sites,
    },
    services::{
        devices::system_device::NETMONITOR_KEY,
        monitoring::metrics_repository,
        shared::{
            errors::{AppError, AppResult},
            transaction::begin_write,
        },
        snmp::collectors::LldpNeighbor,
        topology::{
            edge_state::{edge_speed_bps, edge_status, edge_utilization, is_stale, EndpointState},
            inference::{infer_uplinks, pair_key, INFERRED_METHOD},
            link_resolver::{persist_links_in, persist_resolved_links_detailed, NetworkLink},
            path::{shortest_path, PathEdge, TopologyPath},
        },
    },
};

/// Quantos ids vão num `IN (...)`: longe do teto de parâmetros do SQLite
/// (32766) e do PostgreSQL (65535).
const IDS_PER_QUERY: usize = 500;

#[derive(Debug, Clone, serde::Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyNode {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub status: String,
    #[ts(type = "number | null")]
    pub site_id: Option<i64>,
    pub site_name: Option<String>,
    pub interface_count: usize,
    pub ip_address: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub snmp_enabled: bool,
    #[ts(type = "number | null")]
    pub parent_id: Option<i64>,
    /// O próprio NetMonitor (`devices.system_key = 'netmonitor'`): a origem
    /// natural de um teste "servidor → roteador".
    pub is_system: bool,
    /// Monitores habilitados deste dispositivo que estão `down` agora.
    pub monitors_down: usize,
}

#[derive(Debug, Clone, Default, serde::Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyEdge {
    /// Positivo: linha de `device_links`. Negativo: aresta virtual de
    /// hierarquia, `-id do filho` (ver [`parent_edge_id`]).
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub source: i64,
    #[ts(type = "number")]
    pub target: i64,
    #[ts(type = "number")]
    pub source_device_id: i64,
    #[ts(type = "number")]
    pub target_device_id: i64,
    pub source_device_name: Option<String>,
    pub target_device_name: Option<String>,
    #[ts(type = "number | null")]
    pub source_interface_id: Option<i64>,
    #[ts(type = "number | null")]
    pub target_interface_id: Option<i64>,
    pub source_interface_name: Option<String>,
    pub target_interface_name: Option<String>,
    #[ts(type = "number | null")]
    pub source_interface_speed: Option<i64>,
    #[ts(type = "number | null")]
    pub target_interface_speed: Option<i64>,
    pub source_interface_status: Option<String>,
    pub target_interface_status: Option<String>,
    pub in_bps: Option<f64>,
    pub out_bps: Option<f64>,
    pub traffic_bps: Option<f64>,
    pub traffic_label: Option<String>,
    /// Velocidade do enlace (a menor das pontas conhecidas), em bps.
    #[ts(type = "number | null")]
    pub speed_bps: Option<i64>,
    /// Sentido mais carregado sobre a velocidade, de 0 a 1.
    pub utilization: Option<f64>,
    pub link_type: String,
    pub discovery_method: String,
    pub confidence: i32,
    pub confirmed: bool,
    /// `up`, `down` ou `unknown` (ver [`edge_status`]).
    pub status: String,
    /// Enlace LLDP/CDP não revisto há mais de 7 dias. Manual nunca envelhece.
    pub stale: bool,
    /// Última vez que a descoberta viu o enlace (RFC 3339).
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct TopologyGraph {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
}

fn format_traffic_bps(bps: f64) -> String {
    if bps >= 1_000_000_000.0 {
        format!("{:.2} Gbps", bps / 1_000_000_000.0)
    } else if bps >= 100_000_000.0 {
        format!("{:.1} Mbps", bps / 1_000_000.0)
    } else if bps >= 1_000_000.0 {
        format!("{:.2} Mbps", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.1} Kbps", bps / 1_000.0)
    } else if bps > 0.0 {
        format!("{:.0} bps", bps)
    } else {
        "0 bps".to_string()
    }
}

/// Id da aresta virtual de hierarquia do filho `child_id`.
///
/// Um filho tem **um** pai, então `-child_id` identifica a aresta sem colisão
/// (o esquema antigo `-(filho * 1000 + pai)` colidia com pai ≥ 1000) e volta ao
/// filho sem varrer a tabela.
#[must_use]
pub const fn parent_edge_id(child_id: i64) -> i64 {
    -child_id
}

/// O filho dono de uma aresta virtual, ou `None` para id de enlace real.
#[must_use]
pub const fn parent_edge_child(edge_id: i64) -> Option<i64> {
    if edge_id < 0 {
        Some(-edge_id)
    } else {
        None
    }
}

/// Pares (pai, filho) que viram aresta virtual: os dois no recorte e sem
/// enlace real entre eles (um enlace real já descreve a ligação — duplicar
/// desenharia duas linhas entre o mesmo par).
fn parent_pairs<'a>(
    devices: impl IntoIterator<Item = &'a devices::Model>,
    ids: &BTreeSet<i64>,
    linked: &BTreeSet<(i64, i64)>,
) -> Vec<(i64, i64)> {
    devices
        .into_iter()
        .filter_map(|device| {
            let parent = device.parent_id.filter(|parent| ids.contains(parent))?;
            (!linked.contains(&pair_key(parent, device.id))).then_some((parent, device.id))
        })
        .collect()
}

/// Acrescenta a aresta virtual do `parentId` quando não existe enlace real
/// entre pai e filho (matriz de paridade #48).
///
/// O `id` é **negativo** de propósito: a tela usa o id da aresta para abrir o
/// enlace, e um id positivo aqui apontaria para uma linha de `device_links` que
/// não existe — o clique daria 404. O sinal é o que distingue "hierarquia
/// declarada pelo operador" de "enlace descoberto".
///
/// A aresta só entra se pai e filho estiverem **ambos** no recorte pedido: um
/// pai de outro site viraria nó fantasma no grafo.
fn append_parent_edges(
    edges: &mut Vec<TopologyEdge>,
    devices: &[devices::Model],
    ids: &BTreeSet<i64>,
) {
    let devices_by_id: BTreeMap<i64, &devices::Model> = devices.iter().map(|d| (d.id, d)).collect();
    let linked: BTreeSet<_> = edges
        .iter()
        .map(|edge| pair_key(edge.source, edge.target))
        .collect();
    for (parent, child) in parent_pairs(devices, ids, &linked) {
        let parent_device = devices_by_id.get(&parent);
        let child_device = devices_by_id.get(&child);
        let status = edge_status(
            EndpointState {
                device_status: parent_device.map(|d| d.status.as_str()),
                ..EndpointState::default()
            },
            EndpointState {
                device_status: child_device.map(|d| d.status.as_str()),
                ..EndpointState::default()
            },
        );
        edges.push(TopologyEdge {
            id: parent_edge_id(child),
            source: parent,
            target: child,
            source_device_id: parent,
            target_device_id: child,
            source_device_name: parent_device.map(|d| d.name.clone()),
            target_device_name: child_device.map(|d| d.name.clone()),
            link_type: "parent".into(),
            discovery_method: "parent_hierarchy".into(),
            confidence: 100,
            confirmed: true,
            status: status.into(),
            ..TopologyEdge::default()
        });
    }
}

/// Recorte de dispositivos do grafo. Com site, entra também o dispositivo do
/// sistema — o NetMonitor não pertence a site nenhum, mas é a origem dos
/// testes; ele sai depois se não tiver ligação com o site.
fn device_scope(site_id: Option<i64>) -> Option<Condition> {
    site_id.map(|site| {
        Condition::any()
            .add(devices::Column::SiteId.eq(site))
            .add(devices::Column::SystemKey.eq(NETMONITOR_KEY))
    })
}

/// Busca linhas por id em lotes de [`IDS_PER_QUERY`].
async fn interfaces_by_ids<C: ConnectionTrait>(
    db: &C,
    ids: &BTreeSet<i64>,
) -> AppResult<Vec<device_interfaces::Model>> {
    let ids: Vec<i64> = ids.iter().copied().collect();
    let mut rows = Vec::with_capacity(ids.len());
    for chunk in ids.chunks(IDS_PER_QUERY) {
        rows.extend(
            device_interfaces::Entity::find()
                .filter(device_interfaces_entity::Column::Id.is_in(chunk.to_vec()))
                .all(db)
                .await?,
        );
    }
    Ok(rows)
}

/// Última taxa de entrada/saída de cada interface citada pelos enlaces.
async fn latest_traffic(
    db: &sea_orm::DatabaseConnection,
    interface_ids: &BTreeSet<i64>,
) -> AppResult<BTreeMap<i64, (Option<f64>, Option<f64>)>> {
    let mut latest: BTreeMap<i64, (Option<f64>, Option<f64>)> = BTreeMap::new();
    if interface_ids.is_empty() {
        return Ok(latest);
    }
    let ids: Vec<i64> = interface_ids.iter().copied().collect();
    let recent_metrics = metrics_repository::latest_for_interfaces(
        db,
        None,
        &ids,
        &["inBps", "outBps", "traffic_rx_bytes", "traffic_tx_bytes"],
    )
    .await?;
    for m in recent_metrics {
        if let Some(iface_id) = m.interface_id {
            let entry = latest.entry(iface_id).or_insert((None, None));
            if (m.name == "inBps" || m.name == "traffic_rx_bytes") && entry.0.is_none() {
                entry.0 = Some(m.value);
            } else if (m.name == "outBps" || m.name == "traffic_tx_bytes") && entry.1.is_none() {
                entry.1 = Some(m.value);
            }
        }
    }
    Ok(latest)
}

/// Monta o grafo, com o filtro de site aplicado **no SQL**: dispositivos do
/// site, enlaces com as duas pontas nele e só as interfaces que os enlaces citam.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn get_topology(
    db: &sea_orm::DatabaseConnection,
    site_id: Option<i64>,
) -> AppResult<TopologyGraph> {
    let scope = device_scope(site_id);
    let mut device_query = devices::Entity::find().order_by_asc(devices::Column::Id);
    if let Some(scope) = scope.clone() {
        device_query = device_query.filter(scope);
    }
    let devices = device_query.all(db).await?;
    let ids: BTreeSet<_> = devices.iter().map(|device| device.id).collect();

    // `IN (SELECT id FROM devices WHERE …)`: igual no SQLite e no PostgreSQL.
    let scoped_ids = scope.map(|scope| {
        Query::select()
            .column(devices::Column::Id)
            .from(devices::Entity)
            .cond_where(scope)
            .to_owned()
    });

    let mut counts_query = device_interfaces::Entity::find()
        .select_only()
        .column(device_interfaces_entity::Column::DeviceId)
        .column_as(device_interfaces_entity::Column::Id.count(), "total")
        .group_by(device_interfaces_entity::Column::DeviceId);
    let mut monitors_query = monitors::Entity::find()
        .select_only()
        .column(monitors::Column::DeviceId)
        .column_as(monitors::Column::Id.count(), "total")
        .filter(monitors::Column::Enabled.eq(true))
        .filter(monitors::Column::Status.eq("down"))
        .filter(monitors::Column::DeviceId.is_not_null())
        .group_by(monitors::Column::DeviceId);
    let mut links_query =
        device_links::Entity::find().order_by_asc(device_links_entity::Column::Id);
    if let Some(subquery) = scoped_ids {
        counts_query = counts_query
            .filter(device_interfaces_entity::Column::DeviceId.in_subquery(subquery.clone()));
        monitors_query =
            monitors_query.filter(monitors::Column::DeviceId.in_subquery(subquery.clone()));
        links_query = links_query
            .filter(device_links_entity::Column::SourceDeviceId.in_subquery(subquery.clone()))
            .filter(device_links_entity::Column::TargetDeviceId.in_subquery(subquery));
    }
    let interface_counts: BTreeMap<i64, i64> = counts_query
        .into_tuple::<(i64, i64)>()
        .all(db)
        .await?
        .into_iter()
        .collect();
    let monitors_down: BTreeMap<i64, i64> = monitors_query
        .into_tuple::<(Option<i64>, i64)>()
        .all(db)
        .await?
        .into_iter()
        .filter_map(|(device, total)| device.map(|device| (device, total)))
        .collect();
    let links = links_query.all(db).await?;

    let interface_ids: BTreeSet<i64> = links
        .iter()
        .flat_map(|l| [l.source_interface_id, l.target_interface_id])
        .flatten()
        .collect();
    let interfaces = interfaces_by_ids(db, &interface_ids).await?;
    let traffic = latest_traffic(db, &interface_ids).await?;
    let site_names: BTreeMap<i64, String> = sites::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|site| (site.id, site.name))
        .collect();

    let lookup = EdgeLookup {
        devices: devices.iter().map(|d| (d.id, d)).collect(),
        interfaces: interfaces.iter().map(|i| (i.id, i)).collect(),
        traffic: &traffic,
        now: Utc::now(),
    };
    let mut edges: Vec<_> = links.into_iter().map(|l| lookup.edge(l)).collect();
    append_parent_edges(&mut edges, &devices, &ids);

    let connected: BTreeSet<i64> = edges
        .iter()
        .flat_map(|edge| [edge.source, edge.target])
        .collect();
    let nodes = devices
        .into_iter()
        // O NetMonitor entra no mapa de um site só se estiver ligado a ele.
        .filter(|device| {
            site_id.is_none() || device.site_id == site_id || connected.contains(&device.id)
        })
        .map(|device| TopologyNode {
            id: device.id,
            site_name: device
                .site_id
                .and_then(|site| site_names.get(&site).cloned()),
            interface_count: count(&interface_counts, device.id),
            monitors_down: count(&monitors_down, device.id),
            is_system: device.system_key.as_deref() == Some(NETMONITOR_KEY),
            name: device.name,
            device_type: device.r#type,
            status: device.status,
            site_id: device.site_id,
            ip_address: device.ip_address,
            vendor: device.vendor,
            model: device.model,
            snmp_enabled: device.snmp_enabled,
            parent_id: device.parent_id,
        })
        .collect();
    Ok(TopologyGraph { nodes, edges })
}

fn count(counts: &BTreeMap<i64, i64>, device_id: i64) -> usize {
    counts
        .get(&device_id)
        .and_then(|total| usize::try_from(*total).ok())
        .unwrap_or(0)
}

/// Ponta de um enlace real: a interface (se o enlace a conhece) e o dispositivo.
fn endpoint<'a>(
    iface: Option<&'a device_interfaces::Model>,
    device: Option<&'a devices::Model>,
) -> EndpointState<'a> {
    EndpointState {
        oper_status: iface.and_then(|i| i.oper_status.as_deref()),
        admin_status: iface.and_then(|i| i.admin_status.as_deref()),
        device_status: device.map(|d| d.status.as_str()),
    }
}

/// O que a montagem de uma aresta consulta, indexado por id.
struct EdgeLookup<'a> {
    devices: BTreeMap<i64, &'a devices::Model>,
    interfaces: BTreeMap<i64, &'a device_interfaces::Model>,
    traffic: &'a BTreeMap<i64, (Option<f64>, Option<f64>)>,
    now: chrono::DateTime<Utc>,
}

impl EdgeLookup<'_> {
    fn edge(&self, link: device_links::Model) -> TopologyEdge {
        let source_iface = link
            .source_interface_id
            .and_then(|id| self.interfaces.get(&id).copied());
        let target_iface = link
            .target_interface_id
            .and_then(|id| self.interfaces.get(&id).copied());
        let source_device = self.devices.get(&link.source_device_id).copied();
        let target_device = self.devices.get(&link.target_device_id).copied();

        let traffic_of = |id: Option<i64>| {
            id.and_then(|id| self.traffic.get(&id))
                .copied()
                .unwrap_or((None, None))
        };
        let (src_in, src_out) = traffic_of(link.source_interface_id);
        let (tgt_in, tgt_out) = traffic_of(link.target_interface_id);
        let in_bps = src_in.or(tgt_in);
        let out_bps = src_out.or(tgt_out);
        let total_bps = match (in_bps, out_bps) {
            (Some(i), Some(o)) => Some(i + o),
            (one, other) => one.or(other),
        };
        let source_speed = source_iface.and_then(|i| i.speed);
        let target_speed = target_iface.and_then(|i| i.speed);
        let speed_bps = edge_speed_bps(source_speed, target_speed);

        let status = edge_status(
            endpoint(source_iface, source_device),
            endpoint(target_iface, target_device),
        );
        let interface_status = |iface: Option<&device_interfaces::Model>| {
            iface.and_then(|i| i.oper_status.clone().or_else(|| i.admin_status.clone()))
        };

        TopologyEdge {
            id: link.id,
            source: link.source_device_id,
            target: link.target_device_id,
            source_device_id: link.source_device_id,
            target_device_id: link.target_device_id,
            source_device_name: source_device.map(|d| d.name.clone()),
            target_device_name: target_device.map(|d| d.name.clone()),
            source_interface_id: link.source_interface_id,
            target_interface_id: link.target_interface_id,
            source_interface_name: source_iface.map(|i| i.name.clone()),
            target_interface_name: target_iface.map(|i| i.name.clone()),
            source_interface_speed: source_speed,
            target_interface_speed: target_speed,
            source_interface_status: interface_status(source_iface),
            target_interface_status: interface_status(target_iface),
            in_bps,
            out_bps,
            traffic_bps: total_bps,
            traffic_label: total_bps.map(format_traffic_bps),
            speed_bps,
            utilization: edge_utilization(in_bps, out_bps, speed_bps),
            stale: is_stale(&link.discovery_method, link.last_seen_at.as_ref(), self.now),
            last_seen_at: link.last_seen_at.map(|seen| seen.to_rfc3339()),
            link_type: link.link_type,
            discovery_method: link.discovery_method,
            confidence: link.confidence,
            confirmed: link.confirmed,
            status: status.into(),
        }
    }
}

/// Recusa interface que não pertence ao dispositivo daquela ponta — sem isso a
/// tela desenharia a porta de um equipamento no enlace de outro.
async fn ensure_interface_belongs(
    db: &sea_orm::DatabaseConnection,
    interface_id: Option<i64>,
    device_id: i64,
) -> AppResult<()> {
    let Some(interface_id) = interface_id else {
        return Ok(());
    };
    let belongs = device_interfaces::Entity::find_by_id(interface_id)
        .one(db)
        .await?
        .is_some_and(|interface| interface.device_id == device_id);
    if belongs {
        Ok(())
    } else {
        Err(AppError::validation(
            "A interface escolhida não pertence ao dispositivo da ligação",
        ))
    }
}

/// Cria (ou promove a manual) o enlace entre dois dispositivos.
///
/// # Errors
///
/// Validação (mesmo dispositivo, interface de outro equipamento), dispositivo
/// inexistente ou erro do banco.
pub async fn create_manual_link(
    db: &sea_orm::DatabaseConnection,
    source: i64,
    target: i64,
    source_interface: Option<i64>,
    target_interface: Option<i64>,
    link_type: Option<String>,
) -> AppResult<device_links::Model> {
    if source == target {
        return Err(AppError::validation(
            "Uma ligação deve conectar dispositivos diferentes",
        ));
    }
    let found = devices::Entity::find()
        .filter(devices::Column::Id.is_in([source, target]))
        .count(db)
        .await?;
    if found < 2 {
        return Err(AppError::not_found("Dispositivo não encontrado"));
    }
    ensure_interface_belongs(db, source_interface, source).await?;
    ensure_interface_belongs(db, target_interface, target).await?;
    let result = persist_resolved_links_detailed(
        db,
        vec![NetworkLink {
            source_device_id: source,
            target_device_id: target,
            source_interface_id: source_interface,
            target_interface_id: target_interface,
            link_type: link_type.unwrap_or_else(|| "manual".into()),
            discovery_method: "user_defined".into(),
            confidence: 100,
            confirmed: true,
        }],
    )
    .await?;
    result
        .links
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Falha ao criar ligação")))
}

/// Edita as portas/tipo de um enlace. Editar é uma decisão do operador: o
/// enlace passa a valer como manual (100, confirmado), e a descoberta seguinte
/// não desfaz a escolha.
///
/// # Errors
///
/// Validação (aresta virtual, interface de outro equipamento), enlace
/// inexistente ou erro do banco.
pub async fn update_manual_link(
    db: &sea_orm::DatabaseConnection,
    link_id: i64,
    source_interface: Option<i64>,
    target_interface: Option<i64>,
    link_type: Option<String>,
) -> AppResult<device_links::Model> {
    if link_id <= 0 {
        return Err(AppError::validation(
            "Enlaces virtuais de hierarquia não podem ser editados diretamente",
        ));
    }
    let link = device_links::Entity::find_by_id(link_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Ligação não encontrada"))?;
    ensure_interface_belongs(db, source_interface, link.source_device_id).await?;
    ensure_interface_belongs(db, target_interface, link.target_device_id).await?;

    let mut active: device_links::ActiveModel = link.into();
    active.source_interface_id = sea_orm::Set(source_interface);
    active.target_interface_id = sea_orm::Set(target_interface);
    if let Some(t) = link_type {
        active.link_type = sea_orm::Set(t);
    }
    active.discovery_method = sea_orm::Set("user_defined".into());
    active.confidence = sea_orm::Set(100);
    active.confirmed = sea_orm::Set(true);
    active.updated_at = sea_orm::Set(Utc::now().into());
    let updated = active.update(db).await?;
    Ok(updated)
}

pub async fn create_unmanaged_switch(
    db: &sea_orm::DatabaseConnection,
    input: UnmanagedSwitchInput,
) -> AppResult<devices::Model> {
    if input.name.trim().is_empty() {
        return Err(AppError::validation("Nome do switch é obrigatório"));
    }
    let port_count = input.port_count.clamp(1, 128);
    let now = chrono::Utc::now();
    let device_model = devices::ActiveModel {
        name: sea_orm::Set(input.name.trim().to_string()),
        r#type: sea_orm::Set("unmanaged_switch".to_string()),
        vendor: sea_orm::Set(input.vendor.filter(|v| !v.trim().is_empty())),
        model: sea_orm::Set(input.model.filter(|m| !m.trim().is_empty())),
        site_id: sea_orm::Set(input.site_id),
        network_id: sea_orm::Set(input.network_id),
        is_monitored: sea_orm::Set(false),
        snmp_enabled: sea_orm::Set(false),
        snmp_poll_interval_seconds: sea_orm::Set(60),
        status: sea_orm::Set("online".to_string()),
        created_at: sea_orm::Set(now.into()),
        updated_at: sea_orm::Set(now.into()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    for i in 1..=port_count {
        let port_name = format!("Porta {i}");
        device_interfaces::ActiveModel {
            device_id: sea_orm::Set(device_model.id),
            snmp_index: sea_orm::Set(Some(i as i32)),
            name: sea_orm::Set(port_name),
            description: sea_orm::Set(Some(format!("Porta física #{i}"))),
            r#type: sea_orm::Set(Some("ethernetCsmacd".to_string())),
            speed: sea_orm::Set(Some(1_000_000_000)), // 1 Gbps
            admin_status: sea_orm::Set(Some("up".to_string())),
            oper_status: sea_orm::Set(Some("up".to_string())),
            created_at: sea_orm::Set(now.into()),
            updated_at: sea_orm::Set(now.into()),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }

    Ok(device_model)
}

/// Remove um enlace real ou, para id negativo, a hierarquia pai/filho que a
/// aresta virtual representa (o filho perde o `parent_id`).
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn delete_link(db: &sea_orm::DatabaseConnection, id: i64) -> AppResult<bool> {
    if let Some(child_id) = parent_edge_child(id) {
        let Some(child) = devices::Entity::find_by_id(child_id)
            .filter(devices::Column::ParentId.is_not_null())
            .one(db)
            .await?
        else {
            return Ok(false);
        };
        let mut active: devices::ActiveModel = child.into();
        active.parent_id = sea_orm::Set(None);
        active.updated_at = sea_orm::Set(Utc::now().into());
        active.update(db).await?;
        return Ok(true);
    }
    Ok(device_links::Entity::delete_by_id(id)
        .exec(db)
        .await?
        .rows_affected
        > 0)
}

/// Recalcula os enlaces inferidos por sub-rede (ver [`infer_uplinks`]).
///
/// Os inferidos não confirmados de rodadas anteriores que não saem mais da
/// regra são removidos — é o que limpa o N×M antigo. Enlace observado, manual
/// ou confirmado nunca é tocado. Devolve quantos enlaces inferidos existem
/// depois do recálculo.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn infer_subnet_links(db: &sea_orm::DatabaseConnection) -> AppResult<usize> {
    let txn = begin_write(db).await?;
    let devices = devices::Entity::find()
        .order_by_asc(devices::Column::Id)
        .all(&txn)
        .await?;
    let links = device_links::Entity::find().all(&txn).await?;
    let is_replaceable =
        |link: &device_links::Model| link.discovery_method == INFERRED_METHOD && !link.confirmed;
    let taken: BTreeSet<_> = links
        .iter()
        .filter(|link| !is_replaceable(link))
        .map(|link| pair_key(link.source_device_id, link.target_device_id))
        .collect();
    let inferred = infer_uplinks(&devices, &taken);
    let wanted: BTreeSet<_> = inferred
        .iter()
        .map(|link| pair_key(link.source_device_id, link.target_device_id))
        .collect();
    let obsolete: Vec<i64> = links
        .iter()
        .filter(|link| is_replaceable(link))
        .filter(|link| !wanted.contains(&pair_key(link.source_device_id, link.target_device_id)))
        .map(|link| link.id)
        .collect();
    for chunk in obsolete.chunks(IDS_PER_QUERY) {
        device_links::Entity::delete_many()
            .filter(device_links_entity::Column::Id.is_in(chunk.to_vec()))
            .exec(&txn)
            .await?;
    }
    let total = inferred.len();
    persist_links_in(&txn, inferred).await?;
    txn.commit().await?;
    Ok(total)
}

/// Arestas do grafo inteiro no formato de [`shortest_path`]: os enlaces de
/// `device_links` e as arestas virtuais de hierarquia que não duplicam um
/// enlace. Enlace confirmado conta com confiança 100.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn load_path_edges<C: ConnectionTrait>(db: &C) -> AppResult<Vec<PathEdge>> {
    let links = device_links::Entity::find().all(db).await?;
    let devices = devices::Entity::find().all(db).await?;
    let ids: BTreeSet<i64> = devices.iter().map(|device| device.id).collect();
    let linked: BTreeSet<_> = links
        .iter()
        .map(|link| pair_key(link.source_device_id, link.target_device_id))
        .collect();
    let mut edges: Vec<PathEdge> = links
        .iter()
        .map(|link| {
            let confidence = if link.confirmed {
                100
            } else {
                u8::try_from(link.confidence.clamp(0, 100)).unwrap_or(0)
            };
            (
                link.id,
                link.source_device_id,
                link.target_device_id,
                confidence,
            )
        })
        .collect();
    edges.extend(
        parent_pairs(&devices, &ids, &linked)
            .into_iter()
            .map(|(parent, child)| (parent_edge_id(child), parent, child, 100)),
    );
    Ok(edges)
}

/// Menor caminho entre dois dispositivos cadastrados.
///
/// # Errors
///
/// `NotFound` se algum dos dispositivos não existe; erro do banco.
pub async fn find_path(
    db: &sea_orm::DatabaseConnection,
    from: i64,
    to: i64,
) -> AppResult<Option<TopologyPath>> {
    let found = devices::Entity::find()
        .filter(devices::Column::Id.is_in([from, to]))
        .count(db)
        .await?;
    let expected = if from == to { 1 } else { 2 };
    if found < expected {
        return Err(AppError::not_found("Dispositivo não encontrado"));
    }
    Ok(shortest_path(&load_path_edges(db).await?, from, to))
}

/// Converte os vizinhos LLDP/CDP observados no poll SNMP em enlaces persistidos.
/// O nome e o endereço de gerência são usados apenas para relacionar dispositivos
/// já cadastrados; uma resposta externa nunca cria um dispositivo implicitamente.
pub async fn resolve_discovered_neighbors(
    ctx: &loco_rs::app::AppContext,
    source: &devices::Model,
    neighbors: &[LldpNeighbor],
) -> AppResult<Vec<device_links::Model>> {
    let candidates = devices::Entity::find().all(&ctx.db).await?;
    let source_interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces_entity::Column::DeviceId.eq(source.id))
        .all(&ctx.db)
        .await?;
    let mut raw = Vec::new();
    for neighbor in neighbors {
        let target = candidates.iter().find(|device| {
            device.id != source.id
                && (neighbor.remote_sys_name.as_ref().is_some_and(|name| {
                    device.name.eq_ignore_ascii_case(name)
                        || device
                            .ip_address
                            .as_ref()
                            .is_some_and(|ip| ip.eq_ignore_ascii_case(name))
                }) || neighbor
                    .remote_mgmt_address
                    .as_ref()
                    .is_some_and(|address| {
                        device.ip_address.as_ref().is_some_and(|ip| ip == address)
                    }))
        });
        let Some(target) = target else {
            continue;
        };
        let target_interfaces = device_interfaces::Entity::find()
            .filter(device_interfaces_entity::Column::DeviceId.eq(target.id))
            .all(&ctx.db)
            .await?;
        let source_interface_id = neighbor
            .local_port
            .parse::<i32>()
            .ok()
            .and_then(|index| {
                source_interfaces
                    .iter()
                    .find(|interface| interface.snmp_index == Some(index))
            })
            .map(|interface| interface.id);
        let target_interface_id = neighbor.remote_port.as_ref().and_then(|port| {
            target_interfaces
                .iter()
                .find(|interface| {
                    interface.name.eq_ignore_ascii_case(port)
                        || interface.alias.as_ref().is_some_and(|alias| alias == port)
                })
                .map(|interface| interface.id)
        });
        let (confidence, method) = if neighbor.protocol == "lldp" {
            (95, "lldp")
        } else {
            (90, "cdp")
        };
        raw.push(NetworkLink {
            source_device_id: source.id,
            target_device_id: target.id,
            source_interface_id,
            target_interface_id,
            link_type: neighbor.protocol.clone(),
            discovery_method: method.into(),
            confidence,
            confirmed: false,
        });
    }
    Ok(persist_resolved_links_detailed(&ctx.db, raw).await?.links)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispositivo(id: i64, parent_id: Option<i64>) -> devices::Model {
        devices::Model {
            id,
            site_id: None,
            network_id: None,
            parent_id,
            ip_address: None,
            name: format!("dev-{id}"),
            r#type: "switch".into(),
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
            syslog_server_address: None,
            system_key: None,
            link_interface_id: None,
            link_interface_name: None,
            firmware_version: None,
            observed_os: None,
            observed_os_source: None,
            observed_os_reason: None,
            status: "online".into(),
            last_seen_at: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    fn aresta_real(id: i64, source: i64, target: i64) -> TopologyEdge {
        TopologyEdge {
            id,
            source,
            target,
            source_device_id: source,
            target_device_id: target,
            link_type: "snmp".into(),
            discovery_method: "lldp".into(),
            confidence: 90,
            confirmed: true,
            status: "up".into(),
            ..TopologyEdge::default()
        }
    }

    #[test]
    fn parent_id_vira_aresta_virtual_com_id_negativo() {
        let devices = vec![dispositivo(1, None), dispositivo(7, Some(1))];
        let ids = BTreeSet::from([1, 7]);
        let mut edges = Vec::new();
        append_parent_edges(&mut edges, &devices, &ids);

        assert_eq!(edges.len(), 1);
        let aresta = &edges[0];
        // Matriz de paridade #48: id negativo distingue hierarquia de enlace real.
        assert_eq!(aresta.id, -7);
        assert_eq!(aresta.source, 1);
        assert_eq!(aresta.target, 7);
        assert_eq!(aresta.link_type, "parent");
        assert_eq!(aresta.discovery_method, "parent_hierarchy");
        assert_eq!(aresta.status, "up");
        assert!(!aresta.stale);
    }

    #[test]
    fn enlace_real_existente_dispensa_a_aresta_virtual() {
        let devices = vec![dispositivo(1, None), dispositivo(7, Some(1))];
        let ids = BTreeSet::from([1, 7]);

        // Nos dois sentidos: o enlace descoberto pode ter nascido de qualquer lado.
        for real in [aresta_real(50, 1, 7), aresta_real(50, 7, 1)] {
            let mut edges = vec![real];
            append_parent_edges(&mut edges, &devices, &ids);
            assert_eq!(edges.len(), 1, "aresta virtual duplicou um enlace real");
            assert_eq!(edges[0].id, 50);
        }
    }

    #[test]
    fn pai_fora_do_recorte_nao_vira_no_fantasma() {
        // O filho está no site pedido, o pai não. Desenhar a aresta criaria um
        // nó que a lista de `nodes` não contém.
        let devices = vec![dispositivo(7, Some(99))];
        let ids = BTreeSet::from([7]);
        let mut edges = Vec::new();
        append_parent_edges(&mut edges, &devices, &ids);
        assert!(edges.is_empty());
    }

    #[test]
    fn dispositivo_sem_pai_nao_gera_aresta() {
        let devices = vec![dispositivo(1, None)];
        let mut edges = Vec::new();
        append_parent_edges(&mut edges, &devices, &BTreeSet::from([1]));
        assert!(edges.is_empty());
    }

    #[test]
    fn aresta_virtual_tem_id_negativo_reversivel() {
        assert_eq!(parent_edge_id(3), -3);
        assert_eq!(parent_edge_child(parent_edge_id(3)), Some(3));
        assert_eq!(parent_edge_child(42), None);
        assert_eq!(parent_edge_child(0), None);
    }

    #[test]
    fn ids_de_hierarquia_nao_colidem_com_pai_acima_de_mil() {
        // Esquema antigo: -(1 * 1000 + 1001) == -(2 * 1000 + 1) == -2001.
        let devices = vec![
            dispositivo(1, Some(1001)),
            dispositivo(2, Some(1)),
            dispositivo(1001, None),
        ];
        let ids = BTreeSet::from([1, 2, 1001]);
        let mut edges = Vec::new();
        append_parent_edges(&mut edges, &devices, &ids);
        let edge_ids: BTreeSet<_> = edges.iter().map(|edge| edge.id).collect();
        assert_eq!(edge_ids, BTreeSet::from([-1, -2]));
    }

    #[test]
    fn aresta_virtual_cai_quando_uma_ponta_esta_offline() {
        let mut filho = dispositivo(7, Some(1));
        filho.status = "offline".into();
        let devices = vec![dispositivo(1, None), filho];
        let mut edges = Vec::new();
        append_parent_edges(&mut edges, &devices, &BTreeSet::from([1, 7]));
        assert_eq!(edges[0].status, "down");
    }
}
