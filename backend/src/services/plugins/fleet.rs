//! Plugins de frota: a mesma ação em vários equipamentos.
//!
//! Os membros da frota são os equipamentos onde o plugin está **instalado**
//! (`device_plugin_installs`) — "adicionar o roteador à rede Wi-Fi" é instalar
//! o plugin nele, e as credenciais continuam em cada `/devices/{id}`.
//!
//! Uma ação de frota vira um **lote** (`plugin_batches`):
//!
//! 1. a ação de dispositivo roda em cada membro, até [`CONCURRENCY`] por vez,
//!    cada uma com a sua execução auditada (`plugin_runs.batch_id`) — as
//!    portas são as mesmas da execução avulsa (efeito, aprovação, máscara);
//! 2. o andamento de cada membro sai pelo SSE global (`plugin:batch_updated`);
//! 3. no fim, a função `reduce` do script (pura, sem equipamento) consolida
//!    os resultados — o plano de canais, por exemplo — e pode propor
//!    `settings_patch`, ajustes por equipamento que o operador aceita.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
};

use futures::StreamExt;
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::{json, Map, Value};
use tokio_util::sync::CancellationToken;

use super::{
    compat::{self, DeviceFacts},
    credentials,
    effect::Effect,
    manifest::{FleetAction, PluginManifest, Surface},
    runs::{self, Origin, RunSpec},
    runtime, service,
    service::status,
    settings,
};
use crate::{
    dtos::plugins::{
        BatchDevice, FleetCandidate, FleetMember, FleetView, PluginApp, PluginBatchView,
    },
    models::{device_plugin_installs, devices, plugin_batches, plugins},
    services::{
        events::EventBus,
        shared::errors::{AppError, AppResult},
    },
};

pub const BATCH_EVENT: &str = "plugin:batch_updated";
/// Equipamentos atendidos ao mesmo tempo num lote.
pub const CONCURRENCY: usize = 4;
/// Lotes mostrados na página da frota.
const RECENT_BATCHES: u64 = 15;

fn internal(error: impl std::fmt::Display) -> AppError {
    AppError::Internal(anyhow::anyhow!("{error}"))
}

fn cancellations() -> &'static Mutex<HashMap<i64, CancellationToken>> {
    static TOKENS: OnceLock<Mutex<HashMap<i64, CancellationToken>>> = OnceLock::new();
    TOKENS.get_or_init(Mutex::default)
}

fn lock_cancellations() -> std::sync::MutexGuard<'static, HashMap<i64, CancellationToken>> {
    cancellations()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn manifest_of(plugin: &plugins::Model) -> AppResult<PluginManifest> {
    serde_json::from_value(plugin.manifest.clone()).map_err(internal)
}

/// A ação da frota pelo id; o `statusAction` vale como ação sem `reduce`.
fn resolve_action(manifest: &PluginManifest, id: &str) -> Option<FleetAction> {
    if let Some(action) = manifest.fleet_action(id) {
        return Some(action.clone());
    }
    let fleet = manifest.fleet.as_ref()?;
    (fleet.status_action.as_deref() == Some(id)).then(|| FleetAction {
        id: id.to_owned(),
        title: manifest
            .action(id)
            .map_or_else(|| id.to_owned(), |action| action.title.clone()),
        description: None,
        icon: None,
        action: id.to_owned(),
        reduce: None,
        labels: None,
    })
}

fn title_of(manifest: &PluginManifest, id: &str) -> String {
    resolve_action(manifest, id).map_or_else(|| id.to_owned(), |action| action.title)
}

/// Os membros da frota, na ordem do nome.
///
/// # Errors
///
/// Erro do banco.
pub async fn members<C: ConnectionTrait>(
    db: &C,
    plugin_id: i64,
) -> AppResult<Vec<(devices::Model, String)>> {
    let installs = device_plugin_installs::Entity::find()
        .filter(device_plugin_installs::Column::PluginId.eq(plugin_id))
        .all(db)
        .await?;
    let ids: Vec<i64> = installs.iter().map(|install| install.device_id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let since: HashMap<i64, String> = installs
        .iter()
        .map(|install| (install.device_id, install.created_at.to_rfc3339()))
        .collect();
    let mut list: Vec<(devices::Model, String)> = devices::Entity::find()
        .filter(devices::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|device| {
            let at = since.get(&device.id).cloned().unwrap_or_default();
            (device, at)
        })
        .collect();
    list.sort_by_key(|a| a.0.name.to_lowercase());
    Ok(list)
}

fn ensure_fleet(plugin: &plugins::Model, manifest: &PluginManifest) -> AppResult<()> {
    if !manifest.shows_on(Surface::Fleet) {
        return Err(AppError::business_rule(
            "Este plugin não tem página de frota.",
        ));
    }
    if plugin.status == status::QUARANTINE || plugin.status == status::DISABLED {
        return Err(AppError::business_rule(
            "O plugin está em quarentena ou desativado.",
        ));
    }
    Ok(())
}

/// Os aplicativos (plugins de frota) para o menu.
///
/// # Errors
///
/// Erro do banco.
pub async fn apps<C: ConnectionTrait>(db: &C) -> AppResult<Vec<PluginApp>> {
    service::ensure_builtins(db).await?;
    let rows = plugins::Entity::find()
        .filter(plugins::Column::Status.is_not_in([status::QUARANTINE, status::DISABLED]))
        .order_by_asc(plugins::Column::Name)
        .all(db)
        .await?;
    let mut counts: HashMap<i64, u32> = HashMap::new();
    for install in device_plugin_installs::Entity::find().all(db).await? {
        *counts.entry(install.plugin_id).or_default() += 1;
    }
    let mut apps = Vec::new();
    for model in rows {
        let manifest = manifest_of(&model)?;
        let Some(fleet) = manifest
            .fleet
            .filter(|_| manifest.surfaces.contains(&Surface::Fleet))
        else {
            continue;
        };
        apps.push(PluginApp {
            id: model.id,
            slug: model.slug.clone(),
            title: fleet.title,
            icon: fleet.icon.unwrap_or_else(|| "mdi-apps".into()),
            description: fleet.description.or(model.description),
            members: counts.get(&model.id).copied().unwrap_or(0),
        });
    }
    Ok(apps)
}

/// Tudo que a página do aplicativo precisa.
///
/// # Errors
///
/// Plugin inexistente, sem página de frota, ou erro do banco.
pub async fn view<C: ConnectionTrait>(db: &C, plugin_id: i64) -> AppResult<FleetView> {
    let plugin = service::find(db, plugin_id).await?;
    let package = service::package_of(&plugin)?;
    if !package.manifest.shows_on(Surface::Fleet) {
        return Err(AppError::not_found("Este plugin não tem página de frota."));
    }
    let membership = members(db, plugin.id).await?;
    let member_ids: HashSet<i64> = membership.iter().map(|(device, _)| device.id).collect();
    let mut list = Vec::with_capacity(membership.len());
    for (device, installed_at) in &membership {
        let facts = DeviceFacts::from_device(device);
        let verdict = compat::evaluate(&package.manifest, &package.compatibility, &facts);
        list.push(FleetMember {
            device_id: device.id,
            name: device.name.clone(),
            ip: device.ip_address.clone(),
            platform: facts.platform,
            firmware: facts.firmware,
            compat: verdict.level,
            installed_at: installed_at.clone(),
            credentials_ready: credentials_ready(db, device.id, &package.manifest).await?,
            settings: settings::view(db, &plugin, settings::Scope::Device(device.id)).await?,
        });
    }
    let mut candidates = Vec::new();
    if plugin.device_id.is_none() {
        for device in devices::Entity::find()
            .filter(devices::Column::IpAddress.is_not_null())
            .order_by_asc(devices::Column::Name)
            .all(db)
            .await?
        {
            if member_ids.contains(&device.id)
                || crate::services::devices::system_device::is_protected(&device)
            {
                continue;
            }
            let verdict = compat::evaluate(
                &package.manifest,
                &package.compatibility,
                &DeviceFacts::from_device(&device),
            );
            if verdict.level != compat::Compat::Incompatible {
                candidates.push(FleetCandidate {
                    device_id: device.id,
                    name: device.name,
                    ip: device.ip_address,
                    compat: verdict.level,
                    reasons: verdict.reasons,
                });
            }
        }
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.compat));
    }
    Ok(FleetView {
        plugin: service::summary(&plugin)?,
        settings: settings::view(db, &plugin, settings::Scope::Fleet).await?,
        members: list,
        candidates,
        batches: recent(db, &plugin, RECENT_BATCHES).await?,
    })
}

async fn credentials_ready<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    manifest: &PluginManifest,
) -> AppResult<bool> {
    let list = credentials::list(db, device_id).await?;
    Ok(manifest.transports.iter().all(|transport| {
        // HTTP sem login funciona sem credencial; SSH/Telnet não.
        *transport == super::manifest::TransportKind::Http
            || list.iter().any(|credential| {
                credential.kind == *transport
                    && (credential.has_stored_secret || credential.session_active)
            })
    }))
}

fn has_patch(result: Option<&Value>) -> bool {
    result.is_some_and(|result| {
        result
            .get("settings_patch")
            .and_then(Value::as_object)
            .is_some_and(|patch| !patch.is_empty())
            && result.get("patch_applied").and_then(Value::as_bool) != Some(true)
    })
}

fn batch_view(batch: &plugin_batches::Model, manifest: &PluginManifest) -> PluginBatchView {
    PluginBatchView {
        id: batch.id,
        plugin_id: batch.plugin_id,
        action: batch.action.clone(),
        title: title_of(manifest, &batch.action),
        status: batch.status.clone(),
        devices: serde_json::from_value(batch.devices.clone()).unwrap_or_default(),
        result: batch.result.clone(),
        has_patch: has_patch(batch.result.as_ref()),
        error: batch.error.clone(),
        created_at: batch.created_at.to_rfc3339(),
        finished_at: batch.finished_at.map(|at| at.to_rfc3339()),
    }
}

async fn recent<C: ConnectionTrait>(
    db: &C,
    plugin: &plugins::Model,
    limit: u64,
) -> AppResult<Vec<PluginBatchView>> {
    let manifest = manifest_of(plugin)?;
    Ok(plugin_batches::Entity::find()
        .filter(plugin_batches::Column::PluginId.eq(plugin.id))
        .order_by_desc(plugin_batches::Column::Id)
        .limit(limit)
        .all(db)
        .await?
        .iter()
        .map(|batch| batch_view(batch, &manifest))
        .collect())
}

/// # Errors
///
/// Lote inexistente.
pub async fn find_view<C: ConnectionTrait>(db: &C, batch_id: i64) -> AppResult<PluginBatchView> {
    let batch = find_batch(db, batch_id).await?;
    let plugin = service::find(db, batch.plugin_id).await?;
    Ok(batch_view(&batch, &manifest_of(&plugin)?))
}

async fn find_batch<C: ConnectionTrait>(db: &C, batch_id: i64) -> AppResult<plugin_batches::Model> {
    plugin_batches::Entity::find_by_id(batch_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Lote não encontrado."))
}

/// Dispara uma ação de frota. Roda em segundo plano; o andamento chega pelo
/// SSE.
///
/// # Errors
///
/// Plugin sem frota ou bloqueado, ação inexistente, escrita sem
/// confirmação, ou nenhum membro para atender.
pub async fn start(
    ctx: &AppContext,
    plugin_id: i64,
    action_id: &str,
    device_ids: &[i64],
    params: Value,
    confirm_write: bool,
    user_id: Option<i64>,
) -> AppResult<i64> {
    let plugin = service::find(&ctx.db, plugin_id).await?;
    let manifest = manifest_of(&plugin)?;
    ensure_fleet(&plugin, &manifest)?;
    let action = resolve_action(&manifest, action_id).ok_or_else(|| {
        AppError::not_found(format!("O plugin não tem a ação de frota `{action_id}`."))
    })?;
    let declared = manifest
        .action(&action.action)
        .ok_or_else(|| AppError::not_found("Ação de dispositivo inexistente."))?;
    if declared.effect == Effect::Write && !confirm_write {
        return Err(AppError::business_rule(
            "Esta ação altera os equipamentos e precisa de confirmação explícita.",
        ));
    }
    let mut targets = members(&ctx.db, plugin.id).await?;
    if !device_ids.is_empty() {
        targets.retain(|(device, _)| device_ids.contains(&device.id));
    }
    if targets.is_empty() {
        return Err(AppError::business_rule(
            "Nenhum equipamento na frota para esta ação. Adicione dispositivos antes.",
        ));
    }
    let devices: Vec<BatchDevice> = targets
        .iter()
        .map(|(device, _)| BatchDevice {
            device_id: device.id,
            device_name: device.name.clone(),
            run_id: None,
            status: "pending".into(),
            error: None,
            output: None,
        })
        .collect();
    let batch = plugin_batches::ActiveModel {
        plugin_id: Set(plugin.id),
        action: Set(action.id.chars().take(64).collect()),
        status: Set("running".into()),
        params: Set(Some(params.clone())),
        devices: Set(serde_json::to_value(&devices).map_err(internal)?),
        result: Set(None),
        error: Set(None),
        user_id: Set(user_id),
        finished_at: Set(None),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;
    let cancel = CancellationToken::new();
    lock_cancellations().insert(batch.id, cancel.clone());
    let batch_id = batch.id;
    publish(ctx, &batch, &manifest);
    let job = Job {
        ctx: ctx.clone(),
        batch,
        plugin,
        manifest,
        action,
        targets: targets.into_iter().map(|(device, _)| device).collect(),
        params,
        user_id,
        cancel,
        writer: tokio::sync::Mutex::new(()),
    };
    tokio::spawn(async move {
        if let Err(error) = job.run().await {
            tracing::warn!(%error, batch_id, "falha ao gravar o lote de frota");
        }
    });
    Ok(batch_id)
}

/// Pede o cancelamento do lote: quem não começou é pulado e as execuções em
/// andamento são canceladas. `false` quando o lote já acabou.
#[must_use]
pub fn cancel(batch_id: i64) -> bool {
    lock_cancellations()
        .get(&batch_id)
        .map(CancellationToken::cancel)
        .is_some()
}

fn publish(ctx: &AppContext, batch: &plugin_batches::Model, manifest: &PluginManifest) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        bus.publish_ephemeral(BATCH_EVENT, json!({ "batch": batch_view(batch, manifest) }));
    }
}

struct Job {
    ctx: AppContext,
    batch: plugin_batches::Model,
    plugin: plugins::Model,
    manifest: PluginManifest,
    action: FleetAction,
    targets: Vec<devices::Model>,
    params: Value,
    user_id: Option<i64>,
    cancel: CancellationToken,
    /// Uma gravação de andamento por vez, sempre do estado mais recente —
    /// membros terminando juntos não podem gravar fora de ordem.
    writer: tokio::sync::Mutex<()>,
}

impl Job {
    async fn run(self) -> AppResult<()> {
        let state = Arc::new(Mutex::new(self.batch.clone()));
        let job = Arc::new(self);
        let tasks = job.targets.clone().into_iter().map(|device| {
            let job = job.clone();
            let state = state.clone();
            async move { job.member(device, &state).await }
        });
        futures::stream::iter(tasks)
            .buffer_unordered(CONCURRENCY)
            .collect::<Vec<()>>()
            .await;
        job.finish(&state).await
    }

    fn update(&self, state: &Mutex<plugin_batches::Model>, entry: BatchDevice) {
        let mut batch = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut list: Vec<BatchDevice> =
            serde_json::from_value(batch.devices.clone()).unwrap_or_default();
        if let Some(slot) = list
            .iter_mut()
            .find(|item| item.device_id == entry.device_id)
        {
            *slot = entry;
        }
        batch.devices = serde_json::to_value(&list).unwrap_or_else(|_| json!([]));
    }

    async fn save(&self, state: &Mutex<plugin_batches::Model>) {
        let _turn = self.writer.lock().await;
        let batch = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut active: plugin_batches::ActiveModel = batch.clone().into();
        active.devices = Set(batch.devices.clone());
        active.status = Set(batch.status.clone());
        active.result = Set(batch.result.clone());
        active.error = Set(batch.error.clone());
        active.finished_at = Set(batch.finished_at);
        if let Err(error) = active.update(&self.ctx.db).await {
            tracing::warn!(%error, batch_id = batch.id, "falha ao gravar o andamento do lote");
        }
        publish(&self.ctx, &batch, &self.manifest);
    }

    async fn member(&self, device: devices::Model, state: &Mutex<plugin_batches::Model>) {
        let mut entry = BatchDevice {
            device_id: device.id,
            device_name: device.name.clone(),
            run_id: None,
            status: "running".into(),
            error: None,
            output: None,
        };
        if self.cancel.is_cancelled() {
            entry.status = "skipped".into();
            entry.error = Some("lote cancelado".into());
            self.update(state, entry);
            self.save(state).await;
            return;
        }
        self.update(state, entry.clone());
        self.save(state).await;

        let approval = runs::approval_for(&self.plugin, Origin::Fleet, false);
        let spec = RunSpec {
            plugin: self.plugin.clone(),
            device,
            action: self.action.action.clone(),
            params: self.params.clone(),
            origin: Origin::Fleet,
            user_id: self.user_id,
            reason: None,
            approval,
            batch_id: Some(self.batch.id),
        };
        match runs::prepare(&self.ctx, spec).await {
            Err(error) => {
                entry.status = "failed".into();
                entry.error = Some(error.to_string());
            }
            Ok(prepared) => {
                entry.run_id = Some(prepared.run_id());
                self.update(state, entry.clone());
                self.save(state).await;
                let cancel_watch = {
                    let batch_cancel = self.cancel.clone();
                    let run_id = prepared.run_id();
                    tokio::spawn(async move {
                        batch_cancel.cancelled().await;
                        let _ = runs::cancel(run_id);
                    })
                };
                match runs::run(&self.ctx, prepared).await {
                    Ok(run) => {
                        entry.status = if run.status == "succeeded" {
                            "succeeded".into()
                        } else {
                            "failed".into()
                        };
                        entry.error = run.error;
                        entry.output = run.output;
                    }
                    Err(error) => {
                        entry.status = "failed".into();
                        entry.error = Some(error.to_string());
                    }
                }
                cancel_watch.abort();
            }
        }
        self.update(state, entry);
        self.save(state).await;
    }

    async fn finish(&self, state: &Mutex<plugin_batches::Model>) -> AppResult<()> {
        lock_cancellations().remove(&self.batch.id);
        let mut batch = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let list: Vec<BatchDevice> =
            serde_json::from_value(batch.devices.clone()).unwrap_or_default();
        let succeeded = list
            .iter()
            .filter(|item| item.status == "succeeded")
            .count();
        batch.status = if self.cancel.is_cancelled() {
            "cancelled".into()
        } else if succeeded == list.len() {
            "succeeded".into()
        } else if succeeded == 0 {
            "failed".into()
        } else {
            "partial".into()
        };
        if let Some(reduce) = self.action.reduce.as_deref().filter(|_| succeeded > 0) {
            let package = service::package_of(&self.plugin)?;
            let effective = settings::effective(&self.ctx.db, &self.plugin, None).await?;
            let results = serde_json::to_value(&list).map_err(internal)?;
            match runtime::reduce(
                &package.script,
                reduce,
                results,
                effective.value,
                effective.secrets,
            )
            .await
            {
                Ok(result) => batch.result = Some(result),
                Err(error) => {
                    batch.error = Some(format!("o consolidado falhou: {error}"));
                }
            }
        }
        batch.finished_at = Some(chrono::Utc::now().into());
        *state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = batch;
        self.save(state).await;
        Ok(())
    }
}

/// Aplica o `settings_patch` do consolidado: o ajuste sugerido de cada
/// equipamento (ex.: o canal do plano) vai para a configuração dele.
///
/// # Errors
///
/// Lote sem ajuste pendente, ou ajuste fora do esquema do dispositivo.
pub async fn apply_patch<C: ConnectionTrait>(
    db: &C,
    batch_id: i64,
    user_id: Option<i64>,
) -> AppResult<PluginBatchView> {
    let batch = find_batch(db, batch_id).await?;
    if !has_patch(batch.result.as_ref()) {
        return Err(AppError::business_rule(
            "Este lote não tem ajuste pendente para aplicar.",
        ));
    }
    let plugin = service::find(db, batch.plugin_id).await?;
    let mut result = batch.result.clone().unwrap_or_default();
    let patch: Map<String, Value> = result
        .get("settings_patch")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let member_ids: HashSet<i64> = members(db, plugin.id)
        .await?
        .into_iter()
        .map(|(device, _)| device.id)
        .collect();
    for (device, values) in &patch {
        let device_id: i64 = device.parse().map_err(|_| {
            AppError::validation(format!("ajuste para dispositivo inválido: {device}"))
        })?;
        if !member_ids.contains(&device_id) {
            continue;
        }
        let Some(values) = values.as_object() else {
            continue;
        };
        settings::patch_device(db, &plugin, device_id, values, user_id).await?;
    }
    result["patch_applied"] = Value::Bool(true);
    let mut active: plugin_batches::ActiveModel = batch.into();
    active.result = Set(Some(result));
    let saved = active.update(db).await?;
    Ok(batch_view(&saved, &manifest_of(&plugin)?))
}
