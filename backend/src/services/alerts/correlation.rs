//! Correlação de eventos e causa raiz automática (Root Cause Analysis - RCA).
//!
//! Conforme a especificação 2.3.2 de `docs/evolucao_produto.md`:
//! - Quando múltiplos hosts caem simultaneamente, infere automaticamente se a
//!   causa é o roteador, o switch, o link ISP, o gateway, a VPN ou falha de site.
//! - Grafo de dependências construído a partir da topologia (`devices.parent_id`,
//!   `device_links` de LLDP/CDP/manuais e inferência de sub-redes).
//! - Síntese diagnóstica em linguagem natural:
//!   "17 dispositivos ficaram inacessíveis após `192.168.1.1` parar de responder — causa provável: Gateway da Rede"
//! - Cálculo de pontuação de confiança (0 a 100%), raio de impacto e cadeia de dependência.
//! - Suporte à análise pontual por alerta (`analyze`) e agrupamento global de clusters ativos (`analyze_active_clusters`).

use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::{
    models::{_entities::alert_events as alert_events_entity, alert_events, device_links, devices},
    services::{alerts::contracts::AlertStatus, shared::errors::AppResult},
    views::alerts::{serialize_event, AlertRelations, SerializedAlertEvent},
};

/// Janela temporal padrão em torno do `started_at` do evento alvo (em segundos).
pub const DEFAULT_WINDOW_SECONDS: i64 = 60;

/// Categorias diagnósticas de causa raiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalCategory {
    Gateway,
    Router,
    Switch,
    Firewall,
    VpnTunnel,
    IspLink,
    SiteOutage,
    IsolatedDevice,
}

impl CausalCategory {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gateway => "gateway",
            Self::Router => "router",
            Self::Switch => "switch",
            Self::Firewall => "firewall",
            Self::VpnTunnel => "vpn",
            Self::IspLink => "isp_link",
            Self::SiteOutage => "site_outage",
            Self::IsolatedDevice => "isolated_device",
        }
    }

    #[must_use]
    pub fn label_pt(&self) -> &'static str {
        match self {
            Self::Gateway => "Gateway da Rede",
            Self::Router => "Roteador Principal",
            Self::Switch => "Switch de Rede",
            Self::Firewall => "Firewall / Segurança",
            Self::VpnTunnel => "Túnel VPN / WireGuard",
            Self::IspLink => "Link ISP / Internet",
            Self::SiteOutage => "Falha Geral do Site / Alimentação",
            Self::IsolatedDevice => "Dispositivo Isolado",
        }
    }
}

/// Sumário de um nó na cadeia de dependência topológica até o alvo.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyNodeSummary {
    pub id: i64,
    pub name: String,
    pub ip_address: Option<String>,
    #[serde(rename = "type")]
    pub device_type: String,
    pub status: String,
    pub is_root_cause: bool,
    pub is_target: bool,
}

/// Sumário de um equipamento impactado pela causa raiz.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactedDeviceSummary {
    pub id: i64,
    pub name: String,
    pub ip_address: Option<String>,
    #[serde(rename = "type")]
    pub device_type: String,
    pub status: String,
    pub alert_id: Option<i64>,
    pub severity: Option<String>,
}

/// Resultado detalhado da análise de correlação e causa raiz de um evento.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertCorrelation {
    /// Largura da janela temporal usada na correlação.
    pub window_seconds: i64,
    /// Evento mais provável de ser a causa raiz comum.
    pub primary_cause: Option<SerializedAlertEvent>,
    /// Identificador canônico da categoria da causa raiz (`gateway`, `switch`, etc.).
    pub causal_category: String,
    /// Rótulo em português da categoria da causa raiz.
    pub causal_category_label: String,
    /// Nível de confiança na inferência diagnóstica (0 a 100%).
    pub confidence: i32,
    /// Explicação diagnóstica sintetizada em linguagem natural.
    pub explanation: String,
    /// Quantidade de dispositivos impactados em cascata.
    pub impacted_devices_count: usize,
    /// Lista dos dispositivos impactados pela falha.
    pub impacted_devices: Vec<ImpactedDeviceSummary>,
    /// Caminho no grafo de dependência da causa raiz até o dispositivo do alerta alvo.
    pub dependency_chain: Vec<DependencyNodeSummary>,
    /// Demais eventos abertos na mesma janela.
    pub related_events: Vec<SerializedAlertEvent>,
    /// Site compartilhado pelos eventos correlacionados, quando houver.
    pub common_site_id: Option<i64>,
    /// Rede compartilhada pelos eventos correlacionados, quando houver.
    pub common_network_id: Option<i64>,
    /// Quantos eventos foram encontrados na janela (inclui a causa raiz, mas não o alvo).
    pub correlation_count: usize,
}

/// Cluster de incidentes correlacionados ativos no sistema.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncidentCluster {
    pub id: String,
    pub root_cause_event: Option<SerializedAlertEvent>,
    pub root_cause_device_id: Option<i64>,
    pub root_cause_device_name: Option<String>,
    pub causal_category: String,
    pub causal_category_label: String,
    pub confidence: i32,
    pub explanation: String,
    pub impacted_devices_count: usize,
    pub total_alerts_count: usize,
    pub events: Vec<SerializedAlertEvent>,
    pub started_at: Option<String>,
    pub max_severity: String,
}

/// Sumário geral de incidentes e análise de causa raiz ativa.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootCauseAnalysisSummary {
    pub active_clusters: Vec<IncidentCluster>,
    pub total_active_incidents: usize,
    pub total_correlated_alerts: usize,
}

/// Grafo de dependência direcionado para inferência de causa raiz.
///
/// Uma aresta $u \to v$ indica que $v$ depende de $u$ (se $u$ falhar, $v$ perde
/// conectividade e falha em cascata).
#[derive(Debug, Default, Clone)]
pub struct DependencyGraph {
    /// Arestas de jusante: $u \to \{v_1, v_2, \dots\}$ (filhos/dependentes de $u$).
    pub downstream: HashMap<i64, HashSet<i64>>,
    /// Arestas de montante: $v \to \{u_1, u_2, \dots\}$ (ancestrais/provedores de $v$).
    pub upstream: HashMap<i64, HashSet<i64>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_dependency(&mut self, provider_id: i64, dependent_id: i64) {
        if provider_id == dependent_id {
            return;
        }
        self.downstream
            .entry(provider_id)
            .or_default()
            .insert(dependent_id);
        self.upstream
            .entry(dependent_id)
            .or_default()
            .insert(provider_id);
    }

    /// Retorna todos os descendentes alcançáveis a partir de `root_id` via BFS.
    pub fn reachable_descendants(&self, root_id: i64) -> HashSet<i64> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(root_id);

        while let Some(current) = queue.pop_front() {
            if let Some(children) = self.downstream.get(&current) {
                for &child in children {
                    if visited.insert(child) {
                        queue.push_back(child);
                    }
                }
            }
        }
        visited
    }

    /// Retorna todos os ancestrais alcançáveis a montante a partir de `target_id`.
    pub fn reachable_ancestors(&self, target_id: i64) -> HashSet<i64> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(target_id);

        while let Some(current) = queue.pop_front() {
            if let Some(parents) = self.upstream.get(&current) {
                for &parent in parents {
                    if visited.insert(parent) {
                        queue.push_back(parent);
                    }
                }
            }
        }
        visited
    }

    /// Encontra o caminho mais curto de `source_id` até `target_id` no grafo de dependência.
    pub fn shortest_path(&self, source_id: i64, target_id: i64) -> Option<Vec<i64>> {
        if source_id == target_id {
            return Some(vec![source_id]);
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent_map: HashMap<i64, i64> = HashMap::new();

        queue.push_back(source_id);
        visited.insert(source_id);

        let mut found = false;
        while let Some(current) = queue.pop_front() {
            if current == target_id {
                found = true;
                break;
            }
            if let Some(children) = self.downstream.get(&current) {
                for &child in children {
                    if visited.insert(child) {
                        parent_map.insert(child, current);
                        queue.push_back(child);
                    }
                }
            }
        }

        if !found {
            return None;
        }

        let mut path = Vec::new();
        let mut curr = target_id;
        path.push(curr);
        while let Some(&prev) = parent_map.get(&curr) {
            path.push(prev);
            curr = prev;
            if curr == source_id {
                break;
            }
        }
        path.reverse();
        Some(path)
    }
}

/// Constrói o grafo de dependência completo a partir dos dispositivos e enlaces do banco.
pub async fn build_dependency_graph<C: ConnectionTrait>(
    db: &C,
    devices_list: &[devices::Model],
) -> AppResult<DependencyGraph> {
    let mut graph = DependencyGraph::new();
    let devices_map: HashMap<i64, &devices::Model> =
        devices_list.iter().map(|d| (d.id, d)).collect();

    // 1. Hierarquia declarada (parent_id) - força máxima (100)
    for device in devices_list {
        if let Some(parent_id) = device.parent_id {
            if devices_map.contains_key(&parent_id) {
                graph.add_dependency(parent_id, device.id);
            }
        }
    }

    // 2. Enlaces topológicos descobertos ou manuais (device_links)
    let links = device_links::Entity::find().all(db).await?;
    for link in links {
        let (Some(source), Some(target)) = (
            devices_map.get(&link.source_device_id),
            devices_map.get(&link.target_device_id),
        ) else {
            continue;
        };

        let ws = role_weight(&source.r#type);
        let wt = role_weight(&target.r#type);

        if ws > wt {
            graph.add_dependency(source.id, target.id);
        } else if wt > ws {
            graph.add_dependency(target.id, source.id);
        } else if link.link_type == "parent" {
            graph.add_dependency(source.id, target.id);
        } else {
            // Em caso de enlace entre papéis semelhantes (ex.: switch -> switch), orienta se um for declarado
            graph.add_dependency(source.id, target.id);
        }
    }

    // 3. Inferência por sub-rede: conecta o gateway/router da sub-rede aos hosts sem pai declarado
    let mut subnets: HashMap<i64, Vec<&devices::Model>> = HashMap::new();
    for device in devices_list {
        if let Some(network_id) = device.network_id {
            subnets.entry(network_id).or_default().push(device);
        }
    }

    for (_net_id, members) in subnets {
        // Encontra o equipamento de maior hierarquia na sub-rede (gateway/router)
        let primary_router_id = members
            .iter()
            .filter(|d| matches!(d.r#type.as_str(), "router" | "gateway" | "firewall"))
            .max_by_key(|d| role_weight(&d.r#type))
            .map(|d| d.id);

        if let Some(router_id) = primary_router_id {
            for member in members {
                if member.id != router_id && member.parent_id.is_none() {
                    graph.add_dependency(router_id, member.id);
                }
            }
        }
    }

    Ok(graph)
}

/// Peso de prioridade topológica por tipo de equipamento.
#[must_use]
pub fn role_weight(device_type: &str) -> i32 {
    match device_type.to_lowercase().as_str() {
        "gateway" => 100,
        "router" => 90,
        "firewall" => 85,
        "switch" => 80,
        "access_point" | "ap" => 60,
        "vpn_peer" | "vpn" => 50,
        "server" => 40,
        _ => 20,
    }
}

/// Infere a categoria causal provável com base no tipo, nome e escopo do equipamento.
#[must_use]
pub fn infer_causal_category(
    device: Option<&devices::Model>,
    impacted_count: usize,
    total_site_devices: usize,
) -> CausalCategory {
    let Some(dev) = device else {
        return CausalCategory::IsolatedDevice;
    };

    let name_lower = dev.name.to_lowercase();
    let type_lower = dev.r#type.to_lowercase();

    let words: Vec<&str> = name_lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    let has_word = |w: &str| words.contains(&w);

    if type_lower == "gateway" || name_lower.contains("gateway") || has_word("gw") {
        return CausalCategory::Gateway;
    }

    if type_lower == "router" || name_lower.contains("router") || name_lower.contains("roteador") {
        return CausalCategory::Router;
    }

    if type_lower == "switch"
        || name_lower.contains("switch")
        || name_lower.starts_with("sw-")
        || name_lower.starts_with("sw_")
    {
        return CausalCategory::Switch;
    }

    if type_lower == "firewall"
        || name_lower.contains("firewall")
        || name_lower.contains("pfsense")
        || name_lower.contains("opnsense")
        || name_lower.contains("fortigate")
    {
        return CausalCategory::Firewall;
    }

    if type_lower == "vpn_peer"
        || type_lower == "vpn"
        || has_word("vpn")
        || name_lower.contains("wireguard")
        || name_lower.contains("tunnel")
    {
        return CausalCategory::VpnTunnel;
    }

    if has_word("isp")
        || name_lower.contains("link")
        || name_lower.contains("fibra")
        || has_word("wan")
        || name_lower.contains("internet")
    {
        return CausalCategory::IspLink;
    }

    // Se uma fração maciça do site caiu simultaneamente sem um nó de rede específico
    if total_site_devices >= 4 && impacted_count >= (total_site_devices * 3 / 4) {
        return CausalCategory::SiteOutage;
    }

    if impacted_count > 0 {
        match type_lower.as_str() {
            "router" => CausalCategory::Router,
            "switch" => CausalCategory::Switch,
            "firewall" => CausalCategory::Firewall,
            _ => CausalCategory::Gateway,
        }
    } else {
        CausalCategory::IsolatedDevice
    }
}

/// Sintetiza a explicação diagnóstica em linguagem natural conforme a especificação.
#[must_use]
pub fn synthesize_explanation(
    primary_device: Option<&devices::Model>,
    category: CausalCategory,
    impacted_count: usize,
) -> String {
    let Some(dev) = primary_device else {
        return "Nenhum alerta correlacionado encontrado na janela temporal.".into();
    };

    let target_ident = if let Some(ref ip) = dev.ip_address {
        if dev.name.is_empty() || dev.name.eq_ignore_ascii_case(ip) {
            format!("`{ip}`")
        } else {
            format!("`{ip}` ({})", dev.name)
        }
    } else {
        format!("'{}'", dev.name)
    };

    if impacted_count == 0 {
        return format!(
            "Alerta isolado em '{name}' sem impacto em cascata detectado.",
            name = dev.name
        );
    }

    let plural_dispositivos = if impacted_count == 1 {
        "1 dispositivo ficou inacessível".to_string()
    } else {
        format!("{impacted_count} dispositivos ficaram inacessíveis")
    };

    format!(
        "{plural_dispositivos} após {target_ident} parar de responder — causa provável: {category_label}.",
        category_label = category.label_pt()
    )
}

/// Calcula a pontuação de confiança (0 a 100%) na inferência diagnóstica.
#[must_use]
pub fn calculate_confidence(
    primary_device: Option<&devices::Model>,
    impacted_count: usize,
    graph: &DependencyGraph,
    failing_device_ids: &HashSet<i64>,
) -> i32 {
    let Some(dev) = primary_device else {
        return 0;
    };

    if impacted_count == 0 {
        return 100;
    }

    let descendants = graph.reachable_descendants(dev.id);
    let explained_failing = descendants
        .intersection(failing_device_ids)
        .copied()
        .count();

    if explained_failing > 0 {
        // Se explica descendentes diretos por hierarquia declarada
        let is_declared_parent = failing_device_ids.iter().any(|&child_id| {
            graph
                .downstream
                .get(&dev.id)
                .is_some_and(|s| s.contains(&child_id))
        });

        if is_declared_parent {
            let base = 88;
            (base + (explained_failing as i32 * 2)).min(99)
        } else {
            let base = 75;
            (base + (explained_failing as i32 * 2)).min(95)
        }
    } else {
        55
    }
}

/// Avalia e seleciona o evento de causa raiz provável entre os candidatos.
pub fn select_primary_cause<'a>(
    candidates: &'a [alert_events::Model],
    devices_map: &HashMap<i64, devices::Model>,
    graph: &DependencyGraph,
) -> Option<&'a alert_events::Model> {
    if candidates.is_empty() {
        return None;
    }

    let candidate_device_ids: HashSet<i64> = candidates
        .iter()
        .filter_map(|event| event.device_id)
        .collect();

    let earliest_time = candidates
        .iter()
        .map(|e| e.started_at.with_timezone(&Utc))
        .min();

    let mut scored_candidates: Vec<(&'a alert_events::Model, i32)> = candidates
        .iter()
        .map(|event| {
            let Some(device_id) = event.device_id else {
                return (event, -100);
            };
            let dev = devices_map.get(&device_id);
            let role_w = dev.map_or(20, |d| role_weight(&d.r#type));

            // Descendentes alcançáveis que também estão falhando
            let descendants = graph.reachable_descendants(device_id);
            let failing_descendants_count = descendants
                .intersection(&candidate_device_ids)
                .filter(|&&id| id != device_id)
                .count() as i32;

            // Ancestrais alcançáveis que também estão falhando (se houver, este nó é consequência)
            let ancestors = graph.reachable_ancestors(device_id);
            let failing_ancestors_count = ancestors
                .intersection(&candidate_device_ids)
                .filter(|&&id| id != device_id)
                .count() as i32;

            // Precedência temporal
            let is_earliest = earliest_time.is_some_and(|earliest| {
                let diff = (event.started_at.with_timezone(&Utc) - earliest).num_seconds();
                diff <= 5
            });

            // Bônus se for pai declarado direto de outros falhando
            let is_direct_parent = candidate_device_ids.iter().any(|&cid| {
                cid != device_id
                    && devices_map.get(&cid).and_then(|d| d.parent_id) == Some(device_id)
            });

            let mut score = 0;
            score += failing_descendants_count * 35;
            score += role_w;
            if is_earliest {
                score += 40;
            }
            if is_direct_parent {
                score += 30;
            }
            // Penalidade por ter ancestral falhando acima
            score -= failing_ancestors_count * 80;

            (event, score)
        })
        .collect();

    scored_candidates.sort_by(|(event_a, score_a), (event_b, score_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| {
                event_a
                    .started_at
                    .with_timezone(&Utc)
                    .cmp(&event_b.started_at.with_timezone(&Utc))
            })
            .then_with(|| event_a.id.cmp(&event_b.id))
    });

    scored_candidates.into_iter().next().map(|(event, _)| event)
}

/// Analisa a correlação temporal e topológica de um evento de alerta.
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

    let all_devices = devices::Entity::find().all(db).await?;
    let devices_map: HashMap<i64, devices::Model> =
        all_devices.into_iter().map(|d| (d.id, d)).collect();

    let target_device = event.device_id.and_then(|id| devices_map.get(&id));

    let from = event.started_at.with_timezone(&Utc) - Duration::seconds(window);
    let to = event.started_at.with_timezone(&Utc) + Duration::seconds(window);

    let candidates: Vec<alert_events::Model> = alert_events::Entity::find()
        .filter(alert_events_entity::Column::Id.ne(event_id))
        .filter(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN))
        .filter(alert_events_entity::Column::StartedAt.gte(from))
        .filter(alert_events_entity::Column::StartedAt.lte(to))
        .all(db)
        .await?;

    let all_devices_vec: Vec<devices::Model> = devices_map.values().cloned().collect();
    let graph = build_dependency_graph(db, &all_devices_vec).await?;

    let target_node_summary = target_device.map(|d| DependencyNodeSummary {
        id: d.id,
        name: d.name.clone(),
        ip_address: d.ip_address.clone(),
        device_type: d.r#type.clone(),
        status: d.status.clone(),
        is_root_cause: false,
        is_target: true,
    });

    if candidates.is_empty() {
        let category = infer_causal_category(target_device, 0, 1);
        let explanation = synthesize_explanation(target_device, category, 0);

        let dependency_chain = if let Some(target) = target_node_summary {
            vec![DependencyNodeSummary {
                is_root_cause: true,
                ..target
            }]
        } else {
            Vec::new()
        };

        return Ok(AlertCorrelation {
            window_seconds: window,
            primary_cause: None,
            causal_category: category.as_str().into(),
            causal_category_label: category.label_pt().into(),
            confidence: 100,
            explanation,
            impacted_devices_count: 0,
            impacted_devices: Vec::new(),
            dependency_chain,
            related_events: Vec::new(),
            common_site_id: target_device.and_then(|d| d.site_id),
            common_network_id: target_device.and_then(|d| d.network_id),
            correlation_count: 0,
        });
    }

    // Considera todos os eventos participantes da janela incluindo o evento alvo
    let mut all_incident_events = candidates.clone();
    all_incident_events.push(event.clone());

    let primary = select_primary_cause(&all_incident_events, &devices_map, &graph);
    let primary_device_id = primary.and_then(|e| e.device_id);
    let primary_device = primary_device_id.and_then(|id| devices_map.get(&id));

    let failing_device_ids: HashSet<i64> = all_incident_events
        .iter()
        .filter_map(|e| e.device_id)
        .collect();

    // Descendentes impactados pelo primary cause
    let impacted_device_ids: HashSet<i64> = if let Some(p_dev_id) = primary_device_id {
        graph
            .reachable_descendants(p_dev_id)
            .intersection(&failing_device_ids)
            .copied()
            .collect()
    } else {
        HashSet::new()
    };

    let total_site_devices = if let Some(site_id) = target_device.and_then(|d| d.site_id) {
        devices_map
            .values()
            .filter(|d| d.site_id == Some(site_id))
            .count()
    } else {
        devices_map.len()
    };

    let impacted_count = if impacted_device_ids.is_empty() {
        candidates.len()
    } else {
        impacted_device_ids.len()
    };

    let category = infer_causal_category(primary_device, impacted_count, total_site_devices);
    let confidence =
        calculate_confidence(primary_device, impacted_count, &graph, &failing_device_ids);
    let explanation = synthesize_explanation(primary_device, category, impacted_count);

    // Constrói lista de dispositivos impactados
    let mut impacted_devices = Vec::new();
    for dev_id in &failing_device_ids {
        if primary_device_id == Some(*dev_id) {
            continue;
        }
        if let Some(dev) = devices_map.get(dev_id) {
            let related_event = all_incident_events
                .iter()
                .find(|e| e.device_id == Some(*dev_id));
            impacted_devices.push(ImpactedDeviceSummary {
                id: dev.id,
                name: dev.name.clone(),
                ip_address: dev.ip_address.clone(),
                device_type: dev.r#type.clone(),
                status: dev.status.clone(),
                alert_id: related_event.map(|e| e.id),
                severity: related_event.map(|e| e.severity.clone()),
            });
        }
    }
    impacted_devices.sort_by(|a, b| a.name.cmp(&b.name));

    // Constrói cadeia de dependência visual do root cause até o target
    let mut dependency_chain = Vec::new();
    if let (Some(root_id), Some(target_id)) = (primary_device_id, event.device_id) {
        if let Some(path) = graph.shortest_path(root_id, target_id) {
            for node_id in path {
                if let Some(dev) = devices_map.get(&node_id) {
                    dependency_chain.push(DependencyNodeSummary {
                        id: dev.id,
                        name: dev.name.clone(),
                        ip_address: dev.ip_address.clone(),
                        device_type: dev.r#type.clone(),
                        status: dev.status.clone(),
                        is_root_cause: node_id == root_id,
                        is_target: node_id == target_id,
                    });
                }
            }
        }
    }

    if dependency_chain.is_empty() {
        if let Some(root_dev) = primary_device {
            dependency_chain.push(DependencyNodeSummary {
                id: root_dev.id,
                name: root_dev.name.clone(),
                ip_address: root_dev.ip_address.clone(),
                device_type: root_dev.r#type.clone(),
                status: root_dev.status.clone(),
                is_root_cause: true,
                is_target: primary_device_id == event.device_id,
            });
        }
        if primary_device_id != event.device_id {
            if let Some(target) = target_node_summary {
                dependency_chain.push(target);
            }
        }
    }

    let (common_site_id, common_network_id) =
        common_scopes(target_device, &candidates, &devices_map);

    let relations = AlertRelations::load(db, &candidates).await?;
    let mut serialized: Vec<SerializedAlertEvent> = candidates
        .iter()
        .map(|e| serialize_event(e, &relations))
        .collect();

    let primary_serialized = primary.map(|e| serialize_event(e, &relations));

    if let Some(ref primary) = primary_serialized {
        serialized.retain(|e| e.id != primary.id);
    }

    Ok(AlertCorrelation {
        window_seconds: window,
        primary_cause: primary_serialized,
        causal_category: category.as_str().into(),
        causal_category_label: category.label_pt().into(),
        confidence,
        explanation,
        impacted_devices_count: impacted_count,
        impacted_devices,
        dependency_chain,
        related_events: serialized,
        common_site_id,
        common_network_id,
        correlation_count: candidates.len(),
    })
}

/// Analisa todos os alertas abertos e agrupa em clusters de incidentes com análise de causa raiz.
pub async fn analyze_active_clusters<C: ConnectionTrait>(
    db: &C,
    window_seconds: Option<i64>,
) -> AppResult<RootCauseAnalysisSummary> {
    let window = window_seconds.unwrap_or(DEFAULT_WINDOW_SECONDS).max(1);

    let open_events = alert_events::Entity::find()
        .filter(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN))
        .all(db)
        .await?;

    if open_events.is_empty() {
        return Ok(RootCauseAnalysisSummary {
            active_clusters: Vec::new(),
            total_active_incidents: 0,
            total_correlated_alerts: 0,
        });
    }

    let all_devices = devices::Entity::find().all(db).await?;
    let devices_map: HashMap<i64, devices::Model> =
        all_devices.into_iter().map(|d| (d.id, d)).collect();
    let all_devices_vec: Vec<devices::Model> = devices_map.values().cloned().collect();
    let graph = build_dependency_graph(db, &all_devices_vec).await?;

    let relations = AlertRelations::load(db, &open_events).await?;

    // Agrupamento de eventos em clusters por conectividade de grafo ou proximidade temporal no mesmo site/rede
    let mut clusters_events: Vec<Vec<alert_events::Model>> = Vec::new();
    let mut assigned: HashSet<i64> = HashSet::new();

    for event in &open_events {
        if assigned.contains(&event.id) {
            continue;
        }

        let mut cluster = vec![event.clone()];
        assigned.insert(event.id);

        let event_dev = event.device_id.and_then(|id| devices_map.get(&id));
        let event_time = event.started_at.with_timezone(&Utc);

        for other in &open_events {
            if assigned.contains(&other.id) {
                continue;
            }

            let other_time = other.started_at.with_timezone(&Utc);
            let time_diff = (event_time - other_time).num_seconds().abs();

            let are_connected = match (event.device_id, other.device_id) {
                (Some(id_a), Some(id_b)) => {
                    id_a == id_b
                        || graph.reachable_descendants(id_a).contains(&id_b)
                        || graph.reachable_descendants(id_b).contains(&id_a)
                }
                _ => false,
            };

            let other_dev = other.device_id.and_then(|id| devices_map.get(&id));
            let same_scope = match (event_dev, other_dev) {
                (Some(da), Some(db)) => {
                    (da.site_id.is_some() && da.site_id == db.site_id)
                        || (da.network_id.is_some() && da.network_id == db.network_id)
                }
                _ => false,
            };

            if are_connected || (same_scope && time_diff <= window) {
                cluster.push(other.clone());
                assigned.insert(other.id);
            }
        }

        clusters_events.push(cluster);
    }

    let mut active_clusters = Vec::new();
    let mut total_correlated = 0;

    for (idx, cluster) in clusters_events.into_iter().enumerate() {
        let primary = select_primary_cause(&cluster, &devices_map, &graph);
        let primary_device_id = primary.and_then(|e| e.device_id);
        let primary_device = primary_device_id.and_then(|id| devices_map.get(&id));

        let failing_device_ids: HashSet<i64> = cluster.iter().filter_map(|e| e.device_id).collect();

        let impacted_count = cluster.len().saturating_sub(1);
        if impacted_count > 0 {
            total_correlated += cluster.len();
        }

        let total_site_devices = if let Some(site_id) = primary_device.and_then(|d| d.site_id) {
            devices_map
                .values()
                .filter(|d| d.site_id == Some(site_id))
                .count()
        } else {
            devices_map.len()
        };

        let category = infer_causal_category(primary_device, impacted_count, total_site_devices);
        let confidence =
            calculate_confidence(primary_device, impacted_count, &graph, &failing_device_ids);
        let explanation = synthesize_explanation(primary_device, category, impacted_count);

        let max_severity = cluster
            .iter()
            .map(|e| e.severity.as_str())
            .max_by_key(|s| match *s {
                "critical" => 4,
                "error" => 3,
                "warning" => 2,
                _ => 1,
            })
            .unwrap_or("info")
            .to_string();

        let earliest_start = cluster
            .iter()
            .map(|e| e.started_at)
            .min()
            .map(|dt| dt.to_rfc3339());

        let serialized_events: Vec<SerializedAlertEvent> = cluster
            .iter()
            .map(|e| serialize_event(e, &relations))
            .collect();

        let primary_serialized = primary.map(|e| serialize_event(e, &relations));

        active_clusters.push(IncidentCluster {
            id: format!("cluster-{}", idx + 1),
            root_cause_event: primary_serialized,
            root_cause_device_id: primary_device_id,
            root_cause_device_name: primary_device.map(|d| d.name.clone()),
            causal_category: category.as_str().into(),
            causal_category_label: category.label_pt().into(),
            confidence,
            explanation,
            impacted_devices_count: impacted_count,
            total_alerts_count: cluster.len(),
            events: serialized_events,
            started_at: earliest_start,
            max_severity,
        });
    }

    // Ordena clusters por quantidade de alertas e severidade
    active_clusters.sort_by(|a, b| {
        b.total_alerts_count
            .cmp(&a.total_alerts_count)
            .then_with(|| b.confidence.cmp(&a.confidence))
    });

    let total_active_incidents = active_clusters.len();

    Ok(RootCauseAnalysisSummary {
        active_clusters,
        total_active_incidents,
        total_correlated_alerts: total_correlated,
    })
}

/// Site e rede comuns entre o evento alvo e os candidatos.
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, FixedOffset, Utc};

    fn make_device(
        id: i64,
        parent_id: Option<i64>,
        site_id: Option<i64>,
        network_id: Option<i64>,
        device_type: &str,
        name: &str,
        ip: Option<&str>,
    ) -> devices::Model {
        devices::Model {
            id,
            site_id,
            network_id,
            parent_id,
            ip_address: ip.map(Into::into),
            name: name.into(),
            r#type: device_type.into(),
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
            status: "offline".into(),
            last_seen_at: None,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    fn make_event(
        id: i64,
        device_id: Option<i64>,
        seconds_ago: i64,
        severity: &str,
    ) -> alert_events::Model {
        let now: DateTime<FixedOffset> = Utc::now().into();
        alert_events::Model {
            id,
            alert_rule_id: Some(1),
            device_id,
            monitor_id: None,
            scope_key: Some(format!("device:{id}")),
            status: AlertStatus::Active.as_str().into(),
            severity: severity.into(),
            started_at: (now - Duration::seconds(seconds_ago)),
            resolved_at: None,
            message: Some("down".into()),
            data: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn causa_raiz_identifica_gateway_e_sintetiza_explicacao_exata() {
        let mut devices_map = HashMap::new();
        let mut graph = DependencyGraph::new();

        let gw = make_device(
            1,
            None,
            Some(1),
            Some(1),
            "router",
            "Gateway Principal",
            Some("192.168.1.1"),
        );
        devices_map.insert(1, gw);

        let mut candidates = vec![make_event(100, Some(1), 10, "critical")];

        // 17 dispositivos dependentes do gateway
        for i in 2..=18 {
            let dev = make_device(
                i,
                Some(1),
                Some(1),
                Some(1),
                "server",
                &format!("Host-{i}"),
                Some(&format!("192.168.1.{i}")),
            );
            devices_map.insert(i, dev);
            graph.add_dependency(1, i);
            candidates.push(make_event(100 + i, Some(i), 5, "critical"));
        }

        let primary = select_primary_cause(&candidates, &devices_map, &graph);
        assert_eq!(primary.map(|e| e.id), Some(100));

        let category = infer_causal_category(devices_map.get(&1), 17, 18);
        assert_eq!(category, CausalCategory::Gateway);

        let explanation = synthesize_explanation(devices_map.get(&1), category, 17);
        assert!(explanation.contains("17 dispositivos ficaram inacessíveis após `192.168.1.1` (Gateway Principal) parar de responder — causa provável: Gateway da Rede"));
    }

    #[test]
    fn cascata_de_tres_niveis_gateway_switch_servidor_aponta_gateway() {
        let mut devices_map = HashMap::new();
        let mut graph = DependencyGraph::new();

        let gw = make_device(
            1,
            None,
            Some(1),
            Some(1),
            "router",
            "Gateway",
            Some("10.0.0.1"),
        );
        let sw = make_device(
            2,
            Some(1),
            Some(1),
            Some(1),
            "switch",
            "Switch Core",
            Some("10.0.0.2"),
        );
        let srv = make_device(
            3,
            Some(2),
            Some(1),
            Some(1),
            "server",
            "Servidor DB",
            Some("10.0.0.10"),
        );

        devices_map.insert(1, gw);
        devices_map.insert(2, sw);
        devices_map.insert(3, srv);

        graph.add_dependency(1, 2);
        graph.add_dependency(2, 3);

        let candidates = vec![
            make_event(10, Some(1), 30, "critical"), // Gateway caiu há 30s
            make_event(20, Some(2), 25, "critical"), // Switch caiu há 25s
            make_event(30, Some(3), 20, "critical"), // DB caiu há 20s
        ];

        let primary = select_primary_cause(&candidates, &devices_map, &graph);
        assert_eq!(primary.map(|e| e.id), Some(10));

        let path = graph.shortest_path(1, 3).expect("path exists");
        assert_eq!(path, vec![1, 2, 3]);
    }

    #[test]
    fn dispositivo_isolado_possui_confianca_100() {
        let dev = make_device(
            5,
            None,
            Some(1),
            Some(1),
            "server",
            "NVR Câmeras",
            Some("192.168.1.50"),
        );
        let category = infer_causal_category(Some(&dev), 0, 10);
        assert_eq!(category, CausalCategory::IsolatedDevice);

        let explanation = synthesize_explanation(Some(&dev), category, 0);
        assert_eq!(
            explanation,
            "Alerta isolado em 'NVR Câmeras' sem impacto em cascata detectado."
        );
    }
}
