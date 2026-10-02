//! Qual sistema o equipamento roda — para decidir que plugins servem a ele.
//!
//! O cadastro costuma trazer fabricante e modelo, que descrevem o **hardware**
//! (uma RouterBOARD da MikroTik pode rodar OpenWrt). Aqui se vai ao
//! equipamento ([`identify::observe`]: SSH e SNMP; o Laya quando eles não
//! decidem), grava-se o que ele mostrou e liga-se o que passou a servir.

use futures::StreamExt;
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

use super::{
    compat::{self, Certainty, DeviceFacts},
    credentials, fleet,
    manifest::{TransportKind, DETECT_ACTION},
    runs::{self, Origin, RunSpec},
    service::{self, status},
};
use crate::{
    dtos::plugins::FleetView,
    models::{devices, plugins},
    services::{
        devices::identify,
        events::EventBus,
        shared::errors::{AppError, AppResult},
    },
};

/// Um plugin ligou sozinho: o menu Aplicativos de quem estiver aberto relê.
pub const APPS_EVENT: &str = "plugin:apps_changed";

/// Equipamentos verificados ao mesmo tempo (cada um espera SSH/SNMP).
const CONCURRENCY: usize = 4;

/// Observa o equipamento e liga os plugins que passaram a servir a ele.
///
/// # Errors
///
/// Erro do banco.
pub async fn refresh(ctx: &AppContext, device: &devices::Model) -> AppResult<devices::Model> {
    // O SSH pode não estar na 22: a credencial cadastrada diz onde está.
    let ssh_port = credentials::list(&ctx.db, device.id)
        .await?
        .into_iter()
        .find(|credential| credential.kind == TransportKind::Ssh)
        .map(|credential| credential.port);
    let observed = identify::observe(ctx, device, ssh_port).await?;
    let activated = service::activate_for_device(&ctx.db, &observed).await?;
    if !activated.is_empty() {
        if let Ok(bus) = EventBus::from_context(ctx) {
            bus.publish_ephemeral(APPS_EVENT, json!({ "activated": activated }));
        }
    }
    Ok(observed)
}

/// [`refresh`] sem segurar quem cadastrou: a sonda pode levar segundos.
pub fn refresh_in_background(ctx: &AppContext, device: devices::Model) {
    // Como o agendador e o syslog: nos testes, nada de sonda solta disputando
    // o banco com a limpeza entre um teste e outro (o "Verificar sistema",
    // síncrono, continua coberto).
    if device.ip_address.is_none() || ctx.environment == loco_rs::environment::Environment::Test {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(error) = refresh(&ctx, &device).await {
            tracing::warn!(%error, device_id = device.id, "não foi possível identificar o sistema");
        }
    });
}

/// "Verificar sistema" na página do aplicativo: os equipamentos pedidos ou, sem
/// lista, os da frota e os candidatos cuja compatibilidade ainda é dúvida.
/// Devolve a página atualizada.
///
/// # Errors
///
/// Plugin inexistente ou sem frota, ou erro do banco.
pub async fn refresh_fleet(
    ctx: &AppContext,
    plugin_id: i64,
    device_ids: &[i64],
    user_id: Option<i64>,
) -> AppResult<FleetView> {
    let plugin = service::find(&ctx.db, plugin_id).await?;
    let before = fleet::view(&ctx.db, plugin_id).await?;
    let ids: Vec<i64> = if device_ids.is_empty() {
        let doubtful = |level: compat::Compat| {
            level != compat::Compat::Likely && level != compat::Compat::Validated
        };
        before
            .members
            .iter()
            .filter(|member| doubtful(member.compat))
            .map(|member| member.device_id)
            .chain(
                before
                    .candidates
                    .iter()
                    .filter(|candidate| doubtful(candidate.compat))
                    .map(|candidate| candidate.device_id),
            )
            .collect()
    } else {
        device_ids.to_vec()
    };
    if ids.is_empty() {
        return Ok(before);
    }
    let targets = devices::Entity::find()
        .filter(devices::Column::Id.is_in(ids))
        .filter(devices::Column::IpAddress.is_not_null())
        .all(&ctx.db)
        .await?;
    if targets.is_empty() {
        return Err(AppError::business_rule(
            "Nenhum desses equipamentos tem endereço IP para verificar.",
        ));
    }
    let plugin = &plugin;
    futures::stream::iter(targets.into_iter().map(|device| async move {
        match refresh(ctx, &device).await {
            Ok(observed) => read_firmware(ctx, plugin, observed, user_id).await,
            Err(error) => {
                tracing::warn!(%error, device_id = device.id, "não foi possível identificar o sistema");
            }
        }
    }))
    .buffer_unordered(CONCURRENCY)
    .collect::<Vec<()>>()
    .await;
    fleet::view(&ctx.db, plugin_id).await
}

/// Sistema confirmado e firmware desconhecido: o "Detectar" do plugin (só
/// leitura, auditado) lê a versão — o que falta para a compatibilidade sair
/// da dúvida. Só com o plugin ativo (sem aprovação a cada acesso) e só no
/// sistema que ele atende; sem credencial, fica como está.
async fn read_firmware(
    ctx: &AppContext,
    plugin: &plugins::Model,
    device: devices::Model,
    user_id: Option<i64>,
) {
    if plugin.status != status::ACTIVE || device.firmware_version.is_some() {
        return;
    }
    let Ok(package) = service::package_of(plugin) else {
        return;
    };
    let facts = DeviceFacts::from_device(&device);
    if facts.certainty != Certainty::Certain || !compat::names_platform(&package.manifest, &facts) {
        return;
    }
    let spec = RunSpec {
        plugin: plugin.clone(),
        approval: runs::approval_for(plugin, Origin::User, false),
        device,
        action: DETECT_ACTION.to_owned(),
        params: json!({}),
        origin: Origin::User,
        user_id,
        reason: Some("Verificar sistema: ler a versão do firmware".to_owned()),
        batch_id: None,
    };
    if let Ok(prepared) = runs::prepare(ctx, spec).await {
        if let Err(error) = runs::run(ctx, prepared).await {
            tracing::warn!(%error, "o Detectar da verificação de sistema falhou");
        }
    }
}
