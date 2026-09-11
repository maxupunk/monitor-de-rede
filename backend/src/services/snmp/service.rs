//! Casos de uso SNMP para controllers, scheduler e checker.

use chrono::{DateTime, Utc};
use futures::future;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use std::collections::{BTreeMap, HashMap};

use crate::services::{
    alerts::fields as alert_fields,
    monitoring::{
        contracts::{CheckMetric, CheckResult, MonitorStatus},
        device_status::{self, DeviceStatus},
        execution_guard::calculate_smart_timeout_seconds,
        interface_monitoring, metrics_repository,
    },
    shared::errors::{AppError, AppResult},
    snmp::{
        client::{SnmpConfig, SnmpError, SnmpVersion},
        collectors::{
            collect_cpu, collect_hardware, collect_interfaces_and_traffic, collect_lldp,
            collect_memory, collect_system, status_label, InterfaceTraffic, LldpNeighbor,
            SnmpCpuInfo, SnmpInterface, SnmpMemoryInfo, SnmpSystemInfo,
        },
    },
};
use crate::{
    models::{
        _entities::{
            device_interfaces as device_interfaces_entity, metrics as metrics_entity,
            monitors as monitors_entity,
        },
        device_interfaces, devices, metrics, monitors,
    },
    services::topology,
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpTestResult {
    pub success: bool,
    pub system: SnmpSystemInfo,
    pub message: String,
}
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpScanResult {
    pub snmp_responded: bool,
    pub system_info: SnmpSystemInfo,
    pub interfaces: Vec<SnmpInterface>,
    pub traffic: Vec<InterfaceTraffic>,
    pub cpu_info: SnmpCpuInfo,
    pub memory_info: SnmpMemoryInfo,
    pub neighbors: Vec<LldpNeighbor>,
    /// Perfil de equipamento reconhecido (ex: MPPT, nobreak, sensor IoT)
    pub matched_profile: Option<super::profiles::SnmpProfileSummary>,
    /// Sensores e grandezas elétricas/ambientais descobertos
    pub sensors: Vec<super::profiles::DiscoveredSensor>,
    /// Falhas isoladas por coletor. Dados válidos dos demais coletores são
    /// preservados e a UI consegue explicar por que uma seção ficou vazia.
    pub collector_errors: std::collections::BTreeMap<String, String>,
    /// Campos abaixo dependem do banco: a coleta pura os devolve zerados e
    /// `scan_device` os preenche para a tela de descoberta.
    pub has_cpu_monitor: bool,
    pub has_memory_monitor: bool,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpDetectResult {
    pub detected: bool,
    pub version: Option<String>,
    pub community: Option<String>,
    pub result: Option<SnmpTestResult>,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnmpPollResult {
    pub scan: SnmpScanResult,
    pub interfaces_synced: usize,
    pub metrics_recorded: usize,
    pub links_resolved: usize,
    pub reboot_detected: bool,
}
/// Nomes dos monitores criados pela tela de descoberta. São a chave que liga o
/// que o usuário marcou no diálogo ao que já existe no banco, então mudá-los
/// desgarra os monitores já criados.
pub const CPU_MONITOR_NAME: &str = "Monitor de Uso de CPU";
pub const MEMORY_MONITOR_NAME: &str = "Monitor de Uso de Memoria";
pub const DEFAULT_SNMP_POLL_INTERVAL_SECONDS: i32 = 15;

#[must_use]
pub fn interface_monitor_name(if_name: &str) -> String {
    format!("Interface {if_name}")
}

#[must_use]
pub fn sensor_monitor_name(sensor_name: &str) -> String {
    format!("Sensor {sensor_name}")
}

/// Identificador do coletor de interfaces nos erros parciais da varredura.
///
/// Existe como constante porque `scan` a escreve e `ensure_scan_can_remove` a
/// lê: um literal solto nos dois lados deixaria a guarda silenciosamente cega
/// se a grafia mudasse de um lado só.
const INTERFACES_COLLECTOR: &str = "interfaces";

#[derive(Debug, Default)]
pub struct SnmpApplyOptions {
    pub enable_cpu_monitor: Option<bool>,
    pub enable_memory_monitor: Option<bool>,
    pub monitored_if_indexes: Vec<i32>,
    pub monitored_sensors: Vec<String>,
    pub clear_removed_history: Option<bool>,
}

/// Monta a configuração de coleta a partir do cadastro canônico do dispositivo.
/// Os monitores SNMP vinculados a ele não devem manter credenciais ou alvos
/// divergentes, pois todos participam da mesma coleta.
pub fn device_config(device: &devices::Model) -> AppResult<SnmpConfig> {
    device_config_with(device, crate::services::preferences::DEFAULT_SNMP_COMMUNITY)
}

/// Idem, com a comunidade a usar quando o dispositivo não tem uma própria.
///
/// Existe para o chamador que tem banco à mão poder aplicar a preferência
/// global (`Comunidade SNMP padrão`). Quem não tem cai no padrão de fábrica,
/// que é o mesmo valor que estava fixo aqui antes.
pub fn device_config_with(
    device: &devices::Model,
    comunidade_padrao: &str,
) -> AppResult<SnmpConfig> {
    let host = device
        .ip_address
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| device.name.clone());
    let mut config = SnmpConfig::v2c(
        host,
        device
            .snmp_community
            .as_deref()
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .unwrap_or(comunidade_padrao),
        161,
    );
    config.version = SnmpVersion::parse(device.snmp_version.as_deref().unwrap_or("v2c"))
        .ok_or_else(|| AppError::validation("Versão SNMP inválida"))?;
    Ok(config)
}

/// Mantém a cadência e o timeout de todos os itens SNMP do equipamento alinhados
/// ao intervalo que pertence ao próprio dispositivo.
pub async fn sync_monitor_intervals(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    interval_seconds: i32,
) -> AppResult<()> {
    let interval_seconds = interval_seconds.max(1);
    let timeout_seconds = calculate_smart_timeout_seconds("snmp", interval_seconds)
        .min((interval_seconds - 1).max(1))
        .max(1);
    monitors::Entity::update_many()
        .col_expr(
            monitors_entity::Column::IntervalSeconds,
            Expr::value(interval_seconds),
        )
        .col_expr(
            monitors_entity::Column::TimeoutSeconds,
            Expr::value(timeout_seconds),
        )
        .filter(monitors_entity::Column::DeviceId.eq(Some(device_id)))
        .filter(monitors_entity::Column::Type.eq("snmp"))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn test_connection(config: SnmpConfig) -> AppResult<SnmpTestResult> {
    let client = super::client::SnmpClient::new(config);
    let mut system = collect_system(&client).await.map_err(map_error)?;
    if system.responded() {
        if let Ok((vendor, model)) = collect_hardware(&client).await {
            system.hardware_vendor = vendor;
            system.hardware_model = model;
        }
    }
    let success = system.responded();
    Ok(SnmpTestResult {
        success,
        system,
        message: if success {
            "Conexão SNMP estabelecida".into()
        } else {
            "Agente SNMP respondeu sem OIDs utilizáveis".into()
        },
    })
}

pub async fn query_interfaces(
    config: SnmpConfig,
) -> AppResult<Vec<super::collectors::SnmpInterface>> {
    let client = super::client::SnmpClient::new(config);
    super::collectors::collect_interfaces(&client)
        .await
        .map_err(map_error)
}

pub async fn scan(config: SnmpConfig) -> AppResult<SnmpScanResult> {
    scan_with_profiles(config, &super::profiles::builtin_profiles()).await
}

pub async fn scan_with_profiles(
    config: SnmpConfig,
    profiles: &[super::profiles::SnmpDeviceProfile],
) -> AppResult<SnmpScanResult> {
    let client = super::client::SnmpClient::new(config);
    let (system, interfaces_and_traffic, cpu, memory, neighbors) = tokio::join!(
        collect_system(&client),
        collect_interfaces_and_traffic(&client),
        collect_cpu(&client),
        collect_memory(&client),
        collect_lldp(&client)
    );
    let system = system.map_err(map_error)?;
    let mut collector_errors = std::collections::BTreeMap::new();
    let (interfaces, traffic) = match interfaces_and_traffic {
        Ok(value) => value,
        Err(error) => {
            collector_errors.insert(INTERFACES_COLLECTOR.into(), error.to_string());
            Default::default()
        }
    };
    let cpu_info = cpu.unwrap_or_else(|error| {
        collector_errors.insert("cpu".into(), error.to_string());
        Default::default()
    });
    let memory_info = memory.unwrap_or_else(|error| {
        collector_errors.insert("memory".into(), error.to_string());
        Default::default()
    });
    let neighbors = neighbors.unwrap_or_else(|error| {
        collector_errors.insert("neighbors".into(), error.to_string());
        Default::default()
    });

    let (matched_profile, sensors) = if let Some(profile) = super::profiles::match_profile(
        profiles,
        system.sys_object_id.as_deref(),
        system.sys_descr.as_deref(),
    ) {
        let summary = super::profiles::SnmpProfileSummary::from(profile);
        let collected = super::profiles::collect_profile_sensors(&client, profile)
            .await
            .unwrap_or_else(|error| {
                collector_errors.insert("sensors".into(), error.to_string());
                Default::default()
            });
        (Some(summary), collected)
    } else {
        (None, Vec::new())
    };

    Ok(SnmpScanResult {
        snmp_responded: system.responded(),
        system_info: system,
        interfaces,
        traffic,
        cpu_info,
        memory_info,
        neighbors,
        matched_profile,
        sensors,
        collector_errors,
        has_cpu_monitor: false,
        has_memory_monitor: false,
    })
}

/// Varredura para a tela "Escaneamento & Descoberta SNMP".
///
/// É o `scan` puro mais o que só o banco sabe: quais monitores já estão
/// habilitados — para o diálogo abrir refletindo o que já está configurado, em
/// vez de propor tudo de novo.
pub async fn scan_device(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
) -> AppResult<SnmpScanResult> {
    let profiles = super::profiles::load_all_profiles(&ctx.db).await?;
    let mut scan = scan_with_profiles(config, &profiles).await?;
    let all_device_monitors = monitors::Entity::find()
        .filter(monitors_entity::Column::DeviceId.eq(Some(device.id)))
        .all(&ctx.db)
        .await?;

    let has_cpu_support = scan.cpu_info.usage_percent.is_some();
    let has_memory_support =
        scan.memory_info.used_percent.is_some() || scan.memory_info.total_kb.is_some();

    if has_cpu_support {
        scan.has_cpu_monitor = all_device_monitors
            .iter()
            .any(|m| m.name == CPU_MONITOR_NAME && m.enabled);
    } else {
        scan.has_cpu_monitor = false;
        if let Some(mon) = all_device_monitors
            .iter()
            .find(|m| m.name == CPU_MONITOR_NAME)
        {
            let _ = crate::services::maintenance::resource_cleanup::ResourceCleanupService::delete_monitor(
                &ctx.db, mon.id,
            ).await;
            let _ = metrics::Entity::delete_many()
                .filter(metrics_entity::Column::DeviceId.eq(device.id))
                .filter(metrics_entity::Column::Name.eq("cpu_usage"))
                .exec(&ctx.db)
                .await;
        }
    }

    if has_memory_support {
        scan.has_memory_monitor = all_device_monitors
            .iter()
            .any(|m| m.name == MEMORY_MONITOR_NAME && m.enabled);
    } else {
        scan.has_memory_monitor = false;
        if let Some(mon) = all_device_monitors
            .iter()
            .find(|m| m.name == MEMORY_MONITOR_NAME)
        {
            let _ = crate::services::maintenance::resource_cleanup::ResourceCleanupService::delete_monitor(
                &ctx.db, mon.id,
            ).await;
            let _ = metrics::Entity::delete_many()
                .filter(metrics_entity::Column::DeviceId.eq(device.id))
                .filter(metrics_entity::Column::Name.eq("memory_usage"))
                .exec(&ctx.db)
                .await;
        }
    }

    let existing: Vec<_> = all_device_monitors
        .into_iter()
        .filter(|m| m.enabled)
        .collect();

    // Sensores do perfil: marca se já há monitor habilitado para cada sensor
    for sensor in &mut scan.sensors {
        sensor.is_monitored = existing.iter().any(|m| {
            m.r#type == "snmp"
                && m.configuration.get("sensorKey").and_then(|v| v.as_str())
                    == Some(sensor.key.as_str())
        });
    }

    // A varredura não conhece o `id` da linha: é a cadeia de identidade que liga
    // a porta recém-lida ao registro local, e só então a pergunta "esta é
    // monitorada?" tem um alvo. Pelo nome, a órfã de uma PPPoE renumerada
    // aparecia marcada e o diálogo de descoberta acusava remoção a cada
    // gravação, para uma interface que o operador não tinha como selecionar.
    let monitoradas: std::collections::BTreeSet<i64> = existing
        .iter()
        .filter_map(|monitor| monitor.interface_id)
        .collect();
    let linhas = device_interfaces::Entity::find()
        .filter(device_interfaces_entity::Column::DeviceId.eq(device.id))
        .all(&ctx.db)
        .await?;
    let mut casadas = InterfaceMatcher::resolve(linhas, &scan.interfaces);
    for (posicao, interface) in scan.interfaces.iter_mut().enumerate() {
        interface.is_monitored = casadas
            .take(posicao)
            .is_some_and(|linha| monitoradas.contains(&linha.id));
    }
    Ok(scan)
}

/// Casa cada interface da varredura com a linha já persistida.
///
/// # O problema
///
/// O `ifIndex` não é identidade. A RFC 2863 só garante que ele seja constante
/// **entre re-inicializações** do agente, não através de reboots — e uma
/// interface PPPoE renumera a cada reconexão, liberando um índice que a próxima
/// porta reaproveita. Casar por índice erra dos dois jeitos: perde a interface
/// que se moveu e sequestra a linha de outra que herdou o número.
///
/// # A cadeia
///
/// Em vez de eleger um único campo, cada interface casa pelo sinal mais forte
/// que ela tiver, na ordem `ifAlias` → `ifName` → `ifDescr` → `ifPhysAddress` →
/// `ifIndex`:
///
/// - **`ifAlias`** primeiro porque é o que a RFC 2863 desenha para isto: o
///   agente é obrigado a preservá-lo através de reboots, e é o operador quem o
///   escreve. É a âncora que o PRTG usa para reconciliar índice após reset.
/// - **`ifName`** e **`ifDescr`** em seguida — os dois modos que o LibreNMS
///   oferece como alternativa ao índice.
/// - **`ifPhysAddress`** só como desempate tardio: num roteador Linux as
///   bridges e VLANs herdam o MAC da porta física, então ele identifica muito
///   menos do que parece. Nos dados de produção deste projeto, 42 interfaces
///   têm MAC e só 18 desses valores são únicos no dispositivo.
/// - **`ifIndex`** por último, como recurso de quem não tem mais nada.
///
/// A diferença para o modo configurável do LibreNMS é que aqui a degradação é
/// **por interface**, não por equipamento: num parque misto, o Mikrotik com
/// `ifAlias` preenchido e o OpenWrt sem ele são atendidos pela mesma regra, sem
/// ninguém precisar escolher o modo certo para cada um.
///
/// # Por que a unicidade é exigida dos dois lados
///
/// Um sinal só identifica quando aponta para **uma** linha e vem de **uma**
/// interface. Exigir unicidade apenas no banco deixava o desempate na mão da
/// ordem de iteração: com duas portas `lan` na varredura e uma no banco, quem
/// ficava com a linha era quem chegasse primeiro. Ambíguo de qualquer um dos
/// lados, o sinal é descartado e a decisão desce para o próximo da cadeia.
///
/// # Por que em duas passadas
///
/// Resolver interface a interface faria um casamento fraco consumir a linha que
/// um casamento forte reivindicaria depois. A resolução acontece de uma vez,
/// sinal por sinal sobre o conjunto inteiro: todos os `ifAlias` casam antes de
/// qualquer `ifName` ser considerado.
struct InterfaceMatcher {
    /// Posição da interface na varredura → linha do banco que lhe corresponde.
    casados: HashMap<usize, device_interfaces::Model>,
}

/// Normaliza um sinal textual: `None` quando não serve para identificar.
fn sinal(valor: Option<&str>) -> Option<String> {
    let limpo = valor?.trim().to_lowercase();
    if limpo.is_empty() {
        return None;
    }
    Some(limpo)
}

/// O MAC zerado é o que um agente devolve para interface sem endereço (loopback,
/// túnel, PPP). Tratá-lo como sinal juntaria todas elas numa só.
fn sinal_mac(valor: Option<&str>) -> Option<String> {
    let limpo = sinal(valor)?;
    if limpo.bytes().all(|b| b == b'0' || b == b':' || b == b'-') {
        return None;
    }
    Some(limpo)
}

/// Os extratores da cadeia, do sinal mais forte para o mais fraco.
type Extrator = (
    fn(&device_interfaces::Model) -> Option<String>,
    fn(&SnmpInterface) -> Option<String>,
);

const CADEIA: &[Extrator] = &[
    (
        |row| sinal(row.alias.as_deref()),
        |src| sinal(src.if_alias.as_deref()),
    ),
    (
        |row| sinal(Some(&row.name)),
        |src| sinal(Some(&src.if_name)),
    ),
    (
        |row| sinal(row.description.as_deref()),
        |src| sinal(src.if_descr.as_deref()),
    ),
    (
        |row| sinal_mac(row.mac_address.as_deref()),
        |src| sinal_mac(src.mac_address.as_deref()),
    ),
    (
        |row| row.snmp_index.map(|indice| indice.to_string()),
        |src| Some(src.if_index.to_string()),
    ),
];

/// Índice `valor → chave` contendo só os valores que aparecem uma única vez.
fn apenas_unicos<T: Copy, I: IntoIterator<Item = (String, T)>>(pares: I) -> HashMap<String, T> {
    let mut unicos: HashMap<String, T> = HashMap::new();
    let mut repetidos: Vec<String> = Vec::new();
    for (valor, chave) in pares {
        if unicos.insert(valor.clone(), chave).is_some() {
            repetidos.push(valor);
        }
    }
    for valor in repetidos {
        unicos.remove(&valor);
    }
    unicos
}

impl InterfaceMatcher {
    fn resolve(linhas: Vec<device_interfaces::Model>, varredura: &[SnmpInterface]) -> Self {
        let mut disponiveis: BTreeMap<i64, device_interfaces::Model> =
            linhas.into_iter().map(|row| (row.id, row)).collect();
        let mut pendentes: Vec<usize> = (0..varredura.len()).collect();
        let mut casados = HashMap::new();

        for (do_banco, da_varredura) in CADEIA {
            if pendentes.is_empty() || disponiveis.is_empty() {
                break;
            }
            let no_banco = apenas_unicos(
                disponiveis
                    .values()
                    .filter_map(|row| do_banco(row).map(|valor| (valor, row.id))),
            );
            let na_varredura = apenas_unicos(
                pendentes
                    .iter()
                    .filter_map(|&pos| da_varredura(&varredura[pos]).map(|valor| (valor, pos))),
            );

            for (valor, pos) in na_varredura {
                let Some(&id) = no_banco.get(&valor) else {
                    continue;
                };
                let Some(row) = disponiveis.remove(&id) else {
                    continue;
                };
                casados.insert(pos, row);
            }
            pendentes.retain(|pos| !casados.contains_key(pos));
        }

        Self { casados }
    }

    /// A linha correspondente à interface nesta posição da varredura, ou `None`
    /// quando ela é nova para o dispositivo. Consome: cada linha vale por uma
    /// varredura.
    fn take(&mut self, posicao: usize) -> Option<device_interfaces::Model> {
        self.casados.remove(&posicao)
    }
}

pub async fn poll_device(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
) -> AppResult<SnmpPollResult> {
    let profiles = super::profiles::load_all_profiles(&ctx.db).await?;
    let scan = scan_with_profiles(config, &profiles).await?;
    if !scan.snmp_responded {
        return Ok(SnmpPollResult {
            scan,
            interfaces_synced: 0,
            metrics_recorded: 0,
            links_resolved: 0,
            reboot_detected: false,
        });
    }

    // Carrega interfaces já conhecidas de uma vez para evitar SELECT N+1 no
    // loop de sincronização (QUA-04).
    let existing_interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces_entity::Column::DeviceId.eq(device.id))
        .all(&ctx.db)
        .await?;
    let mut conhecidas = InterfaceMatcher::resolve(existing_interfaces, &scan.interfaces);

    let mut interfaces = std::collections::BTreeMap::new();
    for (posicao, interface) in scan.interfaces.iter().enumerate() {
        let existing = conhecidas.take(posicao);
        let saved = sync_interface(&ctx.db, device.id, interface, existing.as_ref()).await?;
        // Só interface administrativamente habilitada é avaliada: uma porta
        // que o operador desligou não pode gerar alerta de queda de link.
        if saved.interface.admin_status.as_deref() == Some("up") {
            interface_monitoring::evaluate_interface_state(
                ctx,
                device,
                &saved.interface,
                saved.previous_oper_status.as_deref(),
                saved.previous_speed,
            )
            .await?;
        }
        interfaces.insert(interface.if_index, saved.interface);
    }
    if let Some(ref link_name) = device.link_interface_name {
        if let Some(matching) = interfaces
            .values()
            .find(|i| i.name.eq_ignore_ascii_case(link_name))
        {
            if device.link_interface_id != Some(matching.id) {
                let mut dev_active: devices::ActiveModel = device.clone().into();
                dev_active.link_interface_id = Set(Some(matching.id));
                let _ = dev_active.update(&ctx.db).await;
            }
        }
    }
    let previous_uptime = latest_device_metric(&ctx.db, device.id, "snmp_uptime")
        .await?
        .map(|metric| metric.value.max(0.0) as u64);
    let interface_ids: Vec<i64> = interfaces.values().map(|row| row.id).collect();
    let mut previous_metrics = latest_metrics_for_interfaces(
        &ctx.db,
        device.id,
        &interface_ids,
        &["ifHCInOctets", "ifHCOutOctets"],
    )
    .await?;

    // Acumula métricas de tráfego e sistema para inserção em massa (QUA-04).
    let mut pending_metrics: Vec<PendingMetric> = Vec::new();
    let mut reboot_detected = false;
    for traffic in &scan.traffic {
        let Some(interface) = interfaces.get(&traffic.if_index) else {
            continue;
        };
        let (metrics, reboot) = build_traffic_metrics(
            device.id,
            interface,
            traffic,
            &mut previous_metrics,
            previous_uptime,
            scan.system_info.sys_up_time,
        );
        pending_metrics.extend(metrics);
        reboot_detected |= reboot;
    }
    pending_metrics.extend(build_system_metrics(device.id, &scan));
    let metrics_recorded = record_metrics_bulk(&ctx.db, pending_metrics).await?;
    // O status **não** é escrito aqui (matriz de paridade #4): quem decide é o
    // `device_status`, agregando todos os monitores habilitados. Gravar "online"
    // direto era o bug de alternância — a coleta subia o dispositivo em silêncio
    // e o ping seguinte o derrubava, publicando transição a cada ciclo. A coleta
    // entra como `observed_status`, que só prevalece quando não há monitor algum.
    device_status::refresh_from_monitors(ctx, device, Some(DeviceStatus::Online), Some(Utc::now()))
        .await?;
    let links_resolved = topology::resolve_discovered_neighbors(ctx, device, &scan.neighbors)
        .await?
        .len();
    Ok(SnmpPollResult {
        scan,
        interfaces_synced: interfaces.len(),
        metrics_recorded,
        links_resolved,
        reboot_detected,
    })
}

/// Executa uma coleta única e deriva o resultado individual de cada monitor
/// SNMP do dispositivo. Assim o histórico e os alertas continuam por item, mas
/// CPU, memória, interfaces e tráfego não abrem consultas independentes.
pub async fn poll_device_monitors(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    monitored_items: &[monitors::Model],
) -> Vec<(i64, CheckResult)> {
    let started_at = Utc::now();
    // A preferência entra aqui, e não no `device_config` sem banco: é a coleta
    // que precisa dela, e é onde o dispositivo sem comunidade própria aparece.
    let comunidade = crate::services::preferences::load(&ctx.db)
        .await
        .map(|preferencias| preferencias.default_snmp_community)
        .unwrap_or_else(|_| crate::services::preferences::DEFAULT_SNMP_COMMUNITY.to_owned());
    let poll = match device_config_with(device, &comunidade) {
        Ok(config) => poll_device(ctx, device, config).await,
        Err(error) => Err(error),
    };
    let finished_at = Utc::now();
    match poll {
        Ok(poll) => {
            sync_migrated_monitor_indexes(ctx, monitored_items, &poll).await;
            monitored_items
                .iter()
                .map(|monitor| {
                    (
                        monitor.id,
                        monitor_result_from_poll(monitor, &poll, started_at, finished_at),
                    )
                })
                .collect()
        }
        Err(error) => monitored_items
            .iter()
            .map(|monitor| {
                (
                    monitor.id,
                    failed_monitor_result(error.to_string(), started_at, finished_at),
                )
            })
            .collect(),
    }
}

/// Sincroniza no banco o ifIndex de monitores cujas interfaces migraram dinamicamente no roteador.
async fn sync_migrated_monitor_indexes(
    ctx: &loco_rs::app::AppContext,
    monitored_items: &[monitors::Model],
    poll: &SnmpPollResult,
) {
    for monitor in monitored_items {
        let configured_index = monitor
            .configuration
            .get("ifIndex")
            .and_then(serde_json::Value::as_i64)
            .and_then(|v| i32::try_from(v).ok());
        let resolved_index = resolve_monitor_if_index(monitor, poll);
        if let (Some(new_index), Some(old_index)) = (resolved_index, configured_index) {
            if new_index != old_index {
                let mut new_config = monitor.configuration.clone();
                if let serde_json::Value::Object(ref mut map) = new_config {
                    map.insert("ifIndex".to_string(), serde_json::json!(new_index));
                }
                if let Err(error) = monitors::Entity::update_many()
                    .col_expr(
                        monitors_entity::Column::Configuration,
                        sea_orm::sea_query::Expr::value(new_config),
                    )
                    .filter(monitors_entity::Column::Id.eq(monitor.id))
                    .exec(&ctx.db)
                    .await
                {
                    tracing::warn!(
                        %error,
                        monitor_id = monitor.id,
                        new_index,
                        "falha ao sincronizar ifIndex atualizado no monitor"
                    );
                }
            }
        }
    }
}

/// Resolve o ifIndex da interface suportando migração dinâmica de índice (ex: reconexão PPPoE/túnel).
fn resolve_monitor_if_index(monitor: &monitors::Model, poll: &SnmpPollResult) -> Option<i32> {
    let config = &monitor.configuration;
    let configured_index = config
        .get("ifIndex")
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    let configured_name = config
        .get("ifName")
        .and_then(serde_json::Value::as_str)
        .or_else(|| monitor.name.strip_prefix("Interface "));

    if let (Some(index), Some(name)) = (configured_index, configured_name) {
        if poll.scan.interfaces.iter().any(|i| {
            i.if_index == index
                && (i.if_name.eq_ignore_ascii_case(name)
                    || i.if_descr
                        .as_deref()
                        .is_some_and(|d| d.eq_ignore_ascii_case(name)))
        }) {
            return Some(index);
        }
    }

    if let Some(name) = configured_name {
        let name_clean = name.trim();
        if !name_clean.is_empty() {
            if let Some(matched) = poll.scan.interfaces.iter().find(|i| {
                i.if_name.eq_ignore_ascii_case(name_clean)
                    || i.if_descr
                        .as_deref()
                        .is_some_and(|d| d.eq_ignore_ascii_case(name_clean))
                    || i.if_alias
                        .as_deref()
                        .is_some_and(|a| a.eq_ignore_ascii_case(name_clean))
            }) {
                return Some(matched.if_index);
            }
        }
    }

    if let Some(index) = configured_index {
        if poll.scan.traffic.iter().any(|t| t.if_index == index)
            || poll.scan.interfaces.iter().any(|i| i.if_index == index)
        {
            return Some(index);
        }
    }

    configured_index
}

fn monitor_result_from_poll(
    monitor: &monitors::Model,
    poll: &SnmpPollResult,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> CheckResult {
    let if_index = resolve_monitor_if_index(monitor, poll);
    let config = &monitor.configuration;
    let metric = config
        .get("metric")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(if if_index.is_some() {
            "interface_status"
        } else {
            "uptime"
        });
    let (status, message, data) = match metric {
        // `usagePercent`/`usedPercent` soltos, que estas linhas publicavam
        // antes, estavam **fora** do vocabulário do motor: não havia regra
        // possível sobre eles. Trocá-los pelas chaves de saúde de dispositivo
        // não quebra nada e faz o alerta de CPU passar a valer para o parque
        // inteiro, não só para o servidor.
        "cpu_usage" if poll.scan.cpu_info.usage_percent.is_some() => (
            MonitorStatus::Up,
            "Uso de CPU coletado".to_string(),
            serde_json::json!({
                alert_fields::CPU_USAGE_PERCENT: poll.scan.cpu_info.usage_percent
            }),
        ),
        "memory_usage" if poll.scan.memory_info.used_percent.is_some() => (
            MonitorStatus::Up,
            "Uso de memória coletado".to_string(),
            serde_json::json!({
                alert_fields::MEMORY_USED_PERCENT: poll.scan.memory_info.used_percent
            }),
        ),
        "traffic" | "interface_traffic" => match if_index.and_then(|index| {
            poll.scan
                .traffic
                .iter()
                .find(|traffic| traffic.if_index == index)
        }) {
            Some(traffic) => (
                MonitorStatus::Up,
                "Tráfego de interface coletado".to_string(),
                serde_json::json!({
                    "ifIndex": traffic.if_index,
                    "counterBits": traffic.counter_bits,
                }),
            ),
            None => missing_measurement_result("contadores de tráfego da interface"),
        },
        "interface_status" | "status" => match if_index.and_then(|index| {
            poll.scan
                .interfaces
                .iter()
                .find(|interface| interface.if_index == index)
        }) {
            Some(interface) => {
                let status = interface_monitor_status(
                    interface.if_admin_status.unwrap_or(1),
                    interface.if_oper_status.unwrap_or(1),
                );
                (
                    status,
                    "Estado da interface coletado".to_string(),
                    serde_json::json!({
                        "ifName": interface.if_name,
                        "ifIndex": interface.if_index,
                        "adminStatus": interface.if_admin_status.map(status_label),
                        "operStatus": interface.if_oper_status.map(status_label),
                    }),
                )
            }
            None => missing_measurement_result("estado da interface"),
        },
        "cpu_usage" => missing_measurement_result("uso de CPU"),
        "memory_usage" => missing_measurement_result("uso de memória"),
        "sensor" => {
            let sensor_key = config.get("sensorKey").and_then(serde_json::Value::as_str);
            match sensor_key.and_then(|key| poll.scan.sensors.iter().find(|s| s.key == key)) {
                Some(sensor) => match sensor.value {
                    Some(val) => (
                        MonitorStatus::Up,
                        format!("Sensor {} coletado: {val} {}", sensor.name, sensor.unit),
                        serde_json::json!({
                            "sensorKey": sensor.key,
                            "name": sensor.name,
                            "value": val,
                            "unit": sensor.unit,
                            "raw": sensor.raw_value,
                            "dataType": sensor.data_type,
                            "reading": sensor.formatted_value,
                            "states": sensor.states,
                            "icon": sensor.icon,
                            "color": sensor.color,
                        }),
                    ),
                    None => (
                        MonitorStatus::Down,
                        format!("Sensor {} não retornou leitura válida", sensor.name),
                        serde_json::json!({
                            "sensorKey": sensor.key,
                            "name": sensor.name,
                            "reason": "sensor_value_missing",
                            "dataType": sensor.data_type,
                        }),
                    ),
                },
                None => missing_measurement_result("leitura do sensor"),
            }
        }
        _ if poll.scan.snmp_responded => (
            MonitorStatus::Up,
            "Agente SNMP respondeu à coleta consolidada".to_string(),
            serde_json::json!({ "sysUpTime": poll.scan.system_info.sys_up_time }),
        ),
        _ => missing_measurement_result("resposta do agente SNMP"),
    };

    let mut metrics = Vec::new();
    match metric {
        "cpu_usage" => {
            if let Some(usage) = poll.scan.cpu_info.usage_percent {
                metrics.push(CheckMetric {
                    name: "cpu_usage".to_string(),
                    value: usage,
                    unit: "percent".to_string(),
                });
            }
        }
        "memory_usage" => {
            if let Some(usage) = poll.scan.memory_info.used_percent {
                metrics.push(CheckMetric {
                    name: "memory_usage".to_string(),
                    value: usage,
                    unit: "percent".to_string(),
                });
            }
        }
        "sensor" => {
            let sensor_key = config.get("sensorKey").and_then(serde_json::Value::as_str);
            if let Some(sensor) =
                sensor_key.and_then(|key| poll.scan.sensors.iter().find(|s| s.key == key))
            {
                if let Some(val) = sensor.value {
                    metrics.push(CheckMetric {
                        name: sensor.key.clone(),
                        value: val,
                        unit: sensor.unit.clone(),
                    });
                }
            }
        }
        _ => {}
    }

    CheckResult {
        success: matches!(status, MonitorStatus::Up | MonitorStatus::Disabled),
        status,
        started_at,
        finished_at,
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
        message: Some(message),
        metrics,
        data,
    }
}

fn missing_measurement_result(measurement: &str) -> (MonitorStatus, String, serde_json::Value) {
    (
        MonitorStatus::Down,
        format!("A coleta SNMP não informou {measurement}"),
        serde_json::json!({ "reason": "measurement_missing", "measurement": measurement }),
    )
}

fn failed_monitor_result(
    error: String,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
) -> CheckResult {
    CheckResult {
        success: false,
        status: MonitorStatus::Down,
        started_at,
        finished_at,
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
        message: Some(format!("Falha na coleta SNMP consolidada: {error}")),
        metrics: vec![],
        data: serde_json::json!({ "reason": "snmp_poll_failed" }),
    }
}

fn interface_monitor_status(admin: u64, oper: u64) -> MonitorStatus {
    if admin == 2 {
        MonitorStatus::Disabled
    } else if oper == 1 {
        MonitorStatus::Up
    } else if oper == 2 {
        MonitorStatus::Down
    } else {
        MonitorStatus::Warning
    }
}

/// Recusa aplicar a configuração quando a varredura não é confiável.
///
/// `apply_monitors` lê "interface ausente da varredura" como "interface
/// removida do equipamento" e, com `clear_removed_history`, apaga o monitor, as
/// métricas e a própria linha. Só que uma coleta que **falhou** também devolve
/// lista vazia: `scan` registra o erro em `collector_errors` e segue com `Ok`
/// de propósito, para a tela de descoberta conseguir explicar a seção vazia em
/// vez de morrer inteira.
///
/// Sem esta guarda os dois casos eram indistinguíveis, e um único PDU perdido
/// no walk do `ifTable` — UDP, 4 s de timeout por requisição — bastava para
/// varrer monitores e histórico de um equipamento saudável. O terceiro teste é
/// a rede de segurança para uma falha que não vire erro de coletor: nenhum
/// equipamento perde todas as interfaces de uma vez.
fn ensure_scan_can_remove(scan: &SnmpScanResult, known_interfaces: usize) -> AppResult<()> {
    const NADA_ALTERADO: &str = "Nenhuma configuração foi alterada.";

    if !scan.snmp_responded {
        return Err(AppError::service_unavailable(format!(
            "O equipamento não respondeu ao SNMP agora. {NADA_ALTERADO} Tente novamente."
        )));
    }
    if let Some(detail) = scan.collector_errors.get(INTERFACES_COLLECTOR) {
        if known_interfaces > 0 {
            tracing::warn!(
                %detail,
                "coleta de interfaces falhou: aplicação de monitores recusada"
            );
            return Err(AppError::service_unavailable(format!(
                "A leitura das interfaces via SNMP falhou agora. {NADA_ALTERADO} Tente novamente."
            )));
        }
    }
    if scan.interfaces.is_empty() && known_interfaces > 0 {
        tracing::warn!(
            known_interfaces,
            "varredura sem interfaces em equipamento que já tem interfaces registradas:              aplicação de monitores recusada"
        );
        return Err(AppError::service_unavailable(format!(
            "A varredura SNMP não devolveu nenhuma interface, mas este equipamento tem              {known_interfaces} registradas. {NADA_ALTERADO} Tente novamente."
        )));
    }
    Ok(())
}

pub async fn apply_monitors(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
    options: SnmpApplyOptions,
) -> AppResult<()> {
    let profiles = super::profiles::load_all_profiles(&ctx.db).await?;
    let scan = scan_with_profiles(config.clone(), &profiles).await?;
    let clear_history = options.clear_removed_history.unwrap_or(true);

    // Lido antes de qualquer escrita: a guarda precisa do total conhecido, e
    // recusar depois de já ter mexido no dispositivo não seria recusar.
    let db_interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces_entity::Column::DeviceId.eq(device.id))
        .all(&ctx.db)
        .await?;
    ensure_scan_can_remove(&scan, db_interfaces.len())?;

    devices::ActiveModel {
        id: Set(device.id),
        snmp_enabled: Set(true),
        is_monitored: Set(true),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;

    let discovered_indexes: std::collections::HashSet<i32> =
        scan.interfaces.iter().map(|i| i.if_index).collect();
    let discovered_names: std::collections::HashSet<String> = scan
        .interfaces
        .iter()
        .map(|i| i.if_name.to_lowercase())
        .collect();

    let mut conhecidas = InterfaceMatcher::resolve(db_interfaces.clone(), &scan.interfaces);

    // Numa transação porque a remoção de uma interface são três escritas que
    // só valem juntas — métricas, monitor e a própria linha. Sem ela, o pool de
    // uma conexão do SQLite estourando o `connect_timeout` no meio do laço
    // deixava o equipamento pela metade: interface apagada, monitor de pé.
    let removal = ctx.db.begin().await?;
    for db_iface in db_interfaces {
        let Some(snmp_idx) = db_iface.snmp_index else {
            continue;
        };
        if discovered_indexes.contains(&snmp_idx)
            || discovered_names.contains(&db_iface.name.to_lowercase())
        {
            continue;
        }
        // Pelo vínculo, não pelo nome: apagar o monitor da porta homônima
        // que continua viva seria pior do que não apagar nada.
        let monitor = monitors::Entity::find()
            .filter(monitors_entity::Column::DeviceId.eq(Some(device.id)))
            .filter(monitors_entity::Column::InterfaceId.eq(Some(db_iface.id)))
            .one(&removal)
            .await?;
        if clear_history {
            metrics::Entity::delete_many()
                .filter(metrics_entity::Column::InterfaceId.eq(Some(db_iface.id)))
                .exec(&removal)
                .await?;
            if let Some(mon) = monitor {
                crate::services::maintenance::resource_cleanup::ResourceCleanupService::delete_monitor(
                    &removal, mon.id,
                )
                .await?;
            }
            device_interfaces::Entity::delete_by_id(db_iface.id)
                .exec(&removal)
                .await?;
        } else {
            if let Some(mon) = monitor {
                let mut active: monitors::ActiveModel = mon.into();
                active.enabled = Set(false);
                active.update(&removal).await?;
            }
            let mut active: device_interfaces::ActiveModel = db_iface.into();
            active.admin_status = Set(Some("down".into()));
            active.update(&removal).await?;
        }
    }
    removal.commit().await?;

    let selected = options
        .monitored_if_indexes
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    for (posicao, source) in scan.interfaces.iter().enumerate() {
        let existing = conhecidas.take(posicao);
        let interface = sync_interface(&ctx.db, device.id, source, existing.as_ref())
            .await?
            .interface;
        set_monitoring(
            &ctx.db,
            device.id,
            &config,
            &interface,
            selected.contains(&source.if_index),
            device.snmp_poll_interval_seconds,
            clear_history,
        )
        .await?;
    }
    let has_cpu_support = scan.cpu_info.usage_percent.is_some();
    let has_memory_support =
        scan.memory_info.used_percent.is_some() || scan.memory_info.total_kb.is_some();

    if has_cpu_support {
        if let Some(enabled) = options.enable_cpu_monitor {
            sync_monitor(
                &ctx.db,
                MonitorSpec {
                    device_id: device.id,
                    interface: None,
                    name: CPU_MONITOR_NAME,
                    enabled,
                    configuration: monitor_configuration(&config, "cpu_usage"),
                    up: true,
                    interval_seconds: device.snmp_poll_interval_seconds,
                },
            )
            .await?;
            if !enabled && clear_history {
                metrics::Entity::delete_many()
                    .filter(metrics_entity::Column::DeviceId.eq(device.id))
                    .filter(metrics_entity::Column::Name.eq("cpu_usage"))
                    .exec(&ctx.db)
                    .await?;
            }
        }
    } else {
        // Equipamento não possui CPU (ex: MPPT, IoT): remove monitor órfão de CPU se existir
        if let Some(mon) = monitors::Entity::find()
            .filter(monitors_entity::Column::DeviceId.eq(Some(device.id)))
            .filter(monitors_entity::Column::Name.eq(CPU_MONITOR_NAME))
            .one(&ctx.db)
            .await?
        {
            let _ = crate::services::maintenance::resource_cleanup::ResourceCleanupService::delete_monitor(
                &ctx.db, mon.id,
            )
            .await;
            let _ = metrics::Entity::delete_many()
                .filter(metrics_entity::Column::DeviceId.eq(device.id))
                .filter(metrics_entity::Column::Name.eq("cpu_usage"))
                .exec(&ctx.db)
                .await;
        }
    }

    if has_memory_support {
        if let Some(enabled) = options.enable_memory_monitor {
            sync_monitor(
                &ctx.db,
                MonitorSpec {
                    device_id: device.id,
                    interface: None,
                    name: MEMORY_MONITOR_NAME,
                    enabled,
                    configuration: monitor_configuration(&config, "memory_usage"),
                    up: true,
                    interval_seconds: device.snmp_poll_interval_seconds,
                },
            )
            .await?;
            if !enabled && clear_history {
                metrics::Entity::delete_many()
                    .filter(metrics_entity::Column::DeviceId.eq(device.id))
                    .filter(metrics_entity::Column::Name.eq("memory_usage"))
                    .exec(&ctx.db)
                    .await?;
            }
        }
    } else {
        // Equipamento não possui Memória: remove monitor órfão de memória se existir
        if let Some(mon) = monitors::Entity::find()
            .filter(monitors_entity::Column::DeviceId.eq(Some(device.id)))
            .filter(monitors_entity::Column::Name.eq(MEMORY_MONITOR_NAME))
            .one(&ctx.db)
            .await?
        {
            let _ = crate::services::maintenance::resource_cleanup::ResourceCleanupService::delete_monitor(
                &ctx.db, mon.id,
            )
            .await;
            let _ = metrics::Entity::delete_many()
                .filter(metrics_entity::Column::DeviceId.eq(device.id))
                .filter(metrics_entity::Column::Name.eq("memory_usage"))
                .exec(&ctx.db)
                .await;
        }
    }

    let selected_sensors: std::collections::BTreeSet<String> =
        options.monitored_sensors.into_iter().collect();

    for sensor in &scan.sensors {
        let is_monitored = selected_sensors.contains(&sensor.key);
        let mon_name = sensor_monitor_name(&sensor.name);
        let existing = monitors::Entity::find()
            .filter(monitors_entity::Column::DeviceId.eq(Some(device.id)))
            .filter(monitors_entity::Column::Name.eq(&mon_name))
            .one(&ctx.db)
            .await?;

        if is_monitored || existing.is_some() {
            sync_monitor(
                &ctx.db,
                MonitorSpec {
                    device_id: device.id,
                    interface: None,
                    name: &mon_name,
                    enabled: is_monitored,
                    configuration: serde_json::json!({
                        "host": config.host.clone(),
                        "version": version_name(config.version),
                        "community": config.community.clone(),
                        "port": config.port,
                        "metric": "sensor",
                        "sensorKey": sensor.key.clone(),
                        "oid": sensor.oid.clone(),
                        "scale": sensor.scale,
                        "unit": sensor.unit.clone(),
                        "dataType": sensor.data_type.clone(),
                        "icon": sensor.icon.clone(),
                        "color": sensor.color.clone(),
                        "states": sensor.states.clone(),
                    }),
                    up: sensor.value.is_some(),
                    interval_seconds: device.snmp_poll_interval_seconds,
                },
            )
            .await?;

            if !is_monitored && clear_history {
                metrics::Entity::delete_many()
                    .filter(metrics_entity::Column::DeviceId.eq(device.id))
                    .filter(metrics_entity::Column::Name.eq(&sensor.key))
                    .exec(&ctx.db)
                    .await?;
            }
        }
    }

    // Um poll inicial deixa a configuracao e a primeira visualizacao coerentes.
    let _ = poll_device(ctx, device, config).await;
    Ok(())
}

/// Liga ou desliga o monitoramento de **uma** interface já persistida.
///
/// São duas escritas que precisam andar juntas: o `admin_status` — a escolha do
/// operador, que o poll respeita (matriz de paridade #20) e o listador usa para
/// decidir o que aparece nas métricas — e o monitor `Interface X`, que é quem
/// faz o agendador coletar a porta. Separá-las era como as duas telas
/// divergiam: a interface aparecia marcada sem monitor por trás.
async fn set_monitoring(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    config: &SnmpConfig,
    interface: &device_interfaces::Model,
    enabled: bool,
    interval_seconds: i32,
    clear_history: bool,
) -> AppResult<()> {
    device_interfaces::ActiveModel {
        id: Set(interface.id),
        admin_status: Set(Some(if enabled { "up" } else { "down" }.into())),
        ..Default::default()
    }
    .update(db)
    .await?;
    sync_monitor(
        db,
        MonitorSpec {
            device_id,
            interface: Some(interface.id),
            name: &interface_monitor_name(&interface.name),
            enabled,
            configuration: serde_json::json!({
                "host": config.host.clone(),
                "version": version_name(config.version),
                "community": config.community.clone(),
                "port": config.port,
                "ifIndex": interface.snmp_index,
                "ifName": interface.name,
                "metric": "traffic",
            }),
            up: interface.oper_status.as_deref() == Some("up"),
            interval_seconds,
        },
    )
    .await?;
    if !enabled && clear_history {
        metrics::Entity::delete_many()
            .filter(metrics_entity::Column::InterfaceId.eq(Some(interface.id)))
            .exec(db)
            .await?;
    }
    Ok(())
}

/// Inclui ou remove uma interface do monitoramento sem refazer a descoberta.
///
/// A tela de descoberta continua existindo para achar portas novas; este
/// caminho é para quem já as tem listadas e só quer ligar mais uma — refazer a
/// varredura inteira (e reenviar a seleção completa) só para isso é o que
/// tornava a operação penosa.
pub async fn set_interface_monitoring(
    ctx: &loco_rs::app::AppContext,
    device: &devices::Model,
    config: SnmpConfig,
    interface_id: i64,
    enabled: bool,
) -> AppResult<()> {
    let interface = device_interfaces::Entity::find_by_id(interface_id)
        .one(&ctx.db)
        .await?
        .filter(|row| row.device_id == device.id)
        .ok_or_else(|| AppError::not_found("Interface não encontrada neste dispositivo"))?;
    if interface.snmp_index.is_none() {
        return Err(AppError::validation(
            "Interface sem índice SNMP: execute uma coleta antes de monitorá-la",
        ));
    }
    set_monitoring(
        &ctx.db,
        device.id,
        &config,
        &interface,
        enabled,
        device.snmp_poll_interval_seconds,
        true,
    )
    .await?;
    // Sem uma coleta agora o gráfico abriria vazio até o próximo ciclo do
    // agendador, e o operador leria isso como falha. Desligar não tem o que
    // coletar. Falha de coleta não invalida a escolha já gravada.
    if enabled {
        let _ = poll_device(ctx, device, config).await;
    }
    Ok(())
}

/// Uma interface como a aba "Interfaces SNMP" a lê: o registro local mais o que
/// só o banco sabe — se existe monitor habilitado para ela.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInterfaceView {
    pub id: i64,
    pub device_id: i64,
    pub snmp_index: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub alias: Option<String>,
    pub mac_address: Option<String>,
    pub speed: Option<i64>,
    pub admin_status: Option<String>,
    pub oper_status: Option<String>,
    pub is_monitored: bool,
}

/// Interfaces já conhecidas do equipamento.
///
/// Lê do banco de propósito: a listagem é carregada a cada abertura da tela de
/// detalhe, e resolvê-la com uma varredura SNMP ao vivo — como era antes —
/// custava um `walk` inteiro por visita e ainda devolvia registros sem `id`,
/// sem o qual nada consegue ligar a interface às suas métricas.
pub async fn list_interfaces(
    ctx: &loco_rs::app::AppContext,
    device_id: i64,
) -> AppResult<Vec<DeviceInterfaceView>> {
    let interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces_entity::Column::DeviceId.eq(device_id))
        .order_by_asc(device_interfaces_entity::Column::SnmpIndex)
        .all(&ctx.db)
        .await?;
    // Por `interface_id`: com o nome, duas linhas homônimas — o que sobra de
    // uma PPPoE que trocou de índice — se declaravam monitoradas pelo mesmo
    // monitor, e o painel mostrava a porta duas vezes, uma delas zerada.
    let monitored = monitors::Entity::find()
        .filter(monitors_entity::Column::DeviceId.eq(Some(device_id)))
        .filter(monitors_entity::Column::Enabled.eq(true))
        .all(&ctx.db)
        .await?
        .into_iter()
        .filter_map(|monitor| monitor.interface_id)
        .collect::<std::collections::BTreeSet<_>>();
    Ok(interfaces
        .into_iter()
        .map(|row| DeviceInterfaceView {
            is_monitored: monitored.contains(&row.id),
            id: row.id,
            device_id: row.device_id,
            snmp_index: row.snmp_index,
            name: row.name,
            description: row.description,
            alias: row.alias,
            mac_address: row.mac_address,
            speed: row.speed,
            admin_status: row.admin_status,
            oper_status: row.oper_status,
        })
        .collect())
}

fn monitor_configuration(config: &SnmpConfig, metric: &str) -> serde_json::Value {
    serde_json::json!({
        "host": config.host,
        "version": version_name(config.version),
        "community": config.community,
        "port": config.port,
        "metric": metric,
    })
}

/// Cria ou atualiza o monitor gerenciado de um dispositivo.
///
/// `interface` distingue os dois tipos que passam por aqui: o monitor de uma
/// porta, que pertence a uma linha de `device_interfaces`, e o monitor do
/// dispositivo inteiro (`cpu_usage`, `memory_usage`), que não pertence a
/// nenhuma.
///
/// Quando há interface, é o `interface_id` que localiza o monitor — não o nome.
/// A diferença aparece quando o operador renomeia a porta no equipamento: a
/// cadeia de identidade reencontra a linha, o monitor é achado pelo vínculo e
/// **renomeado junto**. Pela busca por nome, ele virava órfão e um segundo
/// monitor nascia para a mesma porta.
struct MonitorSpec<'a> {
    device_id: i64,
    /// `None` em monitor do dispositivo inteiro (`cpu_usage`, `memory_usage`).
    interface: Option<i64>,
    name: &'a str,
    enabled: bool,
    configuration: serde_json::Value,
    up: bool,
    interval_seconds: i32,
}

async fn sync_monitor(db: &sea_orm::DatabaseConnection, spec: MonitorSpec<'_>) -> AppResult<()> {
    let MonitorSpec {
        device_id,
        interface,
        name,
        enabled,
        configuration,
        up,
        interval_seconds,
    } = spec;
    let busca =
        monitors::Entity::find().filter(monitors_entity::Column::DeviceId.eq(Some(device_id)));
    let existing = match interface {
        Some(interface_id) => {
            busca
                .filter(monitors_entity::Column::InterfaceId.eq(Some(interface_id)))
                .one(db)
                .await?
        }
        None => {
            busca
                .filter(monitors_entity::Column::Name.eq(name))
                .one(db)
                .await?
        }
    };
    if let Some(existing) = existing {
        monitors::ActiveModel {
            id: Set(existing.id),
            // Acompanha o nome da porta no equipamento.
            name: Set(name.into()),
            interface_id: Set(interface),
            configuration: Set(configuration),
            interval_seconds: Set(interval_seconds),
            timeout_seconds: Set(calculate_smart_timeout_seconds("snmp", interval_seconds)
                .min((interval_seconds - 1).max(1))
                .max(1)),
            enabled: Set(enabled),
            status: Set(if up { "up" } else { "down" }.into()),
            ..Default::default()
        }
        .update(db)
        .await?;
    } else {
        monitors::ActiveModel {
            device_id: Set(Some(device_id)),
            interface_id: Set(interface),
            probe_id: Set(None),
            r#type: Set("snmp".into()),
            name: Set(name.into()),
            configuration: Set(configuration),
            interval_seconds: Set(interval_seconds),
            timeout_seconds: Set(calculate_smart_timeout_seconds("snmp", interval_seconds)
                .min((interval_seconds - 1).max(1))
                .max(1)),
            retry_count: Set(3),
            enabled: Set(enabled),
            status: Set(if up { "up" } else { "down" }.into()),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

fn version_name(version: SnmpVersion) -> &'static str {
    match version {
        SnmpVersion::V1 => "v1",
        SnmpVersion::V2c => "v2c",
        SnmpVersion::V3 => "v3",
    }
}

/// Uma interface persistida junto do estado que ela tinha antes da coleta.
///
/// O "antes" é lido aqui e não pelo chamador porque é a última janela em que
/// ele ainda existe: depois do `update` a linha já carrega o estado novo, e o
/// motor de alertas não teria como saber que houve transição.
pub struct SyncedInterface {
    pub interface: device_interfaces::Model,
    pub previous_oper_status: Option<String>,
    pub previous_speed: Option<i64>,
}

/// Espelha uma interface lida do agente na tabela local.
///
/// Público porque a matriz de paridade #20 — o `adminStatus` escolhido pelo
/// operador sobrevive ao poll — só se prova contra o banco.
///
/// Recebe a interface existente (quando já carregada em lote pelo chamador)
/// para evitar um `SELECT` por interface durante o poll.
pub async fn sync_interface(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    source: &SnmpInterface,
    existing: Option<&device_interfaces::Model>,
) -> AppResult<SyncedInterface> {
    let now = Utc::now();
    let admin_status = existing.and_then(|row| row.admin_status.clone());
    let previous_oper_status = existing.and_then(|row| row.oper_status.clone());
    let previous_speed = existing.and_then(|row| row.speed);
    let model = device_interfaces::ActiveModel {
        id: existing.map(|row| Set(row.id)).unwrap_or_default(),
        device_id: Set(device_id),
        snmp_index: Set(Some(source.if_index)),
        name: Set(source.if_name.clone()),
        description: Set(source.if_descr.clone()),
        alias: Set(source.if_alias.clone()),
        mac_address: Set(source.mac_address.clone()),
        r#type: Set(source.if_type.map(|kind| kind.to_string())),
        speed: Set(source.if_speed.and_then(|speed| i64::try_from(speed).ok())),
        // Uma escolha manual do usuário prevalece sobre o valor observado no poll.
        admin_status: Set(admin_status.or_else(|| {
            source
                .if_admin_status
                .map(|value| status_label(value).into())
        })),
        oper_status: Set(source
            .if_oper_status
            .map(|value| status_label(value).into())),
        last_seen_at: Set(Some(now.into())),
        ..Default::default()
    };
    let interface = if existing.is_some() {
        model.update(db).await?
    } else {
        model.insert(db).await?
    };
    Ok(SyncedInterface {
        interface,
        previous_oper_status,
        previous_speed,
    })
}

/// Constrói as métricas de tráfego de uma interface para inserção em massa.
///
/// Não grava no banco: apenas devolve os `PendingMetric` e a flag de reboot,
/// permitindo ao chamador um único `insert_many` por coleta SNMP.
fn build_traffic_metrics(
    device_id: i64,
    interface: &device_interfaces::Model,
    current: &InterfaceTraffic,
    previous_metrics: &mut HashMap<(i64, String), metrics::Model>,
    previous_uptime: Option<u64>,
    current_uptime: Option<u64>,
) -> (Vec<PendingMetric>, bool) {
    let previous_in = previous_metrics
        .get(&(interface.id, "ifHCInOctets".to_string()))
        .cloned();
    let previous_out = previous_metrics
        .get(&(interface.id, "ifHCOutOctets".to_string()))
        .cloned();
    let mut pending = Vec::with_capacity(6);
    for (name, value) in [
        ("ifHCInOctets", current.in_octets as f64),
        ("ifHCOutOctets", current.out_octets as f64),
        ("ifInErrors", current.in_errors as f64),
        ("ifOutErrors", current.out_errors as f64),
    ] {
        pending.push(PendingMetric {
            device_id,
            interface_id: Some(interface.id),
            name: name.into(),
            value,
            unit: "bytes".into(),
            recorded_at: current.recorded_at,
        });
    }
    let (Some(previous_in), Some(previous_out)) = (previous_in, previous_out) else {
        return (pending, false);
    };
    let previous = InterfaceTraffic {
        if_index: current.if_index,
        in_octets: previous_in.value.max(0.0) as u64,
        out_octets: previous_out.value.max(0.0) as u64,
        in_errors: 0,
        out_errors: 0,
        counter_bits: current.counter_bits,
        recorded_at: previous_in.recorded_at.with_timezone(&Utc),
    };
    let rates = crate::services::snmp::collectors::calculate_rates_detailed(
        &previous,
        current,
        previous_uptime,
        current_uptime,
    );
    if !rates.reboot_detected {
        pending.push(PendingMetric {
            device_id,
            interface_id: Some(interface.id),
            name: "inBps".into(),
            value: rates.in_bps,
            unit: "bps".into(),
            recorded_at: current.recorded_at,
        });
        pending.push(PendingMetric {
            device_id,
            interface_id: Some(interface.id),
            name: "outBps".into(),
            value: rates.out_bps,
            unit: "bps".into(),
            recorded_at: current.recorded_at,
        });
    }
    (pending, rates.reboot_detected)
}

/// Constrói as métricas de sistema (CPU, memória, uptime) para inserção em massa.
fn build_system_metrics(device_id: i64, scan: &SnmpScanResult) -> Vec<PendingMetric> {
    let recorded_at = Utc::now();
    let mut pending = Vec::with_capacity(3 + scan.sensors.len());
    for (name, value, unit) in [
        ("cpu_usage", scan.cpu_info.usage_percent, "percent"),
        ("memory_usage", scan.memory_info.used_percent, "percent"),
        (
            "snmp_uptime",
            scan.system_info.sys_up_time.map(|uptime| uptime as f64),
            "ticks",
        ),
    ] {
        if let Some(value) = value {
            pending.push(PendingMetric {
                device_id,
                interface_id: None,
                name: name.into(),
                value,
                unit: unit.into(),
                recorded_at,
            });
        }
    }
    for sensor in &scan.sensors {
        if let Some(value) = sensor.value {
            pending.push(PendingMetric {
                device_id,
                interface_id: None,
                name: sensor.key.clone(),
                value,
                unit: sensor.unit.clone(),
                recorded_at,
            });
        }
    }
    pending
}

/// Busca a métrica mais recente de cada `(interface_id, nome)` em uma única
/// query, eliminando o N+1 do loop de contadores SNMP.
pub async fn latest_metrics_for_interfaces(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    interface_ids: &[i64],
    names: &[&str],
) -> AppResult<HashMap<(i64, String), metrics::Model>> {
    if interface_ids.is_empty() || names.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = metrics_repository::latest_for_interfaces(db, Some(device_id), interface_ids, names)
        .await?;
    let mut map = HashMap::new();
    for row in rows {
        if let Some(interface_id) = row.interface_id {
            let key = (interface_id, row.name.clone());
            map.entry(key).or_insert(row);
        }
    }
    Ok(map)
}

async fn latest_device_metric(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    name: &str,
) -> AppResult<Option<metrics::Model>> {
    Ok(metrics::Entity::find()
        .filter(metrics_entity::Column::DeviceId.eq(device_id))
        .filter(metrics_entity::Column::InterfaceId.is_null())
        .filter(metrics_entity::Column::Name.eq(name))
        .order_by_desc(metrics_entity::Column::RecordedAt)
        .one(db)
        .await?)
}

/// Métrica pronta para inserção, usada para acumular várias leituras antes de
/// um único `insert_many` — evita o N+1 de gravação no poll SNMP.
#[derive(Debug, Clone)]
struct PendingMetric {
    device_id: i64,
    interface_id: Option<i64>,
    name: String,
    value: f64,
    unit: String,
    recorded_at: chrono::DateTime<Utc>,
}

impl PendingMetric {
    fn into_active_model(self) -> metrics::ActiveModel {
        metrics::ActiveModel {
            device_id: Set(self.device_id),
            interface_id: Set(self.interface_id),
            monitor_id: Set(None),
            name: Set(self.name),
            value: Set(self.value),
            unit: Set(self.unit),
            recorded_at: Set(self.recorded_at.into()),
            ..Default::default()
        }
    }
}

/// Grava um lote de métricas em uma única operação de inserção em massa.
///
/// SQLite e PostgreSQL suportam `INSERT` multi-row; o `sea_orm` emite a forma
/// correta para cada dialeto. O retorno é a quantidade de itens enviados.
async fn record_metrics_bulk(
    db: &sea_orm::DatabaseConnection,
    metrics: Vec<PendingMetric>,
) -> AppResult<usize> {
    if metrics.is_empty() {
        return Ok(0);
    }
    let count = metrics.len();
    let active_models: Vec<metrics::ActiveModel> = metrics
        .into_iter()
        .map(PendingMetric::into_active_model)
        .collect();
    metrics::Entity::insert_many(active_models)
        .exec_without_returning(db)
        .await?;
    Ok(count)
}

pub async fn detect_connection(
    host: &str,
    port: u16,
    preferred: Option<SnmpConfig>,
) -> AppResult<SnmpDetectResult> {
    let started = std::time::Instant::now();
    let mut candidates = Vec::new();
    if let Some(mut config) = preferred {
        config.host = host.to_string();
        config.port = port;
        candidates.push(config);
    }
    let timeout_ms = std::env::var("SNMP_DISCOVERY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(800)
        .clamp(100, 5_000);
    let communities =
        discovery_communities(std::env::var("SNMP_DISCOVERY_COMMUNITIES").ok().as_deref());
    for (version, community) in
        [SnmpVersion::V2c, SnmpVersion::V1]
            .into_iter()
            .flat_map(|version| {
                communities
                    .iter()
                    .map(move |community| (version, community))
            })
    {
        if !candidates.iter().any(|config| {
            config.version == version && config.community.as_str() == community.as_str()
        }) {
            let mut config = SnmpConfig::v2c(host, community, port);
            config.version = version;
            config.timeout_ms = timeout_ms;
            candidates.push(config);
        }
    }
    candidates.extend(discovery_v3_profiles(host, port, timeout_ms));
    let candidate_count = candidates.len();
    let attempts = candidates.into_iter().map(|config| async move {
        let label = (
            config.version,
            (config.version != SnmpVersion::V3).then(|| config.community.clone()),
        );
        test_connection(config).await.map(|result| (label, result))
    });
    let outcomes = future::join_all(attempts).await;
    // A ordem das tentativas é a de preferência, mas quem vence é quem
    // respondeu: pegar o primeiro `Ok` devolvia "não detectado" quando a
    // combinação preferida falava com o agente sem entregar OID nenhum, mesmo
    // havendo outra combinação logo atrás que funcionava.
    let mut answered = None;
    let mut fallback = None;
    for outcome in outcomes.into_iter().flatten() {
        if outcome.1.success {
            answered = Some(outcome);
            break;
        }
        fallback.get_or_insert(outcome);
    }
    let detected = answered.or(fallback);
    tracing::info!(
        target = host,
        port,
        attempts = candidate_count,
        success = detected.as_ref().is_some_and(|outcome| outcome.1.success),
        duration_ms = started.elapsed().as_millis(),
        "detecção SNMP concluída"
    );
    match detected {
        Some(((version, community), result)) => Ok(SnmpDetectResult {
            detected: result.success,
            version: Some(version_label(version).into()),
            community,
            result: Some(result),
        }),
        None => Ok(SnmpDetectResult {
            detected: false,
            version: None,
            community: None,
            result: None,
        }),
    }
}

fn discovery_communities(value: Option<&str>) -> Vec<String> {
    let mut communities = value
        .unwrap_or("public")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    communities.sort();
    communities.dedup();
    communities
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveryV3Profile {
    username: String,
    auth_protocol: Option<String>,
    auth_key: Option<String>,
    priv_protocol: Option<String>,
    priv_key: Option<String>,
}

fn discovery_v3_profiles(host: &str, port: u16, timeout_ms: u64) -> Vec<SnmpConfig> {
    let Some(raw) = std::env::var("SNMP_DISCOVERY_V3_PROFILES")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Vec::new();
    };
    let profiles = match serde_json::from_str::<Vec<DiscoveryV3Profile>>(&raw) {
        Ok(profiles) => profiles,
        Err(error) => {
            tracing::warn!(%error, "SNMP_DISCOVERY_V3_PROFILES inválido; perfis ignorados");
            return Vec::new();
        }
    };
    profiles
        .into_iter()
        .filter(|profile| !profile.username.trim().is_empty())
        .map(|profile| SnmpConfig {
            host: host.to_string(),
            version: SnmpVersion::V3,
            community: String::new(),
            username: Some(profile.username),
            auth_protocol: profile.auth_protocol,
            auth_key: profile.auth_key,
            priv_protocol: profile.priv_protocol,
            priv_key: profile.priv_key,
            port,
            timeout_ms,
        })
        .collect()
}
const fn version_label(version: SnmpVersion) -> &'static str {
    match version {
        SnmpVersion::V1 => "v1",
        SnmpVersion::V2c => "v2c",
        SnmpVersion::V3 => "v3",
    }
}
fn map_error(error: SnmpError) -> AppError {
    AppError::BusinessRule(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linha_de_teste(id: i64, snmp_index: Option<i32>, name: &str) -> device_interfaces::Model {
        let agora = Utc::now().into();
        device_interfaces::Model {
            id,
            device_id: 1,
            snmp_index,
            name: name.into(),
            description: None,
            alias: None,
            mac_address: None,
            r#type: None,
            speed: None,
            admin_status: None,
            oper_status: None,
            last_seen_at: Some(agora),
            created_at: agora,
            updated_at: agora,
        }
    }

    fn linha_completa(
        id: i64,
        snmp_index: Option<i32>,
        name: &str,
        alias: Option<&str>,
        mac: Option<&str>,
    ) -> device_interfaces::Model {
        let mut row = linha_de_teste(id, snmp_index, name);
        row.alias = alias.map(Into::into);
        row.mac_address = mac.map(Into::into);
        row
    }

    fn origem(
        if_index: i32,
        if_name: &str,
        if_alias: Option<&str>,
        mac_address: Option<&str>,
    ) -> SnmpInterface {
        let mut src = interface_de_teste(if_index, if_name);
        src.if_alias = if_alias.map(Into::into);
        src.mac_address = mac_address.map(Into::into);
        src
    }

    fn casa(
        linhas: Vec<device_interfaces::Model>,
        varredura: Vec<SnmpInterface>,
    ) -> Vec<Option<i64>> {
        let mut matcher = InterfaceMatcher::resolve(linhas, &varredura);
        (0..varredura.len())
            .map(|pos| matcher.take(pos).map(|row| row.id))
            .collect()
    }

    /// O caso PPPoE: o nome e o mesmo, o indice mudou na reconexao. Sem
    /// reencontrar a linha, a sincronizacao inseriria uma segunda `pppoe-wan` e
    /// deixaria a primeira orfa para sempre.
    #[test]
    fn interface_renumerada_casa_pelo_nome() {
        let casamento = casa(
            vec![linha_de_teste(17, Some(34), "pppoe-wan")],
            vec![interface_de_teste(55, "pppoe-wan")],
        );
        assert_eq!(casamento, vec![Some(17)]);
    }

    /// Indice liberado e reaproveitado por outra porta no boot seguinte. Com o
    /// casamento por indice na frente, a linha da `pppoe-wan` era renomeada
    /// para `br-lan` e o historico continuava colado num registro que passou a
    /// descrever outra coisa.
    #[test]
    fn indice_reaproveitado_nao_sequestra_a_linha_de_outra_interface() {
        let casamento = casa(
            vec![
                linha_de_teste(17, Some(34), "pppoe-wan"),
                linha_de_teste(10, Some(11), "br-lan"),
            ],
            vec![interface_de_teste(34, "br-lan")],
        );
        assert_eq!(casamento, vec![Some(10)], "a br-lan, nao a pppoe-wan");
    }

    /// `ifAlias` e o que a RFC 2863 obriga o agente a preservar atraves de
    /// reboots, e e o operador quem o escreve. Quando existe, vence o nome —
    /// que o proprio operador pode ter mudado no equipamento.
    #[test]
    fn alias_vence_o_nome_quando_a_porta_foi_renomeada() {
        let casamento = casa(
            vec![
                linha_completa(1, Some(5), "lan1", Some("Uplink Matriz"), None),
                linha_completa(2, Some(6), "uplink", None, None),
            ],
            vec![origem(5, "uplink", Some("Uplink Matriz"), None)],
        );
        assert_eq!(
            casamento,
            vec![Some(1)],
            "o alias identifica a porta renomeada, nao a homonima do novo nome"
        );
    }

    /// A resolucao e sinal a sinal sobre o conjunto inteiro. Se fosse interface
    /// a interface, a `eth0` casaria por nome com a linha 1 antes de a `wan`
    /// poder reivindica-la pelo alias, que e o sinal mais forte.
    #[test]
    fn a_ancora_forte_e_resolvida_antes_da_fraca() {
        let casamento = casa(
            vec![
                linha_completa(1, Some(1), "eth0", Some("WAN Fibra"), None),
                linha_completa(2, Some(9), "eth1", None, None),
            ],
            vec![
                origem(9, "eth0", None, None),
                origem(1, "wan", Some("WAN Fibra"), None),
            ],
        );
        // Interface a interface, a `eth0` casaria pelo nome com a linha 1 e
        // roubaria a que o alias da `wan` reivindica depois — resultado
        // `[Some(1), None]`, com a WAN perdendo o historico.
        assert_eq!(
            casamento,
            vec![Some(2), Some(1)],
            "o alias resolve antes; a eth0 fica com a linha que sobrou, pelo indice"
        );
    }

    /// Agente que reporta duas portas com o mesmo `ifName`: o nome nao
    /// identifica ninguem e casar por ele faria as duas disputarem uma linha.
    #[test]
    fn nome_repetido_no_aparelho_cai_para_o_indice() {
        let casamento = casa(
            vec![
                linha_de_teste(1, Some(5), "lan"),
                linha_de_teste(2, Some(6), "lan"),
            ],
            vec![interface_de_teste(6, "lan"), interface_de_teste(5, "lan")],
        );
        assert_eq!(
            casamento,
            vec![Some(2), Some(1)],
            "cada uma pelo seu indice"
        );
    }

    /// Ambiguidade do lado da varredura conta igual. Exigir unicidade so no
    /// banco deixava o desempate na ordem de iteracao: com duas `lan` chegando
    /// e uma no banco, ficava com a linha quem chegasse primeiro.
    #[test]
    fn nome_repetido_na_varredura_tambem_descarta_o_sinal() {
        let casamento = casa(
            vec![linha_de_teste(1, Some(5), "lan")],
            vec![interface_de_teste(9, "lan"), interface_de_teste(5, "lan")],
        );
        assert_eq!(
            casamento,
            vec![None, Some(1)],
            "o nome e descartado; quem casa e a que bate o indice"
        );
    }

    /// Num roteador Linux as bridges e VLANs herdam o MAC da porta fisica. Nos
    /// dados de producao deste projeto, 42 interfaces tem MAC e so 18 desses
    /// valores sao unicos — por isso ele e desempate tardio, e so quando unico.
    #[test]
    fn mac_compartilhado_entre_bridges_nao_identifica() {
        let compartilhado = Some("d6:8c:72:59:65:fc");
        let casamento = casa(
            vec![
                linha_completa(1, Some(9), "sfp2", None, compartilhado),
                linha_completa(2, Some(11), "br-lan", None, compartilhado),
            ],
            vec![origem(11, "br-lan", None, compartilhado)],
        );
        assert_eq!(casamento, vec![Some(2)], "quem decide e o nome, nao o MAC");
    }

    /// MAC zerado e o que o agente devolve para interface sem endereco
    /// (loopback, tunel, PPP). Trata-lo como sinal juntaria todas numa so.
    #[test]
    fn mac_zerado_nao_e_sinal() {
        let zerado = Some("00:00:00:00:00:00");
        let casamento = casa(
            vec![linha_completa(1, Some(1), "lo", None, zerado)],
            vec![origem(99, "tun0", None, zerado)],
        );
        assert_eq!(casamento, vec![None], "portas diferentes seguem diferentes");
    }

    /// Cada linha vale por uma varredura: duas interfaces nao podem colapsar
    /// numa so.
    #[test]
    fn uma_linha_so_e_reivindicada_uma_vez() {
        let casamento = casa(
            vec![linha_de_teste(17, Some(34), "pppoe-wan")],
            vec![
                interface_de_teste(55, "pppoe-wan"),
                interface_de_teste(34, "pppoe-wan"),
            ],
        );
        assert_eq!(
            casamento.iter().filter(|item| item.is_some()).count(),
            1,
            "so uma das duas leva a linha; a outra vira registro novo"
        );
    }

    /// Porta que o dispositivo nunca reportou nao casa com nada.
    #[test]
    fn interface_desconhecida_nao_casa() {
        let casamento = casa(
            vec![linha_de_teste(1, Some(5), "lan")],
            vec![interface_de_teste(99, "wan-nova")],
        );
        assert_eq!(casamento, vec![None]);
    }

    fn interface_de_teste(if_index: i32, if_name: &str) -> SnmpInterface {
        SnmpInterface {
            if_index,
            if_name: if_name.into(),
            if_descr: None,
            if_alias: None,
            if_type: None,
            if_speed: None,
            if_admin_status: Some(1),
            if_oper_status: Some(1),
            mac_address: None,
            is_monitored: false,
        }
    }

    /// Agente mudo não é equipamento sem interfaces. Antes da guarda este
    /// cenário chegava ao laço de limpeza com a lista vazia e apagava monitor,
    /// métricas e interfaces de tudo que estava registrado.
    #[test]
    fn varredura_sem_resposta_nao_autoriza_remocao() {
        let scan = SnmpScanResult::default();
        let erro = ensure_scan_can_remove(&scan, 21).expect_err("deveria recusar");
        assert_eq!(erro.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    /// O caso real: `sysDescr` responde, o walk do `ifTable` perde um PDU e
    /// `scan` devolve `Ok` com a lista vazia e o erro em `collector_errors`.
    #[test]
    fn falha_do_coletor_de_interfaces_nao_autoriza_remocao() {
        let mut scan = SnmpScanResult {
            snmp_responded: true,
            ..Default::default()
        };
        scan.collector_errors
            .insert(INTERFACES_COLLECTOR.into(), "tempo esgotado".into());
        let erro = ensure_scan_can_remove(&scan, 21).expect_err("deveria recusar");
        assert_eq!(erro.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    /// Rede de segurança para a falha que não vira erro de coletor: nenhum
    /// equipamento perde todas as interfaces de uma vez.
    #[test]
    fn varredura_vazia_com_interfaces_conhecidas_nao_autoriza_remocao() {
        let scan = SnmpScanResult {
            snmp_responded: true,
            ..Default::default()
        };
        let erro = ensure_scan_can_remove(&scan, 21).expect_err("deveria recusar");
        assert_eq!(erro.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    /// Primeira configuração de um equipamento sem interface nenhuma no banco:
    /// não há o que proteger, e recusar aqui travaria o cadastro inicial.
    #[test]
    fn varredura_vazia_sem_interfaces_conhecidas_e_permitida() {
        let scan = SnmpScanResult {
            snmp_responded: true,
            ..Default::default()
        };
        assert!(ensure_scan_can_remove(&scan, 0).is_ok());
    }

    /// Coleta boa continua passando — inclusive com erro de um coletor que não
    /// é o de interfaces, que não tem nada a ver com remover porta.
    #[test]
    fn varredura_com_interfaces_autoriza_remocao() {
        let mut scan = SnmpScanResult {
            snmp_responded: true,
            interfaces: vec![interface_de_teste(1, "eth0")],
            ..Default::default()
        };
        scan.collector_errors
            .insert("cpu".into(), "sem OID de CPU".into());
        assert!(ensure_scan_can_remove(&scan, 21).is_ok());
    }

    /// O diálogo "Escaneamento & Descoberta SNMP" lê estes campos direto do
    /// JSON. Quando eles saíam como `system`/`cpu`/`memory`, a tela quebrava em
    /// `Cannot read properties of undefined` antes de renderizar qualquer coisa.
    #[test]
    fn a_varredura_sai_com_os_campos_que_a_tela_de_descoberta_le() {
        let scan = SnmpScanResult {
            snmp_responded: true,
            system_info: SnmpSystemInfo::default(),
            interfaces: vec![SnmpInterface {
                if_index: 1,
                if_name: "eth0".into(),
                if_descr: None,
                if_alias: None,
                if_type: None,
                if_speed: None,
                if_admin_status: Some(1),
                if_oper_status: Some(1),
                mac_address: None,
                is_monitored: true,
            }],
            traffic: vec![],
            cpu_info: SnmpCpuInfo::default(),
            memory_info: SnmpMemoryInfo::default(),
            neighbors: vec![],
            collector_errors: Default::default(),
            has_cpu_monitor: true,
            has_memory_monitor: false,
            matched_profile: None,
            sensors: vec![],
        };
        let json = serde_json::to_value(&scan).unwrap();

        for campo in [
            "snmpResponded",
            "systemInfo",
            "cpuInfo",
            "memoryInfo",
            "interfaces",
            "hasCpuMonitor",
            "hasMemoryMonitor",
        ] {
            assert!(json.get(campo).is_some(), "campo ausente: {campo}");
        }
        assert_eq!(json["interfaces"][0]["isMonitored"], true);
        assert_eq!(json["hasCpuMonitor"], true);
    }

    #[test]
    fn resultado_consolidado_respeita_o_estado_da_interface() {
        assert_eq!(interface_monitor_status(2, 2), MonitorStatus::Disabled);
        assert_eq!(interface_monitor_status(1, 1), MonitorStatus::Up);
        assert_eq!(interface_monitor_status(1, 2), MonitorStatus::Down);
        assert_eq!(interface_monitor_status(1, 7), MonitorStatus::Warning);
    }

    #[test]
    fn resolve_if_index_por_nome_quando_indice_pppoe_migra() {
        let monitor = monitors::Model {
            id: 19,
            interface_id: None,
            name: "Interface pppoe-wan".into(),
            r#type: "snmp".into(),
            configuration: serde_json::json!({
                "ifIndex": 19, // índice antigo no monitor
                "ifName": "pppoe-wan",
                "metric": "traffic",
            }),
            device_id: Some(1),
            probe_id: None,
            interval_seconds: 15,
            timeout_seconds: 5,
            retry_count: 3,
            enabled: true,
            status: "down".into(),
            last_run_at: None,
            next_run_at: None,
            created_at: Utc::now().fixed_offset(),
            updated_at: Utc::now().fixed_offset(),
        };
        let poll = SnmpPollResult {
            scan: SnmpScanResult {
                snmp_responded: true,
                system_info: SnmpSystemInfo::default(),
                interfaces: vec![SnmpInterface {
                    if_index: 24, // novo índice no roteador
                    if_name: "pppoe-wan".into(),
                    if_descr: Some("pppoe-wan interface".into()),
                    if_alias: None,
                    if_type: None,
                    if_speed: None,
                    if_admin_status: Some(1),
                    if_oper_status: Some(1),
                    mac_address: None,
                    is_monitored: true,
                }],
                traffic: vec![InterfaceTraffic {
                    if_index: 24,
                    in_octets: 1000,
                    out_octets: 2000,
                    in_errors: 0,
                    out_errors: 0,
                    counter_bits: 64,
                    recorded_at: Utc::now(),
                }],
                cpu_info: SnmpCpuInfo::default(),
                memory_info: SnmpMemoryInfo::default(),
                neighbors: vec![],
                collector_errors: Default::default(),
                has_cpu_monitor: false,
                has_memory_monitor: false,
                matched_profile: None,
                sensors: vec![],
            },
            interfaces_synced: 1,
            metrics_recorded: 1,
            links_resolved: 0,
            reboot_detected: false,
        };

        let resolved = resolve_monitor_if_index(&monitor, &poll);
        assert_eq!(resolved, Some(24));

        let result = monitor_result_from_poll(&monitor, &poll, Utc::now(), Utc::now());
        assert_eq!(result.status, MonitorStatus::Up);
        assert!(result.success);
    }
}
