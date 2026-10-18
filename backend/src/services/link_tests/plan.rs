//! O plano antes de rodar (ADR 015 §5): as pernas, o motor de cada uma, o que
//! será instalado, o que falta e quanto tempo leva.
//!
//! O mesmo plano é refeito no `POST /api/link-tests`: o que roda é sempre o
//! estado do momento da confirmação, nunca um plano antigo guardado na tela.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use super::{
    endpoint::{self, Endpoint, Role},
    env::LinkTestEnv,
    guard,
    model::{
        LinkTestDirection, LinkTestEndpointKind, LinkTestFix, LinkTestInstall, LinkTestKind,
        LinkTestLegKind, LinkTestPlan, LinkTestPlanLeg, LinkTestRequest, LinkTestSide,
        LinkTestSpec,
    },
    negotiate::{negotiate, Capabilities, Negotiation, Party},
    transparent::{self, Hop},
};
use crate::{
    models::{device_links, devices},
    services::{
        devices::system_device::{SystemDeviceService, NETMONITOR_KEY},
        network_tools::throughput::ThroughputEngine,
        shared::errors::{AppError, AppResult},
        topology::{
            path::{shortest_path, TopologyPath},
            service::{find_path, load_path_edges, parent_edge_child},
        },
    },
};

/// Folga fixa de uma perna (conexões, relatórios), em segundos.
const LEG_OVERHEAD_S: u32 = 4;
/// Folga de um `prepare` (pode instalar pacote).
const PREPARE_S: u32 = 20;
/// Folga de um `prepare` que só confere (`prepareChecksOnly`).
const CHECK_S: u32 = 2;
/// Folga de `serve` + `stop` num equipamento.
const SERVE_STOP_S: u32 = 8;
/// Folga de cada medida (abertura e relatório).
const MEASURE_OVERHEAD_S: u32 = 3;

/// Uma perna do plano, com o que a execução precisa.
#[derive(Debug, Clone)]
pub struct PlannedLeg {
    pub view: LinkTestPlanLeg,
    pub a: Endpoint,
    pub b: Endpoint,
    pub negotiation: Option<Negotiation>,
}

impl PlannedLeg {
    #[must_use]
    pub const fn endpoint(&self, side: LinkTestSide) -> &Endpoint {
        match side {
            LinkTestSide::A => &self.a,
            LinkTestSide::B => &self.b,
        }
    }
}

/// O plano montado.
#[derive(Debug, Clone)]
pub struct BuiltPlan {
    pub view: LinkTestPlan,
    pub legs: Vec<PlannedLeg>,
    pub spec: LinkTestSpec,
    pub request: LinkTestRequest,
}

impl BuiltPlan {
    /// Os equipamentos que o teste ocupa (a central e os agentes não entram).
    #[must_use]
    pub fn device_ids(&self) -> BTreeSet<i64> {
        self.legs
            .iter()
            .filter(|leg| leg.view.runnable)
            .flat_map(|leg| [leg.a.device_id(), leg.b.device_id()])
            .flatten()
            .collect()
    }

    /// Nome de um equipamento do plano, para as mensagens.
    #[must_use]
    pub fn device_name(&self, id: i64) -> String {
        self.legs
            .iter()
            .flat_map(|leg| [&leg.a, &leg.b])
            .find(|endpoint| endpoint.device_id() == Some(id))
            .map_or_else(
                || format!("O dispositivo #{id}"),
                |endpoint| endpoint.view.name.clone(),
            )
    }
}

/// Pontas já carregadas, para não repetir consultas num caminho longo.
struct Endpoints<'a> {
    env: &'a LinkTestEnv,
    cache: HashMap<i64, Endpoint>,
}

impl Endpoints<'_> {
    async fn device(&mut self, id: i64) -> AppResult<Endpoint> {
        if let Some(found) = self.cache.get(&id) {
            return Ok(found.clone());
        }
        let device = find_device(self.env, id).await?;
        // Sem IP não mede: o teste passa por ele (ver `transparent`).
        if transparent::is_transparent(&device) {
            return Err(transparent::not_an_endpoint(&device.name));
        }
        // O próprio servidor nunca é "um equipamento com plugin": em qualquer
        // tipo de teste ele é a central, com os motores nativos.
        let built = if device.system_key.as_deref() == Some(NETMONITOR_KEY) {
            endpoint::central_endpoint(Some(device))
        } else {
            endpoint::device_endpoint(&self.env.db, device).await?
        };
        self.cache.insert(id, built.clone());
        Ok(built)
    }
}

async fn find_device(env: &LinkTestEnv, id: i64) -> AppResult<devices::Model> {
    devices::Entity::find_by_id(id)
        .one(&env.db)
        .await?
        .ok_or_else(|| AppError::not_found(format!("Dispositivo #{id} não encontrado")))
}

/// As duas pontas de uma aresta do grafo: enlace (`id > 0`) ou hierarquia
/// (`id < 0`, pai e filho).
async fn edge_pair(env: &LinkTestEnv, edge_id: i64) -> AppResult<(i64, i64)> {
    if let Some(child) = parent_edge_child(edge_id) {
        let child = find_device(env, child).await?;
        let parent = child
            .parent_id
            .ok_or_else(|| AppError::not_found("Enlace não encontrado"))?;
        return Ok((parent, child.id));
    }
    let link = device_links::Entity::find_by_id(edge_id)
        .one(&env.db)
        .await?
        .ok_or_else(|| AppError::not_found("Enlace não encontrado"))?;
    Ok((link.source_device_id, link.target_device_id))
}

/// Por onde a perna passa: as arestas, em ordem de A a B, e os equipamentos
/// sem IP do meio.
#[derive(Debug, Clone, Default)]
struct Route {
    edge_ids: Vec<i64>,
    via_device_ids: Vec<i64>,
    via_names: Vec<String>,
}

impl Route {
    fn edge(edge_id: i64) -> Self {
        Self {
            edge_ids: vec![edge_id],
            ..Self::default()
        }
    }

    fn of(hop: &Hop, names: &HashMap<i64, String>) -> Self {
        Self {
            edge_ids: hop.edge_ids.clone(),
            via_device_ids: hop.via_device_ids.clone(),
            via_names: hop
                .via_device_ids
                .iter()
                .map(|id| names.get(id).cloned().unwrap_or_else(|| format!("#{id}")))
                .collect(),
        }
    }
}

/// Perna ainda sem avaliação.
struct RawLeg {
    kind: LinkTestLegKind,
    a: Endpoint,
    b: Endpoint,
    route: Route,
}

/// Os equipamentos sem IP dentre `ids`, com o nome.
async fn transparent_names(env: &LinkTestEnv, ids: &[i64]) -> AppResult<HashMap<i64, String>> {
    Ok(devices::Entity::find()
        .filter(devices::Column::Id.is_in(ids.iter().copied()))
        .all(&env.db)
        .await?
        .into_iter()
        .filter(transparent::is_transparent)
        .map(|device| (device.id, device.name))
        .collect())
}

/// O caminho em pernas entre equipamentos com IP: cada sequência de
/// equipamentos sem IP vira uma perna só. `origin` (a central, o agente)
/// nunca é transparente.
async fn hops_of(
    env: &LinkTestEnv,
    path: &TopologyPath,
    origin: Option<i64>,
) -> AppResult<(Vec<Hop>, HashMap<i64, String>)> {
    let names = transparent_names(env, &path.device_ids).await?;
    let hops = transparent::collapse(&path.device_ids, &path.edge_ids, |id| {
        Some(id) != origin && names.contains_key(&id)
    })
    .map_err(|id| {
        transparent::not_an_endpoint(names.get(&id).map_or("O equipamento", String::as_str))
    })?;
    Ok((hops, names))
}

struct Draft {
    legs: Vec<RawLeg>,
    warnings: Vec<String>,
    blocking: Vec<String>,
}

fn required(value: Option<i64>, field: &str) -> AppResult<i64> {
    value.ok_or_else(|| AppError::validation(format!("Informe {field}")))
}

async fn draft_link(env: &LinkTestEnv, request: &LinkTestRequest) -> AppResult<Draft> {
    let Some(link_id) = request.link_id else {
        return draft_segment(env, request).await;
    };
    let (a, b) = edge_pair(env, link_id).await?;
    // Quem pediu pode orientar o enlace pela origem escolhida.
    let (a, b) = if request.source_device_id == Some(b) {
        (b, a)
    } else {
        (a, b)
    };
    let mut endpoints = Endpoints {
        env,
        cache: HashMap::new(),
    };
    Ok(Draft {
        legs: vec![RawLeg {
            kind: LinkTestLegKind::Link,
            a: endpoints.device(a).await?,
            b: endpoints.device(b).await?,
            route: Route::edge(link_id),
        }],
        warnings: Vec::new(),
        blocking: Vec::new(),
    })
}

/// `link` sem `linkId`: o trecho entre dois equipamentos com IP que só tem
/// equipamentos sem IP no meio (AP — switch não gerenciável — roteador). Com
/// outro equipamento com IP no caminho, o pedido é de caminho, não de enlace.
async fn draft_segment(env: &LinkTestEnv, request: &LinkTestRequest) -> AppResult<Draft> {
    let source = required(
        request.source_device_id,
        "o enlace (linkId) ou as duas pontas (sourceDeviceId)",
    )?;
    let target = required(
        request.target_device_id,
        "o enlace (linkId) ou as duas pontas (targetDeviceId)",
    )?;
    if source == target {
        return Err(AppError::validation(
            "As pontas do enlace precisam ser dispositivos diferentes",
        ));
    }
    let mut endpoints = Endpoints {
        env,
        cache: HashMap::new(),
    };
    let (from, to) = (
        endpoints.device(source).await?,
        endpoints.device(target).await?,
    );
    let devices = devices::Entity::find().all(&env.db).await?;
    let names: HashMap<i64, String> = devices
        .iter()
        .map(|device| (device.id, device.name.clone()))
        .collect();
    let hidden: HashMap<i64, String> = devices
        .into_iter()
        .filter(transparent::is_transparent)
        .map(|device| (device.id, device.name))
        .collect();
    let edges = load_path_edges(&env.db).await?;
    let segment = transparent::segment_edges(&edges, source, target, |id| hidden.contains_key(&id));
    let mut draft = Draft {
        legs: Vec::new(),
        warnings: Vec::new(),
        blocking: Vec::new(),
    };
    if let Some(path) = shortest_path(&segment, source, target) {
        let hops = transparent::collapse(&path.device_ids, &path.edge_ids, |id| {
            hidden.contains_key(&id)
        })
        .map_err(|id| transparent::not_an_endpoint(names.get(&id).map_or("?", String::as_str)))?;
        for hop in hops {
            draft.legs.push(RawLeg {
                kind: LinkTestLegKind::Link,
                a: from.clone(),
                b: to.clone(),
                route: Route::of(&hop, &hidden),
            });
        }
        return Ok(draft);
    }
    let Some(path) = shortest_path(&edges, source, target) else {
        draft.blocking.push(format!(
            "Não há caminho conhecido na topologia entre {} e {}",
            from.view.name, to.view.name
        ));
        return Ok(draft);
    };
    let between: Vec<&str> = path.device_ids[1..path.device_ids.len() - 1]
        .iter()
        .filter(|id| !hidden.contains_key(id))
        .filter_map(|id| names.get(id).map(String::as_str))
        .collect();
    Err(AppError::validation(format!(
        "Entre {} e {} há outros equipamentos com IP ({}): não é um enlace só. Para medir \
         enlace por enlace, use «Caminho completo»",
        from.view.name,
        to.view.name,
        between.join(", ")
    )))
}

async fn draft_path(env: &LinkTestEnv, request: &LinkTestRequest) -> AppResult<Draft> {
    let source = required(request.source_device_id, "a origem (sourceDeviceId)")?;
    let target = required(request.target_device_id, "o destino (targetDeviceId)")?;
    if source == target {
        return Err(AppError::validation(
            "A origem e o destino precisam ser dispositivos diferentes",
        ));
    }
    let mut endpoints = Endpoints {
        env,
        cache: HashMap::new(),
    };
    let (from, to) = (
        endpoints.device(source).await?,
        endpoints.device(target).await?,
    );
    let mut draft = Draft {
        legs: Vec::new(),
        warnings: Vec::new(),
        blocking: Vec::new(),
    };
    match find_path(&env.db, source, target).await? {
        None => draft.blocking.push(format!(
            "Não há caminho conhecido na topologia entre {} e {}",
            from.view.name, to.view.name
        )),
        Some(path) => {
            let (hops, names) = hops_of(env, &path, None).await?;
            for hop in hops {
                draft.legs.push(RawLeg {
                    kind: LinkTestLegKind::Link,
                    a: endpoints.device(hop.a).await?,
                    b: endpoints.device(hop.b).await?,
                    route: Route::of(&hop, &names),
                });
            }
        }
    }
    Ok(draft)
}

/// Ponta de um salto do caminho: o dispositivo de onde o teste parte vira a
/// própria origem (a central ou o agente), não um plugin nele.
async fn hop(
    endpoints: &mut Endpoints<'_>,
    id: i64,
    from: i64,
    origin: &Endpoint,
) -> AppResult<Endpoint> {
    if id == from {
        Ok(origin.clone())
    } else {
        endpoints.device(id).await
    }
}

async fn draft_server(env: &LinkTestEnv, request: &LinkTestRequest) -> AppResult<Draft> {
    let target = required(request.target_device_id, "o destino (targetDeviceId)")?;
    let system = SystemDeviceService::new(&env.db).find().await?;
    if system.as_ref().is_some_and(|device| device.id == target) {
        return Err(AppError::validation(
            "O destino precisa ser outro equipamento que não o próprio servidor",
        ));
    }
    let mut endpoints = Endpoints {
        env,
        cache: HashMap::new(),
    };
    let destination = endpoints.device(target).await?;
    // Destino alcançado só pelo agente do site: o teste parte dele.
    let origin = match endpoint::agent_of(&env.db, target).await? {
        Some(agent_id) => endpoint::agent_endpoint(&env.db, agent_id).await?,
        None => endpoint::central_endpoint(system),
    };
    let mut draft = Draft {
        legs: vec![RawLeg {
            kind: LinkTestLegKind::EndToEnd,
            a: origin.clone(),
            b: destination.clone(),
            route: Route::default(),
        }],
        warnings: Vec::new(),
        blocking: Vec::new(),
    };
    let Some(from) = origin.view.device_id else {
        draft.warnings.push(format!(
            "{} não está no mapa: o caminho enlace por enlace fica de fora e só a perna ponta a \
             ponta roda",
            origin.view.name
        ));
        return Ok(draft);
    };
    match find_path(&env.db, from, target).await? {
        None => draft.warnings.push(format!(
            "Não há caminho conhecido na topologia entre {} e {}: só a perna ponta a ponta roda",
            origin.view.name, destination.view.name
        )),
        Some(path) => {
            let (hops, names) = hops_of(env, &path, Some(from)).await?;
            // Uma perna só (um salto, ou só switches sem IP no meio) é a
            // própria perna ponta a ponta.
            if hops.len() >= 2 {
                for leg in hops {
                    draft.legs.push(RawLeg {
                        kind: LinkTestLegKind::Link,
                        a: hop(&mut endpoints, leg.a, from, &origin).await?,
                        b: hop(&mut endpoints, leg.b, from, &origin).await?,
                        route: Route::of(&leg, &names),
                    });
                }
            }
        }
    }
    Ok(draft)
}

/// Problemas de execução que a negociação não enxerga.
fn runtime_issues(
    env: &LinkTestEnv,
    server: &Endpoint,
    driver: &Endpoint,
    n: &Negotiation,
) -> Vec<String> {
    let mut issues = Vec::new();
    for endpoint in [server, driver] {
        issues.extend(endpoint.issues.iter().cloned());
    }
    if matches!(server.role, Role::Central { .. }) && !env.btest.available() {
        issues.push(
            "O servidor btest da central está desligado (BTEST_LISTEN vazio ou porta 2000 \
             ocupada)"
                .to_owned(),
        );
    }
    if let Role::Agent { agent_id } = driver.role {
        if let Err(error) = env.drivers.agent(agent_id) {
            issues.push(format!("{}: {error}", driver.view.name));
        }
    }
    if n.engine == ThroughputEngine::Btest
        && matches!(server.role, Role::Central { .. })
        && driver.ip().is_none()
    {
        issues.push(format!(
            "{} precisa de um IP válido: a central só aceita a conexão btest vinda dele",
            driver.view.name
        ));
    }
    issues
}

fn estimated_seconds(leg: &PlannedLeg, spec: &LinkTestSpec) -> u32 {
    let Some(n) = leg.negotiation.as_ref().filter(|_| leg.view.runnable) else {
        return 0;
    };
    let prepare_s: u32 = [&leg.a, &leg.b]
        .into_iter()
        .filter_map(|endpoint| endpoint.plugin())
        .filter(|plugin| plugin.roles.actions.prepare.is_some())
        .map(|plugin| {
            if plugin.roles.prepare_checks_only {
                CHECK_S
            } else {
                PREPARE_S
            }
        })
        .sum();
    let serve_stop = if leg.endpoint(n.server).view.kind == LinkTestEndpointKind::Device {
        SERVE_STOP_S
    } else {
        0
    };
    let per_flow = u32::from(n.duration_s) + MEASURE_OVERHEAD_S;
    let measure =
        if n.engine == ThroughputEngine::Btest && spec.direction == LinkTestDirection::Both {
            per_flow
        } else {
            per_flow * u32::try_from(spec.direction.flows().len()).unwrap_or(2)
        };
    LEG_OVERHEAD_S + prepare_s + serve_stop + measure
}

/// Sem combinação com o que está instalado: tenta de novo como se cada
/// ponta sem plugin já tivesse um dos que pode receber, do mais indicado para
/// o menos. Devolve a combinação que passaria a valer e os passos para chegar
/// a ela (instalar, cadastrar o acesso).
fn rescue(a: &Endpoint, b: &Endpoint, duration_s: u8) -> Option<(Negotiation, Vec<LinkTestFix>)> {
    if a.offers.is_empty() && b.offers.is_empty() {
        return None;
    }
    // Quem tem plugin (ou não tem oferta) entra como está.
    let options = |endpoint: &Endpoint| -> Vec<Option<usize>> {
        if endpoint.offers.is_empty() {
            vec![None]
        } else {
            (0..endpoint.offers.len()).map(Some).collect()
        }
    };
    let caps = |endpoint: &Endpoint, pick: Option<usize>| -> Capabilities {
        pick.map_or_else(
            || endpoint.caps.clone(),
            |index| endpoint.offers[index].caps.clone(),
        )
    };
    for pick_a in options(a) {
        for pick_b in options(b) {
            let (caps_a, caps_b) = (caps(a, pick_a), caps(b, pick_b));
            let party_a = Party {
                caps: &caps_a,
                ..a.party()
            };
            let party_b = Party {
                caps: &caps_b,
                ..b.party()
            };
            let Ok(n) = negotiate(party_a, party_b, duration_s) else {
                continue;
            };
            let mut fixes = Vec::new();
            for (endpoint, pick) in [(a, pick_a), (b, pick_b)] {
                if let (Some(index), Some(device_id)) = (pick, endpoint.device_id()) {
                    fixes.extend(endpoint.offers[index].fixes(device_id, &endpoint.view.name));
                }
            }
            return Some((n, fixes));
        }
    }
    None
}

fn evaluate(env: &LinkTestEnv, seq: u32, raw: RawLeg, spec: &LinkTestSpec) -> PlannedLeg {
    let RawLeg { kind, a, b, route } = raw;
    let mut view = LinkTestPlanLeg {
        seq,
        kind,
        a: a.view.clone(),
        b: b.view.clone(),
        edge_id: transparent::single_edge(&route.edge_ids),
        edge_ids: route.edge_ids,
        via_device_ids: route.via_device_ids,
        via_names: route.via_names,
        engine: None,
        server_side: None,
        duration_s: spec.duration_s,
        runnable: false,
        issues: Vec::new(),
        fixes: a.fixes.iter().chain(&b.fixes).cloned().collect(),
    };
    let sides = |n: &Negotiation| match n.server {
        LinkTestSide::A => (&a, &b),
        LinkTestSide::B => (&b, &a),
    };
    let negotiation = match negotiate(a.party(), b.party(), spec.duration_s) {
        Err(reason) => {
            match rescue(&a, &b, spec.duration_s) {
                // Roda depois dos passos: a perna já mostra como vai medir.
                Some((n, fixes)) => {
                    let (server, driver) = sides(&n);
                    view.issues = fixes.iter().map(|fix| fix.message().to_owned()).collect();
                    view.issues.extend(runtime_issues(env, server, driver, &n));
                    view.fixes.splice(0..0, fixes);
                    view.engine = Some(n.engine);
                    view.server_side = Some(n.server);
                    view.duration_s = n.duration_s;
                }
                None => view.issues.push(reason),
            }
            None
        }
        Ok(n) => {
            let (server, driver) = sides(&n);
            view.issues = runtime_issues(env, server, driver, &n);
            view.engine = Some(n.engine);
            view.server_side = Some(n.server);
            view.duration_s = n.duration_s;
            Some(n)
        }
    };
    view.runnable = negotiation.is_some() && view.issues.is_empty();
    PlannedLeg {
        view,
        a,
        b,
        negotiation,
    }
}

fn will_install(legs: &[PlannedLeg]) -> Vec<LinkTestInstall> {
    let mut by_device: BTreeMap<i64, (String, BTreeSet<String>)> = BTreeMap::new();
    for leg in legs.iter().filter(|leg| leg.view.runnable) {
        let Some(n) = &leg.negotiation else { continue };
        for side in [LinkTestSide::A, LinkTestSide::B] {
            let endpoint = leg.endpoint(side);
            let (Some(device_id), Some(plugin)) = (endpoint.device_id(), endpoint.plugin()) else {
                continue;
            };
            // Só confere (Linux): não há o que prometer instalar.
            if plugin.roles.actions.prepare.is_none() || plugin.roles.prepare_checks_only {
                continue;
            }
            let packages = endpoint::packages_for(n.engine, side == n.server);
            if packages.is_empty() {
                continue;
            }
            by_device
                .entry(device_id)
                .or_insert_with(|| (endpoint.view.name.clone(), BTreeSet::new()))
                .1
                .extend(packages);
        }
    }
    by_device
        .into_iter()
        .map(|(device_id, (device_name, packages))| LinkTestInstall {
            device_id,
            device_name,
            packages: packages.into_iter().collect(),
        })
        .collect()
}

fn describe(leg: &PlannedLeg) -> String {
    format!(
        "Perna {} ({} ↔ {})",
        leg.view.seq, leg.view.a.name, leg.view.b.name
    )
}

/// O que impede a perna e não tem passo que resolva ali mesmo.
fn open_issues(leg: &LinkTestPlanLeg) -> Vec<&str> {
    leg.issues
        .iter()
        .map(String::as_str)
        .filter(|issue| !leg.fixes.iter().any(|fix| fix.message() == *issue))
        .collect()
}

/// Os passos das pernas que ainda não rodam, sem repetição, na ordem em que
/// aparecem.
fn plan_fixes(legs: &[PlannedLeg]) -> Vec<LinkTestFix> {
    let mut fixes: Vec<LinkTestFix> = Vec::new();
    for fix in legs
        .iter()
        .filter(|leg| !leg.view.runnable)
        .flat_map(|leg| &leg.view.fixes)
    {
        if !fixes.contains(fix) {
            fixes.push(fix.clone());
        }
    }
    fixes
}

/// Monta o plano do pedido.
///
/// # Errors
///
/// `Validation` (pedido incompleto, especificação fora da faixa),
/// `NotFound` (dispositivo, enlace ou agente inexistente) ou erro do banco.
/// O que impede de rodar **não** é erro: vai em `blocking`.
pub async fn build(env: &LinkTestEnv, request: &LinkTestRequest) -> AppResult<BuiltPlan> {
    let spec = request.spec.clone().unwrap_or_default().resolve()?;
    let draft = match request.kind {
        LinkTestKind::Link => draft_link(env, request).await?,
        LinkTestKind::Path => draft_path(env, request).await?,
        LinkTestKind::Server => draft_server(env, request).await?,
    };
    let mut warnings = draft.warnings;
    let mut blocking = draft.blocking;
    let legs: Vec<PlannedLeg> = draft
        .legs
        .into_iter()
        .zip(1u32..)
        .map(|(raw, seq)| evaluate(env, seq, raw, &spec))
        .collect();

    for leg in &legs {
        if let Some(n) = &leg.negotiation {
            warnings.extend(n.warnings.iter().cloned());
        }
    }
    if spec.streams > 1
        && legs
            .iter()
            .any(|leg| leg.view.runnable && leg.view.engine == Some(ThroughputEngine::Btest))
    {
        warnings.push(
            "O btest mede com uma conexão TCP só: o número de fluxos vale apenas para o iperf3"
                .to_owned(),
        );
    }
    let runnable = legs.iter().filter(|leg| leg.view.runnable).count();
    for leg in legs.iter().filter(|leg| !leg.view.runnable) {
        let open = open_issues(&leg.view);
        // Só falta um passo (instalar, cadastrar o acesso): não é erro — a
        // tela oferece o passo. Sem nenhuma perna pronta, os passos também
        // impedem (ver `LinkTestPlan::fixes`).
        if open.is_empty() {
            if runnable > 0 {
                warnings.push(format!(
                    "{}: falta um passo (veja acima) — sem ele, esta perna será pulada",
                    describe(leg)
                ));
            }
            continue;
        }
        let text = format!("{}: {}", describe(leg), open.join("; "));
        if runnable == 0 {
            blocking.push(text);
        } else {
            warnings.push(format!("{text} — esta perna será pulada"));
        }
    }
    if legs.is_empty() && blocking.is_empty() {
        blocking.push("Nada a medir".to_owned());
    }

    let mut built = BuiltPlan {
        view: LinkTestPlan {
            kind: request.kind,
            legs: Vec::new(),
            estimated_seconds: legs.iter().map(|leg| estimated_seconds(leg, &spec)).sum(),
            warnings: Vec::new(),
            blocking: Vec::new(),
            will_install: will_install(&legs),
            fixes: plan_fixes(&legs),
        },
        legs,
        spec,
        request: request.clone(),
    };
    let busy = guard::busy_devices();
    for device_id in built.device_ids() {
        if let Some(test_id) = busy.get(&device_id) {
            let which = test_id.map_or_else(String::new, |id| format!(" (#{id})"));
            blocking.push(format!(
                "{} já está em outro teste de enlace{which}. Espere terminar ou cancele-o.",
                built.device_name(device_id)
            ));
        }
    }
    built.view.legs = built.legs.iter().map(|leg| leg.view.clone()).collect();
    built.view.warnings = warnings;
    built.view.blocking = blocking;
    Ok(built)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::link_tests::{
        endpoint::central_endpoint,
        model::LinkTestEndpoint,
        negotiate::{Capabilities, DriveCap},
    };

    fn device_endpoint(id: i64, caps: Capabilities, with_prepare: bool) -> Endpoint {
        use crate::services::plugins::manifest::{ThroughputActions, ThroughputRoles};
        let device = devices::Model {
            id,
            name: format!("D{id}"),
            ip_address: Some(format!("10.0.0.{id}")),
            ..test_device()
        };
        Endpoint {
            view: LinkTestEndpoint {
                kind: LinkTestEndpointKind::Device,
                device_id: Some(id),
                agent_id: None,
                name: device.name.clone(),
                ip: device.ip_address.clone(),
                platform: None,
            },
            role: Role::Device {
                device: Box::new(device),
                plugin: Some(endpoint::DevicePlugin {
                    slug: "p".into(),
                    name: "P".into(),
                    roles: ThroughputRoles {
                        serve: Vec::new(),
                        drive: Vec::new(),
                        actions: ThroughputActions {
                            prepare: with_prepare.then(|| "tp_prepare".to_owned()),
                            ..ThroughputActions::default()
                        },
                        prepare_checks_only: false,
                    },
                }),
            },
            caps,
            issues: Vec::new(),
            fixes: Vec::new(),
            offers: Vec::new(),
        }
    }

    /// Ponta cujo `prepare` só confere (o Linux).
    fn checks_only(mut endpoint: Endpoint) -> Endpoint {
        if let Role::Device {
            plugin: Some(plugin),
            ..
        } = &mut endpoint.role
        {
            plugin.roles.prepare_checks_only = true;
        }
        endpoint
    }

    fn test_device() -> devices::Model {
        let now = chrono::Utc::now().into();
        devices::Model {
            id: 0,
            site_id: None,
            network_id: None,
            parent_id: None,
            ip_address: None,
            name: String::new(),
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
            created_at: now,
            updated_at: now,
        }
    }

    fn iperf3() -> Capabilities {
        Capabilities {
            serve: vec![ThroughputEngine::Iperf3],
            drive: vec![DriveCap {
                engine: ThroughputEngine::Iperf3,
                auth: true,
                max_duration_s: None,
            }],
        }
    }

    fn leg(a: Endpoint, b: Endpoint, n: Negotiation) -> PlannedLeg {
        PlannedLeg {
            view: LinkTestPlanLeg {
                seq: 1,
                kind: LinkTestLegKind::Link,
                a: a.view.clone(),
                b: b.view.clone(),
                edge_id: Some(1),
                edge_ids: vec![1],
                via_device_ids: Vec::new(),
                via_names: Vec::new(),
                engine: Some(n.engine),
                server_side: Some(n.server),
                duration_s: n.duration_s,
                runnable: true,
                issues: Vec::new(),
                fixes: Vec::new(),
            },
            a,
            b,
            negotiation: Some(n),
        }
    }

    fn negotiation(engine: ThroughputEngine, server: LinkTestSide) -> Negotiation {
        Negotiation {
            engine,
            server,
            require_auth: false,
            duration_s: 5,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn estima_o_tempo_pelo_motor_e_pelos_papeis() {
        let spec = LinkTestSpec::default();
        // iperf3 entre dois equipamentos: serve/stop + dois sentidos em série.
        let iperf = leg(
            device_endpoint(1, iperf3(), false),
            device_endpoint(2, iperf3(), true),
            negotiation(ThroughputEngine::Iperf3, LinkTestSide::B),
        );
        assert_eq!(
            estimated_seconds(&iperf, &spec),
            LEG_OVERHEAD_S + PREPARE_S + SERVE_STOP_S + 2 * (5 + MEASURE_OVERHEAD_S)
        );
        // btest contra a central: sem serve no equipamento, sentidos juntos.
        let btest = leg(
            central_endpoint(None),
            device_endpoint(3, Capabilities::default(), false),
            negotiation(ThroughputEngine::Btest, LinkTestSide::A),
        );
        assert_eq!(
            estimated_seconds(&btest, &spec),
            LEG_OVERHEAD_S + 5 + MEASURE_OVERHEAD_S
        );
        let mut parado = btest;
        parado.view.runnable = false;
        assert_eq!(estimated_seconds(&parado, &spec), 0);
        // `prepare` que só confere (Linux) não conta o tempo de instalar.
        let linux = leg(
            device_endpoint(1, iperf3(), false),
            checks_only(device_endpoint(2, iperf3(), true)),
            negotiation(ThroughputEngine::Iperf3, LinkTestSide::B),
        );
        assert_eq!(
            estimated_seconds(&linux, &spec),
            LEG_OVERHEAD_S + CHECK_S + SERVE_STOP_S + 2 * (5 + MEASURE_OVERHEAD_S)
        );
    }

    #[test]
    fn lista_o_que_sera_instalado() {
        let legs = vec![leg(
            device_endpoint(1, iperf3(), true),
            device_endpoint(2, iperf3(), true),
            negotiation(ThroughputEngine::Iperf3, LinkTestSide::B),
        )];
        let install = will_install(&legs);
        assert_eq!(install.len(), 2);
        assert_eq!(install[0].device_name, "D1");
        assert_eq!(install[0].packages, ["iperf3"]);

        let legs = vec![leg(
            device_endpoint(4, iperf3(), true),
            device_endpoint(5, iperf3(), false),
            negotiation(ThroughputEngine::Btest, LinkTestSide::B),
        )];
        let install = will_install(&legs);
        assert_eq!(install.len(), 1, "só quem declara prepare");
        assert_eq!(install[0].packages, ["mikrotik-btest"], "o cliente btest");

        // Linux: o `prepare` só confere — o plano não promete instalar nada.
        let legs = vec![leg(
            checks_only(device_endpoint(6, iperf3(), true)),
            device_endpoint(7, iperf3(), true),
            negotiation(ThroughputEngine::Iperf3, LinkTestSide::B),
        )];
        let install = will_install(&legs);
        assert_eq!(install.len(), 1, "{install:?}");
        assert_eq!(install[0].device_name, "D7");
    }
}
