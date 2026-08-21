//! Cadastro de dispositivos e recursos dependentes.

use std::collections::HashMap;

use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use loco_rs::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ExprTrait, QueryFilter, QueryOrder, QuerySelect,
    Set,
};

use crate::{
    dtos::{
        devices::{DeviceEventItem, DeviceMetricItem, DevicePresenterItem, ParentRef, SiteRef},
        resources::{DeviceInput, PaginationQuery},
    },
    models::{
        _entities::{
            alert_events, device_interfaces, discovery_results, metrics as metrics_entity,
            vpn_peers,
        },
        devices, monitors, sites,
    },
    services::{
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        devices::{
            access::{AccessContext, AccessMode},
            capabilities,
            system_device::{self, ProposedIdentity},
            systems,
        },
        maintenance::resource_cleanup::ResourceCleanupService,
        monitoring::{
            presenter::{present_monitors, RECENT_RESULTS_LIMIT},
            reachability,
        },
        preferences,
        shared::{
            errors::{AppError, AppResult},
            pagination::{paginate_compat, MaybePaged},
        },
        snmp::service::{sync_monitor_intervals, DEFAULT_SNMP_POLL_INTERVAL_SECONDS},
        syslog::hints,
    },
    views::vpn::VpnPeerResponse,
};

/// Serialização canônica de um dispositivo para a API.
///
/// É `pub(crate)` porque as telas de VPN devolvem o mesmo objeto dentro de
/// `{peer, device}` (§7.13). Duplicar a lista de campos lá faria as duas
/// versões divergirem no primeiro campo novo — e o `Model` do `sea-orm`
/// serializa em `snake_case`, o que quebraria o contrato camelCase da §5.1.
pub(crate) async fn present(
    db: &sea_orm::DatabaseConnection,
    device: devices::Model,
) -> AppResult<DevicePresenterItem> {
    present_many(db, vec![device], VpnLink::Include)
        .await?
        .pop()
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))
}

/// Serializa um dispositivo **sem** o vínculo de VPN.
///
/// É o que as telas de VPN usam: lá o peer já é o contexto que envolve o
/// dispositivo, e repeti-lo dentro dele seria devolver a mesma informação duas
/// vezes em profundidades diferentes — além de custar uma consulta por linha
/// para produzir o que a tela não lê. O `VpnPeerDeviceView` dos bindings nunca
/// declarou este campo; até aqui ele viajava como um `null` que ninguém pediu.
pub(crate) async fn present_for_vpn(
    db: &sea_orm::DatabaseConnection,
    device: devices::Model,
) -> AppResult<DevicePresenterItem> {
    present_many(db, vec![device], VpnLink::Omit)
        .await?
        .pop()
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))
}

/// Se o objeto devolvido carrega o peer de VPN do dispositivo.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VpnLink {
    Include,
    Omit,
}

/// Serializa vários dispositivos com um número **fixo** de consultas.
///
/// A versão anterior chamava `present` dentro de um `for`, e cada chamada abria
/// até duas consultas pontuais (site e pai). Com 300 dispositivos isso era mais
/// de 600 idas ao banco, em série, para montar uma lista. Aqui são três, com
/// `IN (…)`: sites, pais e peers.
pub(crate) async fn present_many(
    db: &sea_orm::DatabaseConnection,
    rows: Vec<devices::Model>,
    vpn: VpnLink,
) -> AppResult<Vec<DevicePresenterItem>> {
    use std::collections::HashSet;

    let site_ids: HashSet<i64> = rows.iter().filter_map(|row| row.site_id).collect();
    let parent_ids: HashSet<i64> = rows.iter().filter_map(|row| row.parent_id).collect();
    let device_ids: Vec<i64> = rows.iter().map(|row| row.id).collect();

    let sites: HashMap<i64, String> = if site_ids.is_empty() {
        HashMap::new()
    } else {
        sites::Entity::find()
            .filter(sites::Column::Id.is_in(site_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|row| (row.id, row.name))
            .collect()
    };
    let parents: HashMap<i64, String> = if parent_ids.is_empty() {
        HashMap::new()
    } else {
        devices::Entity::find()
            .filter(devices::Column::Id.is_in(parent_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|row| (row.id, row.name))
            .collect()
    };
    let peers: HashMap<i64, vpn_peers::Model> = if vpn == VpnLink::Omit || device_ids.is_empty() {
        HashMap::new()
    } else {
        vpn_peers::Entity::find()
            .filter(vpn_peers::Column::DeviceId.is_in(device_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|row| (row.device_id, row))
            .collect()
    };
    // Três consultas fixas, independentes do tamanho da lista — pelo mesmo
    // motivo das de cima. A forma de acesso deduzida acompanha todo dispositivo
    // porque é ela que a tela mostra na opção "Automático": esconder a conclusão
    // do sistema faria o operador declarar no escuro.
    let acessos = AccessContext::load(db).await?;

    Ok(rows
        .into_iter()
        .map(|device| {
            let mut item = corpo(
                &device,
                device.site_id.and_then(|id| sites.get(&id)),
                device.parent_id.and_then(|id| parents.get(&id)),
                &acessos,
            );
            if vpn == VpnLink::Include {
                let peer_val = peers
                    .get(&device.id)
                    .map_or(serde_json::Value::Null, |row| {
                        serde_json::to_value(VpnPeerResponse::from(row))
                            .unwrap_or(serde_json::Value::Null)
                    });
                item.vpn_peer = Some(peer_val);
            }
            item
        })
        .collect())
}

fn corpo(
    device: &devices::Model,
    site_name: Option<&String>,
    parent_name: Option<&String>,
    acessos: &AccessContext,
) -> DevicePresenterItem {
    let site = site_name.cloned().map(|name| SiteRef {
        id: device.site_id,
        name,
    });
    let parent = parent_name.cloned().map(|name| ParentRef {
        id: device.parent_id,
        name,
    });
    // Três campos, e os três precisam existir separados: `accessMode` é o que o
    // operador declarou (nulo quando ele escolheu "automático"), o `effective` é
    // o que o sistema vai usar de fato, e o `reason` é por quê. Devolver só o
    // efetivo faria a tela mostrar uma dedução no lugar onde o operador espera
    // ver a própria escolha — e ele nunca conseguiria voltar para o automático,
    // porque não teria como saber que já não estava nele.
    let acesso = acessos.resolve(device);
    // Mesma separação do acesso: `operatingSystem` é o que o operador declarou
    // (nulo no automático) e `effectiveOperatingSystem` é o que vale hoje. A
    // dedução aqui é a barata — do texto livre do cadastro, sem rede: consultar
    // o `sysDescr` por SNMP de cada linha transformaria listar dispositivos numa
    // varredura. Quem precisa da versão com SNMP é a tela de ativação de log, e
    // ela pede um dispositivo de cada vez.
    let sistema = systems::detect(&systems::Evidence {
        declared: device.operating_system.as_deref(),
        vendor: device.vendor.as_deref(),
        model: device.model.as_deref(),
        ..systems::Evidence::default()
    });
    DevicePresenterItem {
        id: device.id,
        site_id: device.site_id,
        network_id: device.network_id,
        parent_id: device.parent_id,
        ip_address: device.ip_address.clone(),
        name: device.name.clone(),
        device_type: device.r#type.clone(),
        vendor: device.vendor.clone(),
        model: device.model.clone(),
        serial_number: device.serial_number.clone(),
        description: device.description.clone(),
        is_monitored: device.is_monitored,
        snmp_enabled: device.snmp_enabled,
        snmp_community: device.snmp_community.clone(),
        snmp_version: device.snmp_version.clone(),
        snmp_poll_interval_seconds: device.snmp_poll_interval_seconds,
        status: device.status.clone(),
        access_mode: device.access_mode.clone(),
        effective_access_mode: acesso.mode.id().to_string(),
        access_mode_reason: acesso.reason,
        access_mode_declared: acesso.declared,
        operating_system: device.operating_system.clone(),
        effective_operating_system: sistema.system.id.to_string(),
        operating_system_source: sistema.source.to_string(),
        operating_system_reason: sistema.reason,
        last_seen_at: device.last_seen_at.map(|v| v.to_rfc3339()),
        created_at: device.created_at.to_rfc3339(),
        updated_at: device.updated_at.to_rfc3339(),
        site,
        parent,
        system_key: device.system_key.clone(),
        is_system: system_device::is_protected(device),
        vpn_peer: None,
    }
}

/// Lê a forma de acesso vinda da tela.
fn access_mode_declarado(bruto: Option<&str>, atual: Option<String>) -> AppResult<Option<String>> {
    let Some(texto) = bruto else {
        return Ok(atual);
    };
    Ok(AccessMode::parse(texto)
        .map_err(AppError::validation)?
        .map(|modo| modo.id().to_owned()))
}

/// Mesma semântica de [`access_mode_declarado`], para o sistema do equipamento.
fn sistema_declarado(bruto: Option<&str>, atual: Option<String>) -> AppResult<Option<String>> {
    let Some(texto) = bruto else {
        return Ok(atual);
    };
    Ok(systems::parse(texto)
        .map_err(AppError::validation)?
        .map(|sistema| sistema.id.to_owned()))
}

fn require_name_type(input: &DeviceInput) -> AppResult<(&str, &str)> {
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| AppError::validation("Nome do dispositivo é obrigatório"))?;
    if name.contains('\n') || name.contains('\r') {
        return Err(AppError::validation(
            "Nome do dispositivo não pode conter quebra de linha",
        ));
    }
    let kind = input
        .device_type
        .as_deref()
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .ok_or_else(|| AppError::validation("Tipo do dispositivo é obrigatório"))?;
    if kind.contains('\n') || kind.contains('\r') {
        return Err(AppError::validation(
            "Tipo do dispositivo não pode conter quebra de linha",
        ));
    }
    if let Some(ip) = input
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|ip| !ip.is_empty())
    {
        if ip.parse::<std::net::IpAddr>().is_err() {
            return Err(AppError::validation(format!(
                "Endereço IP inválido: '{ip}'"
            )));
        }
    }
    Ok((name, kind))
}

async fn sync_device_monitor(
    db: &sea_orm::DatabaseConnection,
    device: &devices::Model,
) -> AppResult<()> {
    let existing = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device.id))
        .filter(monitors::Column::Type.eq("ping"))
        .one(db)
        .await?;
    // Quem decide é o domínio, não este controller: o dispositivo do sistema
    // não é alcançado pela rede, e um dispositivo sem endereço não tem alvo a
    // checar. Nos dois casos, um ping provisionado aqui só poderia falhar —
    // era daqui que nascia o monitor com o **nome exibido** como host.
    let alvo = reachability::auto_target(device)
        .filter(|_| reachability::ensure_allowed_for_device(device, "ping").is_ok());
    if device.is_monitored {
        let Some(host) = alvo else {
            // Sem alvo válido, um ping preexistente para de checar em vez de
            // seguir marcando o equipamento como offline por um alvo inventado.
            if let Some(row) = existing {
                let mut active: monitors::ActiveModel = row.into();
                active.enabled = Set(false);
                active.update(db).await?;
            }
            return Ok(());
        };
        let configuration = serde_json::json!({ "host": host });
        if let Some(row) = existing {
            let mut active: monitors::ActiveModel = row.into();
            active.enabled = Set(true);
            active.name = Set(format!("Ping {}", device.name));
            active.configuration = Set(configuration);
            active.update(db).await?;
        } else {
            monitors::ActiveModel {
                device_id: Set(Some(device.id)),
                r#type: Set("ping".into()),
                name: Set(format!("Ping {}", device.name)),
                configuration: Set(configuration),
                interval_seconds: Set(60),
                timeout_seconds: Set(10),
                retry_count: Set(3),
                enabled: Set(true),
                status: Set("unknown".into()),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    } else if let Some(row) = existing {
        let mut active: monitors::ActiveModel = row.into();
        active.enabled = Set(false);
        active.update(db).await?;
    }
    Ok(())
}

async fn index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let rows = devices::Entity::find()
        .order_by_asc(devices::Column::Name)
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        present_many(&ctx.db, rows, VpnLink::Include).await?,
    )?)
}

async fn store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<DeviceInput>,
) -> AppResult<Response> {
    let (name, kind) = require_name_type(&input)?;
    let name = name.to_string();
    let kind = kind.to_string();
    // A comunidade padrão é gravada **no dispositivo**, e não aplicada só na
    // hora de coletar. Duas razões: ela fica visível no cadastro em vez de
    // depender de um valor global invisível, e mudar a preferência depois não
    // repassa em silêncio a comunidade de equipamentos que já estavam
    // funcionando — o que quebraria a coleta deles sem aviso.
    let snmp_enabled = input.snmp_enabled.unwrap_or(false);
    let snmp_community = match input
        .snmp_community
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
    {
        Some(valor) => Some(valor.to_owned()),
        None if snmp_enabled => Some(preferences::load(&ctx.db).await?.default_snmp_community),
        None => None,
    };
    let row = devices::ActiveModel {
        site_id: Set(input.site_id),
        network_id: Set(input.network_id),
        parent_id: Set(input.parent_id),
        ip_address: Set(input.ip_address),
        name: Set(name),
        r#type: Set(kind),
        vendor: Set(input.vendor),
        model: Set(input.model),
        serial_number: Set(input.serial_number),
        description: Set(input.description),
        is_monitored: Set(input.is_monitored.unwrap_or(false)),
        snmp_enabled: Set(snmp_enabled),
        snmp_community: Set(snmp_community),
        snmp_version: Set(input.snmp_version),
        snmp_poll_interval_seconds: Set(std::cmp::max(
            input
                .snmp_poll_interval_seconds
                .unwrap_or(DEFAULT_SNMP_POLL_INTERVAL_SECONDS),
            1,
        )),
        access_mode: Set(access_mode_declarado(input.access_mode.as_deref(), None)?),
        operating_system: Set(sistema_declarado(input.operating_system.as_deref(), None)?),
        status: Set(input.status.unwrap_or_else(|| "unknown".into())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    sync_device_monitor(&ctx.db, &row).await?;
    if let Some(ip) = &row.ip_address {
        discovery_results::Entity::delete_many()
            .filter(discovery_results::Column::IpAddress.eq(ip))
            .exec(&ctx.db)
            .await?;
    }

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Create,
                resource_type: ResourceType::Device,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!(
                    "Dispositivo '{}' ({}) criado",
                    row.name, row.r#type
                )),
                changes: None,
            },
        )
        .await;

    Ok((StatusCode::CREATED, Json(present(&ctx.db, row).await?)).into_response())
}

async fn show(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let row = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    Ok(format::json(present(&ctx.db, row).await?)?)
}

async fn update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<DeviceInput>,
) -> AppResult<Response> {
    let current = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let old_presented = present(&ctx.db, current.clone()).await?;
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&current.name)
        .to_string();
    if name.contains('\n') || name.contains('\r') {
        return Err(AppError::validation(
            "Nome do dispositivo não pode conter quebra de linha",
        ));
    }
    let kind = input
        .device_type
        .as_deref()
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .unwrap_or(&current.r#type)
        .to_string();
    if kind.contains('\n') || kind.contains('\r') {
        return Err(AppError::validation(
            "Tipo do dispositivo não pode conter quebra de linha",
        ));
    }
    if let Some(ip) = input
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|ip| !ip.is_empty())
    {
        if ip.parse::<std::net::IpAddr>().is_err() {
            return Err(AppError::validation(format!(
                "Endereço IP inválido: '{ip}'"
            )));
        }
    }
    let snmp_poll_interval_seconds = std::cmp::max(
        input
            .snmp_poll_interval_seconds
            .unwrap_or(current.snmp_poll_interval_seconds),
        1,
    );
    // Regra de negócio, não perfil de acesso: o dispositivo que representa
    // esta instalação não aceita mudança do que sustenta sua identidade.
    system_device::ensure_identity_preserved(
        &current,
        &ProposedIdentity {
            device_type: input
                .device_type
                .as_deref()
                .map(str::trim)
                .filter(|k| !k.is_empty()),
            ip_address: input
                .ip_address
                .as_deref()
                .map(str::trim)
                .filter(|ip| !ip.is_empty()),
            snmp_enabled: input.snmp_enabled,
            network_id: input.network_id,
        },
    )?;
    let access_mode = access_mode_declarado(input.access_mode.as_deref(), current.access_mode)?;
    let operating_system =
        sistema_declarado(input.operating_system.as_deref(), current.operating_system)?;
    let row = devices::ActiveModel {
        id: Set(current.id),
        site_id: Set(input.site_id.or(current.site_id)),
        network_id: Set(input.network_id.or(current.network_id)),
        parent_id: Set(input.parent_id.or(current.parent_id)),
        ip_address: Set(input.ip_address.or(current.ip_address)),
        name: Set(name),
        r#type: Set(kind),
        vendor: Set(input.vendor.or(current.vendor)),
        model: Set(input.model.or(current.model)),
        serial_number: Set(input.serial_number.or(current.serial_number)),
        description: Set(input.description.or(current.description)),
        is_monitored: Set(input.is_monitored.unwrap_or(current.is_monitored)),
        snmp_enabled: Set(input.snmp_enabled.unwrap_or(current.snmp_enabled)),
        snmp_community: Set(input.snmp_community.or(current.snmp_community)),
        snmp_version: Set(input.snmp_version.or(current.snmp_version)),
        snmp_poll_interval_seconds: Set(snmp_poll_interval_seconds),
        access_mode: Set(access_mode),
        operating_system: Set(operating_system),
        status: Set(input.status.unwrap_or(current.status)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    sync_device_monitor(&ctx.db, &row).await?;
    if input.clear_history == Some(true) {
        let logs_db = crate::services::syslog::LogsDb::from_context(&ctx).ok();
        ResourceCleanupService::clear_device_history(
            &ctx.db,
            logs_db.as_ref().map(|l| l.connection()),
            row.id,
        )
        .await?;
    }
    if row.snmp_poll_interval_seconds != current.snmp_poll_interval_seconds {
        sync_monitor_intervals(&ctx.db, row.id, row.snmp_poll_interval_seconds).await?;
    }

    let new_presented = present(&ctx.db, row.clone()).await?;
    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::Device,
                resource_id: Some(row.id),
                resource_label: Some(row.name.clone()),
                description: Some(format!(
                    "Dispositivo '{}' ({}) atualizado",
                    row.name, row.r#type
                )),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(old_presented).ok(),
                    new: serde_json::to_value(new_presented.clone()).ok(),
                }),
            },
        )
        .await;

    Ok(format::json(new_presented)?)
}

async fn destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let row = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let old_presented = present(&ctx.db, row.clone()).await?;
    let old_name = row.name.clone();
    let old_type = row.r#type.clone();
    system_device::ensure_deletable(&row)?;
    ResourceCleanupService::delete_device(&ctx.db, id).await?;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Delete,
                resource_type: ResourceType::Device,
                resource_id: Some(id),
                resource_label: Some(old_name.clone()),
                description: Some(format!(
                    "Dispositivo '{}' ({}) excluído",
                    old_name, old_type
                )),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(old_presented).ok(),
                    new: None,
                }),
            },
        )
        .await;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `GET /api/devices/{id}/capabilities` — o que esta página pode mostrar.
///
/// Uma rota, e não um campo dentro de `GET /devices/{id}`: as capacidades
/// custam algumas contagens e a lista de dispositivos não precisa delas. A
/// página de detalhe pede uma vez, e com a resposta decide **abas e botões** —
/// as duas coisas, pela mesma projeção.
async fn device_capabilities(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let device = devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    Ok(format::json(
        capabilities::for_device(&ctx.db, &device).await?,
    )?)
}

async fn device_monitors(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
    let rows = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(id))
        .order_by_asc(monitors::Column::Name)
        .all(&ctx.db)
        .await?;
    Ok(format::json(
        present_monitors(&ctx.db, rows, RECENT_RESULTS_LIMIT).await?,
    )?)
}

fn metric_json(
    row: metrics_entity::Model,
    interface_names: &HashMap<i64, String>,
) -> DeviceMetricItem {
    let interface_name = row
        .interface_id
        .and_then(|interface_id| interface_names.get(&interface_id))
        .cloned();
    DeviceMetricItem {
        id: row.id,
        device_id: row.device_id,
        interface_id: row.interface_id,
        interface_name,
        metric_name: row.name,
        metric_value: row.value,
        unit: row.unit,
        created_at: row.recorded_at.format("%d/%m/%Y %H:%M:%S").to_string(),
    }
}

async fn metrics(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<PaginationQuery>,
) -> AppResult<Response> {
    let interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces::Column::DeviceId.eq(id))
        .all(&ctx.db)
        .await?;
    let interface_names: HashMap<i64, String> = interfaces
        .iter()
        .filter(|interface| interface.admin_status.as_deref() == Some("up"))
        .map(|interface| (interface.id, interface.name.clone()))
        .collect();
    let visible_interface_ids: Vec<i64> = interface_names.keys().copied().collect();
    let base = metrics_entity::Entity::find()
        .filter(metrics_entity::Column::DeviceId.eq(id))
        .filter(if visible_interface_ids.is_empty() {
            metrics_entity::Column::InterfaceId.is_null()
        } else {
            metrics_entity::Column::InterfaceId
                .is_null()
                .or(metrics_entity::Column::InterfaceId.is_in(visible_interface_ids))
        })
        .order_by_desc(metrics_entity::Column::RecordedAt);
    let body = if let Some(page) = query.page {
        MaybePaged::Page(
            paginate_compat(&ctx.db, base, page, query.limit.unwrap_or(20), |metric| {
                metric_json(metric, &interface_names)
            })
            .await?,
        )
    } else {
        MaybePaged::List(
            base.limit(1000)
                .all(&ctx.db)
                .await?
                .into_iter()
                .map(|metric| metric_json(metric, &interface_names))
                .collect(),
        )
    };
    Ok(format::json(body)?)
}

fn event_json(row: alert_events::Model) -> DeviceEventItem {
    DeviceEventItem {
        id: row.id,
        device_id: row.device_id.unwrap_or_default(),
        event_type: row.status,
        severity: row.severity,
        message: row
            .message
            .unwrap_or_else(|| "Sem mensagem de detalhes".into()),
        created_at: row.created_at.format("%d/%m/%Y %H:%M:%S").to_string(),
    }
}

async fn events(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Query(query): Query<PaginationQuery>,
) -> AppResult<Response> {
    let base = alert_events::Entity::find()
        .filter(alert_events::Column::DeviceId.eq(id))
        .order_by_desc(alert_events::Column::CreatedAt);
    let body = if let Some(page) = query.page {
        MaybePaged::Page(
            paginate_compat(&ctx.db, base, page, query.limit.unwrap_or(20), event_json).await?,
        )
    } else {
        MaybePaged::List(
            base.limit(50)
                .all(&ctx.db)
                .await?
                .into_iter()
                .map(event_json)
                .collect(),
        )
    };
    Ok(format::json(body)?)
}

/// Corpo de `POST /api/devices/identify`.
///
/// Vem do **formulário**, e não de um id: o operador precisa poder identificar
/// antes de salvar, e num cadastro novo ainda não há dispositivo para consultar.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentifyInput {
    ip_address: Option<String>,
    snmp_enabled: Option<bool>,
    snmp_version: Option<String>,
    snmp_community: Option<String>,
    vendor: Option<String>,
    model: Option<String>,
}

/// `POST /api/devices/identify` — descobre o sistema **agora**, e diz como.
///
/// Consulta o SNMP (quando há comunidade) e lê a identificação do servidor SSH,
/// as duas em paralelo. Devolve a evidência crua junto da conclusão: o campo que
/// só afirma "Linux" não tem como ser conferido, e foi exatamente assim que um
/// OpenWrt ficou identificado errado sem ninguém perceber.
///
/// **Não grava nada.** É consulta — quem decide o que fica é o formulário.
async fn identify(Json(entrada): Json<IdentifyInput>) -> AppResult<Response> {
    let host = entrada
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .and_then(|texto| texto.parse::<std::net::IpAddr>().ok());

    let comunidade = entrada
        .snmp_community
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty());

    let (snmp, ssh) = match host {
        Some(endereco) => {
            let consulta_snmp = async {
                match comunidade {
                    Some(chave) if entrada.snmp_enabled.unwrap_or(false) => {
                        hints::identidade_snmp(endereco, chave, entrada.snmp_version.as_deref())
                            .await
                    }
                    _ => None,
                }
            };
            tokio::join!(consulta_snmp, hints::sonda_ssh(endereco))
        }
        None => (None, (false, None)),
    };
    let ssh_banner = ssh.1;

    let achado = systems::detect(&systems::Evidence {
        // A declaração fica de fora: o botão existe para dizer o que o
        // **equipamento** é, e devolver de volta o que o operador acabou de
        // escolher no seletor faria a detecção concordar consigo mesma.
        declared: None,
        sys_object_id: snmp.as_ref().and_then(|info| info.sys_object_id.as_deref()),
        sys_descr: snmp.as_ref().and_then(|info| info.sys_descr.as_deref()),
        ssh_banner: ssh_banner.as_deref(),
        vendor: entrada.vendor.as_deref(),
        model: entrada.model.as_deref(),
    });

    Ok(format::json(systems::IdentifyResult {
        operating_system: achado.system.id.to_owned(),
        label: achado.system.label.to_owned(),
        source: achado.source.to_owned(),
        reason: achado.reason,
        sys_descr: snmp.as_ref().and_then(|info| info.sys_descr.clone()),
        sys_object_id: snmp.as_ref().and_then(|info| info.sys_object_id.clone()),
        probed: snmp.is_some() || ssh_banner.is_some(),
        ssh_banner,
    })?)
}

/// `GET /api/devices/systems` — o catálogo de sistemas.
///
/// Vem do servidor em vez de ser uma constante duplicada no frontend: são as
/// mesmas entradas que decidem a receita de syslog, o meio de acesso e o perfil
/// da VPN, e uma segunda cópia divergiria na primeira adição.
async fn operating_systems() -> AppResult<Response> {
    Ok(format::json(systems::options())?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/devices")
        .add("/", get(index).post(store))
        // Antes de `/{id}`: o `matchit` do axum prioriza segmento literal sobre
        // parâmetro, e um teste garante que a rota não seja engolida.
        .add("/systems", get(operating_systems))
        .add("/identify", post(identify))
        .add("/{id}", get(show).put(update).delete(destroy))
        .add("/{id}/capabilities", get(device_capabilities))
        .add("/{id}/monitors", get(device_monitors))
        .add("/{id}/metrics", get(metrics))
        .add("/{id}/events", get(events))
}
