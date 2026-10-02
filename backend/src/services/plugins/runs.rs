//! Execução de uma ação de plugin num equipamento real.
//!
//! Cada execução vira uma linha em `plugin_runs` — com parâmetros, resultado e
//! transcript já sem segredos — e é acompanhada pelo SSE global:
//!
//! * `plugin:run_started` / `plugin:run_finished` — o estado da execução;
//! * `plugin:run_output` — cada acesso ao equipamento, à medida que acontece;
//! * `plugin:approval_required` — quando o gate pede aprovação (ver `gate`).
//!
//! O mesmo caminho serve à tela (em segundo plano, [`spawn`]), à IA e à
//! validação funcional (esperando o fim, [`run`]).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::{
    compat::DeviceFacts,
    credentials,
    effect::{self, Effect},
    gate::{AccessGate, AllowAll, ApprovalContext, InteractiveGate},
    manifest::{PluginAction, PluginManifest, TransportKind, DETECT_ACTION},
    package::{CompatEntry, PluginPackage},
    params,
    runtime::{
        self, DeviceInfo, ExecutionContext, Extras, LibraryPlugin, Limits, Observer,
        TranscriptEntry,
    },
    service::{self, status},
    settings, testing,
    transport::{agent::AgentTransport, local::LocalTransport, DeviceTransport},
};
use crate::{
    dtos::plugins::PluginRunView,
    models::{devices, plugin_runs, plugins},
    services::{
        agents::hub::AgentHub,
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        devices::systems,
        events::EventBus,
        shared::errors::{AppError, AppResult},
    },
};

pub const RUN_STARTED_EVENT: &str = "plugin:run_started";
pub const RUN_OUTPUT_EVENT: &str = "plugin:run_output";
/// Um acesso começou (o que está rodando agora; ainda sem resposta).
pub const RUN_STEP_EVENT: &str = "plugin:run_step";
pub const RUN_FINISHED_EVENT: &str = "plugin:run_finished";

/// Ação sintética que registra a suíte funcional.
pub const VALIDATION_ACTION: &str = "validate";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    User,
    Ai,
    Validation,
    /// Parte de uma ação de frota (vários equipamentos).
    Fleet,
}

impl Origin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Ai => "ai",
            Self::Validation => "validation",
            Self::Fleet => "fleet",
        }
    }
}

/// Como os acessos desta execução são aprovados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    /// Tudo liberado (plugin ativo pela tela, ou IA com modo automático).
    Auto,
    /// Cada acesso pausa e espera o operador.
    PerCall,
}

/// A política da nota do módulo `gate`, num lugar só.
#[must_use]
pub fn approval_for(plugin: &plugins::Model, origin: Origin, ai_auto_accept: bool) -> Approval {
    let active = plugin.status == status::ACTIVE;
    match origin {
        Origin::Ai if ai_auto_accept => Approval::Auto,
        Origin::User | Origin::Validation | Origin::Fleet | Origin::Ai if active => Approval::Auto,
        _ => Approval::PerCall,
    }
}

pub struct RunSpec {
    pub plugin: plugins::Model,
    pub device: devices::Model,
    pub action: String,
    pub params: Value,
    pub origin: Origin,
    pub user_id: Option<i64>,
    /// Motivo declarado (IA), repassado a cada pedido de aprovação.
    pub reason: Option<String>,
    pub approval: Approval,
    /// Lote de frota a que a execução pertence.
    pub batch_id: Option<i64>,
}

/// Uma execução pronta para rodar: linha criada, credenciais resolvidas.
pub struct Prepared {
    run: plugin_runs::Model,
    spec: RunSpec,
    package: PluginPackage,
    transport: Arc<dyn DeviceTransport>,
    credentials: runtime::Credentials,
    extras: Extras,
    cancel: CancellationToken,
}

impl Prepared {
    #[must_use]
    pub const fn run_id(&self) -> i64 {
        self.run.id
    }
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

/// Pede o cancelamento. `false` quando a execução já acabou.
#[must_use]
pub fn cancel(run_id: i64) -> bool {
    lock_cancellations()
        .get(&run_id)
        .map(CancellationToken::cancel)
        .is_some()
}

async fn transport_for(
    ctx: &AppContext,
    via_probe_id: Option<i64>,
) -> AppResult<Arc<dyn DeviceTransport>> {
    match via_probe_id {
        None => Ok(Arc::new(LocalTransport::new("central")?)),
        Some(probe_id) => {
            let session = AgentHub::from_context(ctx)
                .ok()
                .and_then(|hub| hub.get(probe_id))
                .ok_or_else(|| {
                    AppError::business_rule(
                        "O agente remoto escolhido para este equipamento não está conectado.",
                    )
                })?;
            Ok(Arc::new(AgentTransport::new(session)?))
        }
    }
}

fn action_of<'a>(package: &'a PluginPackage, action: &str) -> AppResult<&'a PluginAction> {
    package
        .manifest
        .action(action)
        .ok_or_else(|| AppError::not_found(format!("O plugin não tem a ação `{action}`.")))
}

/// Valida, resolve credenciais e transporte e cria a linha da execução.
///
/// # Errors
///
/// Plugin desativado/em quarentena, exclusivo de outro equipamento, ação ou
/// parâmetros inválidos, equipamento sem IP, credencial ausente, agente fora.
pub async fn prepare(ctx: &AppContext, spec: RunSpec) -> AppResult<Prepared> {
    match spec.plugin.status.as_str() {
        status::DISABLED => return Err(AppError::business_rule("O plugin está desativado.")),
        status::QUARANTINE => {
            return Err(AppError::business_rule(
                "O plugin está em quarentena: aceite a revisão de segurança antes de executá-lo.",
            ))
        }
        _ => {}
    }
    if spec
        .plugin
        .device_id
        .is_some_and(|device_id| device_id != spec.device.id)
    {
        return Err(AppError::business_rule(
            "Este plugin é exclusivo de outro equipamento.",
        ));
    }
    let ip = device_ip(&spec.device)?;

    let package = service::package_of(&spec.plugin)?;
    let mut spec = spec;
    // Parâmetro `secret` (a senha de uma rede Wi-Fi) chega em claro ao
    // script, mas o banco guarda a máscara e o runtime a esconde da saída.
    let mut secret_params = Vec::new();
    let mut stored_params = spec.params.clone();
    if spec.action != VALIDATION_ACTION {
        let action = action_of(&package, &spec.action)?;
        spec.params = params::validate(action.params.as_ref(), &spec.params)
            .map_err(|errors| AppError::validation(errors.join("; ")))?;
        secret_params = params::secret_values(action.params.as_ref(), &spec.params);
        stored_params = params::mask_secrets(action.params.as_ref(), &spec.params);
    }
    let resolved =
        credentials::resolve(&ctx.db, spec.device.id, &package.manifest.transports).await?;
    let transport = transport_for(ctx, resolved.via_probe_id).await?;
    let mut extras = load_extras(&ctx.db, &spec.plugin, &package, spec.device.id).await?;
    extras.secrets.extend(secret_params);

    let run = insert_run(
        &ctx.db,
        NewRun {
            plugin_id: Some(spec.plugin.id),
            device: &spec.device,
            user_id: spec.user_id,
            action: &spec.action,
            origin: spec.origin,
            params: stored_params,
            batch_id: spec.batch_id,
        },
    )
    .await?;
    audit_execution(
        &ctx.db,
        spec.user_id,
        (ResourceType::Plugin, spec.plugin.id, &spec.plugin.name),
        spec.origin,
        &format!("`{}`", spec.action),
        &spec.device,
        &ip,
        spec.reason.as_deref(),
    )
    .await;

    let cancel = CancellationToken::new();
    lock_cancellations().insert(run.id, cancel.clone());
    Ok(Prepared {
        run,
        spec,
        package,
        transport,
        credentials: resolved.credentials,
        extras,
        cancel,
    })
}

/// Configuração guardada e plugins reaproveitados (`uses`) da execução.
///
/// # Errors
///
/// Erro do banco ou de decifra da configuração.
pub async fn load_extras<C: ConnectionTrait>(
    db: &C,
    plugin: &plugins::Model,
    package: &PluginPackage,
    device_id: i64,
) -> AppResult<Extras> {
    let mut extras = Extras::default();
    if package.manifest.settings.is_some() {
        let effective = settings::effective(db, plugin, Some(device_id)).await?;
        extras.settings = effective.value;
        extras.secrets = effective.secrets;
    }
    extras.library = load_library(db, &package.manifest.uses).await?;
    Ok(extras)
}

/// Os plugins ativos que `uses` cita, para `device.use_plugin`.
///
/// # Errors
///
/// Erro do banco.
pub async fn load_library<C: ConnectionTrait>(
    db: &C,
    uses: &[String],
) -> AppResult<Vec<LibraryPlugin>> {
    let mut library = Vec::new();
    for slug in uses {
        let found = plugins::Entity::find()
            .filter(plugins::Column::Slug.eq(slug.clone()))
            .filter(plugins::Column::Status.eq(status::ACTIVE))
            .filter(plugins::Column::DeviceId.is_null())
            .order_by_desc(plugins::Column::Id)
            .one(db)
            .await?;
        if let Some(model) = found {
            let package = service::package_of(&model)?;
            library.push(LibraryPlugin {
                slug: slug.clone(),
                manifest: package.manifest,
                script: package.script,
            });
        }
    }
    Ok(library)
}

fn device_ip(device: &devices::Model) -> AppResult<String> {
    let ip = device
        .ip_address
        .clone()
        .filter(|ip| !ip.trim().is_empty())
        .ok_or_else(|| AppError::business_rule("O equipamento não tem endereço IP cadastrado."))?;
    ip.parse::<std::net::IpAddr>()
        .map_err(|_| AppError::business_rule(format!("Endereço inválido: {ip}")))?;
    Ok(ip)
}

/// Uma execução prestes a começar (a linha de auditoria em `plugin_runs`).
struct NewRun<'a> {
    plugin_id: Option<i64>,
    device: &'a devices::Model,
    user_id: Option<i64>,
    action: &'a str,
    origin: Origin,
    params: Value,
    batch_id: Option<i64>,
}

async fn insert_run<C: ConnectionTrait>(db: &C, run: NewRun<'_>) -> AppResult<plugin_runs::Model> {
    Ok(plugin_runs::ActiveModel {
        plugin_id: Set(run.plugin_id),
        device_id: Set(run.device.id),
        user_id: Set(run.user_id),
        action: Set(run.action.chars().take(64).collect()),
        origin: Set(run.origin.as_str().to_owned()),
        status: Set("running".to_owned()),
        params: Set(Some(run.params)),
        output: Set(None),
        transcript: Set(json!([])),
        error: Set(None),
        finished_at: Set(None),
        batch_id: Set(run.batch_id),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

#[allow(clippy::too_many_arguments)]
async fn audit_execution<C: ConnectionTrait>(
    db: &C,
    user_id: Option<i64>,
    (resource_type, resource_id, label): (ResourceType, i64, &str),
    origin: Origin,
    what: &str,
    device: &devices::Model,
    ip: &str,
    reason: Option<&str>,
) {
    let _ = AuditService::new(db)
        .log(
            AuditActor {
                user_id,
                ..AuditActor::default()
            },
            AuditEntryInput {
                action: AuditAction::Execute,
                resource_type,
                resource_id: Some(resource_id),
                resource_label: Some(label.to_owned()),
                description: Some(format!(
                    "{} executou {what} em {} ({ip}){}",
                    match origin {
                        Origin::Ai => "A IA",
                        Origin::User => "O operador",
                        Origin::Validation => "A validação",
                        Origin::Fleet => "A frota",
                    },
                    device.name,
                    reason
                        .map(|reason| format!(" — motivo: {reason}"))
                        .unwrap_or_default()
                )),
                changes: None,
            },
        )
        .await;
}

fn device_info(device: &devices::Model) -> DeviceInfo {
    let facts = DeviceFacts::from_device(device);
    DeviceInfo {
        id: device.id,
        name: device.name.clone(),
        ip: device.ip_address.clone().unwrap_or_default(),
        vendor: facts.vendor,
        model: facts.model,
        platform: facts.platform,
        firmware: facts.firmware,
    }
}

struct Shared {
    bus: Option<EventBus>,
    run_id: i64,
    device_id: i64,
}

impl Shared {
    fn observer(self: &Arc<Self>) -> Observer {
        let shared = self.clone();
        Arc::new(move |entry: &TranscriptEntry| {
            if let Some(bus) = &shared.bus {
                let event = if entry.kind.starts_with(runtime::STEP_PREFIX) {
                    RUN_STEP_EVENT
                } else {
                    RUN_OUTPUT_EVENT
                };
                bus.publish_ephemeral(
                    event,
                    json!({ "runId": shared.run_id, "deviceId": shared.device_id, "entry": entry }),
                );
            }
        })
    }
}

impl Prepared {
    fn gate(&self, bus: Option<EventBus>) -> Arc<dyn AccessGate> {
        match self.spec.approval {
            Approval::Auto => Arc::new(AllowAll),
            Approval::PerCall => Arc::new(InteractiveGate::new(
                ApprovalContext {
                    run_id: self.run.id,
                    device_id: self.spec.device.id,
                    device_name: self.spec.device.name.clone(),
                    plugin_name: self.spec.plugin.name.clone(),
                    action: self.spec.action.clone(),
                },
                bus,
                self.cancel.clone(),
            )),
        }
    }

    fn context(
        &self,
        effect: Effect,
        gate: Arc<dyn AccessGate>,
        observer: Observer,
    ) -> ExecutionContext {
        ExecutionContext {
            extras: self.extras.clone(),
            device: device_info(&self.spec.device),
            transports: self.package.manifest.transports.clone(),
            action_effect: effect,
            reason: self.spec.reason.clone(),
            credentials: self.credentials.clone(),
            transport: self.transport.clone(),
            gate,
            cancel: self.cancel.clone(),
            observer: Some(observer),
            limits: Limits::default(),
        }
    }
}

/// Executa até o fim e devolve a linha atualizada.
///
/// # Errors
///
/// Só erro do banco ao gravar o desfecho — falha da ação vira `failed` na
/// linha, não erro aqui.
pub async fn run(ctx: &AppContext, prepared: Prepared) -> AppResult<plugin_runs::Model> {
    let bus = EventBus::from_context(ctx).ok();
    let shared = Arc::new(Shared {
        bus: bus.clone(),
        run_id: prepared.run.id,
        device_id: prepared.spec.device.id,
    });
    if let Some(bus) = &bus {
        bus.publish_ephemeral(
            RUN_STARTED_EVENT,
            json!({ "run": view(&prepared.run, Some(&prepared.spec.plugin.name)) }),
        );
    }
    let gate = prepared.gate(bus.clone());
    let observer = shared.observer();

    let (output, transcript) = if prepared.spec.action == VALIDATION_ACTION {
        validate_suite(ctx, &prepared, gate, observer).await
    } else {
        let action = action_of(&prepared.package, &prepared.spec.action)?.clone();
        let outcome = runtime::execute(
            &prepared.package.script,
            &action.id,
            prepared.spec.params.clone(),
            prepared.context(action.effect, gate, observer),
        )
        .await;
        if action.id == DETECT_ACTION {
            if let Ok(output) = &outcome.output {
                remember_detection(
                    &ctx.db,
                    &prepared.spec.device,
                    &prepared.package.manifest,
                    output,
                )
                .await;
            }
        }
        (outcome.output, outcome.transcript)
    };

    lock_cancellations().remove(&prepared.run.id);
    let cancelled = prepared.cancel.is_cancelled();
    let mut active: plugin_runs::ActiveModel = prepared.run.clone().into();
    active.transcript = Set(serde_json::to_value(&transcript).unwrap_or_else(|_| json!([])));
    active.finished_at = Set(Some(chrono::Utc::now().into()));
    match output {
        Ok(output) => {
            active.status = Set("succeeded".to_owned());
            active.output = Set(Some(output));
        }
        Err(error) => {
            active.status = Set(if cancelled { "cancelled" } else { "failed" }.to_owned());
            active.error = Set(Some(error));
        }
    }
    let finished = active.update(&ctx.db).await?;
    if let Some(bus) = &bus {
        bus.publish_ephemeral(
            RUN_FINISHED_EVENT,
            json!({ "run": view(&finished, Some(&prepared.spec.plugin.name)) }),
        );
    }
    Ok(finished)
}

/// Executa em segundo plano; o desfecho chega pelo SSE.
pub fn spawn(ctx: AppContext, prepared: Prepared) -> i64 {
    let run_id = prepared.run_id();
    tokio::spawn(async move {
        if let Err(error) = run(&ctx, prepared).await {
            tracing::warn!(%error, run_id, "falha ao gravar o desfecho da execução do plugin");
        }
    });
    run_id
}

/// O `detect` devolveu firmware (e talvez modelo): vale guardar no cadastro,
/// porque é disso que a compatibilidade depende. Plugin de um sistema só que
/// leu o firmware no equipamento também **provou** o sistema — vira a
/// observação (`observed_os`), que vale mais que o fabricante do cadastro.
async fn remember_detection<C: ConnectionTrait>(
    db: &C,
    device: &devices::Model,
    manifest: &PluginManifest,
    output: &Value,
) {
    let text = |key: &str| {
        output
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(64).collect::<String>())
    };
    let firmware = text("firmware");
    let model = text("model");
    if firmware.is_none() && model.is_none() {
        return;
    }
    let mut active: devices::ActiveModel = device.clone().into();
    if let (Some(firmware), [platform]) = (&firmware, manifest.matcher.platforms.as_slice()) {
        active.observed_os = Set(Some(platform.clone()));
        active.observed_os_source = Set(Some(systems::source::PLUGIN.to_owned()));
        active.observed_os_reason = Set(Some(format!(
            "o plugin \"{}\" leu o sistema no equipamento (versão {firmware})",
            manifest.name
        )));
    }
    if let Some(firmware) = firmware {
        active.firmware_version = Set(Some(firmware));
    }
    if device
        .model
        .as_deref()
        .is_none_or(|current| current.trim().is_empty())
    {
        if let Some(model) = model {
            active.model = Set(Some(model));
        }
    }
    if let Err(error) = active.update(db).await {
        tracing::warn!(%error, device_id = device.id, "não foi possível gravar o firmware detectado");
    }
}

/// Roda a suíte funcional e grava a entrada de compatibilidade.
async fn validate_suite(
    ctx: &AppContext,
    prepared: &Prepared,
    gate: Arc<dyn AccessGate>,
    observer: Observer,
) -> (Result<Value, String>, Vec<TranscriptEntry>) {
    let package = &prepared.package;
    if package.tests.functional.is_empty() {
        return (Err("o plugin não tem testes funcionais".into()), Vec::new());
    }
    let mut transcript = Vec::new();
    let mut cases = Vec::new();
    let mut detected: Option<Value> = None;
    for test in &package.tests.functional {
        let Some(action) = package.manifest.action(&test.action) else {
            cases.push(
                json!({ "action": test.action, "passed": false, "message": "ação inexistente" }),
            );
            continue;
        };
        if action.effect == Effect::Write && !action.safe_to_retest {
            cases.push(json!({ "action": test.action, "passed": false,
                               "message": "ação de escrita fora da suíte funcional" }));
            continue;
        }
        let params = match params::validate(action.params.as_ref(), &test.params) {
            Ok(params) => params,
            Err(errors) => {
                cases.push(
                    json!({ "action": test.action, "passed": false, "message": errors.join("; ") }),
                );
                continue;
            }
        };
        let outcome = runtime::execute(
            &package.script,
            &action.id,
            params,
            prepared.context(action.effect, gate.clone(), observer.clone()),
        )
        .await;
        transcript.extend(outcome.transcript);
        let verdict = outcome.output.and_then(|output| {
            if action.id == DETECT_ACTION {
                detected = Some(output.clone());
            }
            testing::check_functional(test, &output)
        });
        cases.push(json!({
            "action": test.action,
            "passed": verdict.is_ok(),
            "message": verdict.err(),
        }));
        if prepared.cancel.is_cancelled() {
            break;
        }
    }
    if let Some(output) = &detected {
        remember_detection(
            &ctx.db,
            &prepared.spec.device,
            &prepared.package.manifest,
            output,
        )
        .await;
    }
    let passed = cases
        .iter()
        .all(|case| case.get("passed").and_then(Value::as_bool) == Some(true));

    // A compatibilidade descreve o equipamento como o `detect` o viu agora.
    let device = devices::Entity::find_by_id(prepared.spec.device.id)
        .one(&ctx.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| prepared.spec.device.clone());
    let facts = DeviceFacts::from_device(&device);
    let entry = CompatEntry {
        platform: Some(facts.platform),
        vendor: facts.vendor,
        model: facts.model,
        firmware: facts.firmware,
        status: if passed { "passed" } else { "failed" }.to_owned(),
        validated_at: chrono::Utc::now().to_rfc3339(),
        plugin_version: package.manifest.version.clone(),
        run_id: Some(prepared.run.id),
    };
    if let Err(error) = service::record_compatibility(&ctx.db, prepared.spec.plugin.id, entry).await
    {
        tracing::warn!(%error, "não foi possível gravar a compatibilidade");
    }
    let output = json!({ "passed": passed, "cases": cases });
    if passed {
        (Ok(output), transcript)
    } else {
        (
            Err(format!(
                "a validação falhou: {}",
                cases
                    .iter()
                    .filter(|case| case.get("passed").and_then(Value::as_bool) != Some(true))
                    .filter_map(|case| {
                        Some(format!(
                            "{} ({})",
                            case.get("action")?.as_str()?,
                            case.get("message")?.as_str().unwrap_or("falhou")
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            )),
            transcript,
        )
    }
}

/// Um acesso avulso da IA — um comando ou uma requisição, sem plugin. É o
/// que ela usa para explorar o equipamento enquanto escreve um plugin.
#[derive(Debug, Clone)]
pub enum AdHocRequest {
    Ssh { command: String },
    Telnet { command: String },
    Http { request: Value },
}

impl AdHocRequest {
    #[must_use]
    pub const fn transport(&self) -> TransportKind {
        match self {
            Self::Ssh { .. } => TransportKind::Ssh,
            Self::Telnet { .. } => TransportKind::Telnet,
            Self::Http { .. } => TransportKind::Http,
        }
    }

    /// O efeito provável, pela mesma classificação do runtime.
    #[must_use]
    pub fn classified_effect(&self) -> Effect {
        match self {
            Self::Ssh { command } | Self::Telnet { command } => effect::classify_command(command),
            Self::Http { request } => {
                if request.get("login").and_then(Value::as_bool) == Some(true) {
                    Effect::Read
                } else {
                    effect::classify_http(
                        request
                            .get("method")
                            .and_then(Value::as_str)
                            .unwrap_or("GET"),
                    )
                }
            }
        }
    }

    fn params(&self) -> Value {
        match self {
            Self::Ssh { command } => json!({ "kind": "ssh", "command": command }),
            Self::Telnet { command } => json!({ "kind": "telnet", "command": command }),
            Self::Http { request } => json!({ "kind": "http", "request": request }),
        }
    }
}

/// O "plugin" de uma chamada só: repassa o pedido ao `device`, pelas mesmas
/// portas (transporte, efeito, credencial, máscara) de um plugin de verdade.
const AD_HOC_SCRIPT: &str = r#"
fn access(device, params) {
    if params.kind == "ssh" { return device.ssh(params.command); }
    if params.kind == "telnet" { return device.telnet(params.command); }
    device.http(params.request)
}
"#;

pub struct AdHocSpec {
    pub device: devices::Model,
    pub request: AdHocRequest,
    /// Efeito declarado; vale o maior entre ele e o classificado.
    pub effect: Effect,
    pub reason: Option<String>,
    pub user_id: Option<i64>,
}

/// Executa um acesso avulso já aprovado (confirmação da ferramenta ou modo
/// automático) e espera o fim.
///
/// # Errors
///
/// Equipamento sem IP, credencial ausente, agente fora ou erro do banco. A
/// falha do acesso em si vira `failed` na linha, não erro aqui.
pub async fn run_ad_hoc(ctx: &AppContext, spec: AdHocSpec) -> AppResult<plugin_runs::Model> {
    let ip = device_ip(&spec.device)?;
    let transport_kind = spec.request.transport();
    let resolved = credentials::resolve(&ctx.db, spec.device.id, &[transport_kind]).await?;
    let transport = transport_for(ctx, resolved.via_probe_id).await?;
    let effect = spec.effect.max(spec.request.classified_effect());
    let run = insert_run(
        &ctx.db,
        NewRun {
            plugin_id: None,
            device: &spec.device,
            user_id: spec.user_id,
            action: transport_kind.as_str(),
            origin: Origin::Ai,
            params: spec.request.params(),
            batch_id: None,
        },
    )
    .await?;
    audit_execution(
        &ctx.db,
        spec.user_id,
        (ResourceType::Device, spec.device.id, &spec.device.name),
        Origin::Ai,
        &format!(
            "um acesso {} ({})",
            transport_kind.as_str().to_uppercase(),
            effect.as_str()
        ),
        &spec.device,
        &ip,
        spec.reason.as_deref(),
    )
    .await;

    let bus = EventBus::from_context(ctx).ok();
    let shared = Arc::new(Shared {
        bus: bus.clone(),
        run_id: run.id,
        device_id: spec.device.id,
    });
    let cancel = CancellationToken::new();
    lock_cancellations().insert(run.id, cancel.clone());
    let outcome = runtime::execute(
        AD_HOC_SCRIPT,
        "access",
        spec.request.params(),
        ExecutionContext {
            extras: Extras::default(),
            device: device_info(&spec.device),
            transports: vec![transport_kind],
            action_effect: effect,
            reason: spec.reason.clone(),
            credentials: resolved.credentials,
            transport,
            gate: Arc::new(AllowAll),
            cancel: cancel.clone(),
            observer: Some(shared.observer()),
            limits: Limits::default(),
        },
    )
    .await;
    lock_cancellations().remove(&run.id);

    let mut active: plugin_runs::ActiveModel = run.into();
    active.transcript =
        Set(serde_json::to_value(&outcome.transcript).unwrap_or_else(|_| json!([])));
    active.finished_at = Set(Some(chrono::Utc::now().into()));
    match outcome.output {
        Ok(output) => {
            active.status = Set("succeeded".to_owned());
            active.output = Set(Some(output));
        }
        Err(error) => {
            let status = if cancel.is_cancelled() {
                "cancelled"
            } else {
                "failed"
            };
            active.status = Set(status.to_owned());
            active.error = Set(Some(error));
        }
    }
    let finished = active.update(&ctx.db).await?;
    if let Some(bus) = &bus {
        bus.publish_ephemeral(RUN_FINISHED_EVENT, json!({ "run": view(&finished, None) }));
    }
    Ok(finished)
}

#[must_use]
pub fn view(run: &plugin_runs::Model, plugin_name: Option<&str>) -> PluginRunView {
    PluginRunView {
        id: run.id,
        plugin_id: run.plugin_id,
        plugin_name: plugin_name.map(str::to_owned),
        device_id: run.device_id,
        action: run.action.clone(),
        origin: run.origin.clone(),
        status: run.status.clone(),
        params: run.params.clone(),
        output: run.output.clone(),
        transcript: serde_json::from_value(run.transcript.clone()).unwrap_or_default(),
        error: run.error.clone(),
        created_at: run.created_at.to_rfc3339(),
        finished_at: run.finished_at.map(|at| at.to_rfc3339()),
    }
}

/// As últimas execuções de um equipamento.
///
/// # Errors
///
/// Erro do banco.
pub async fn recent<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
    limit: u64,
) -> AppResult<Vec<PluginRunView>> {
    let runs = plugin_runs::Entity::find()
        .filter(plugin_runs::Column::DeviceId.eq(device_id))
        .order_by_desc(plugin_runs::Column::Id)
        .limit(limit)
        .all(db)
        .await?;
    let names: HashMap<i64, String> = plugins::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|plugin| (plugin.id, plugin.name))
        .collect();
    Ok(runs
        .iter()
        .map(|run| {
            view(
                run,
                run.plugin_id
                    .and_then(|id| names.get(&id))
                    .map(String::as_str),
            )
        })
        .collect())
}

/// # Errors
///
/// Execução inexistente.
pub async fn find_view<C: ConnectionTrait>(db: &C, run_id: i64) -> AppResult<PluginRunView> {
    let run = plugin_runs::Entity::find_by_id(run_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Execução não encontrada."))?;
    let name = match run.plugin_id {
        Some(id) => plugins::Entity::find_by_id(id)
            .one(db)
            .await?
            .map(|p| p.name),
        None => None,
    };
    Ok(view(&run, name.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(status: &str) -> plugins::Model {
        plugins::Model {
            id: 1,
            slug: "x".into(),
            name: "x".into(),
            version: "1.0.0".into(),
            description: None,
            scope: "model".into(),
            device_id: None,
            source: "user".into(),
            status: status.into(),
            manifest: json!({}),
            script: String::new(),
            usage: String::new(),
            compatibility: json!([]),
            tests: json!({}),
            review: None,
            checksum: String::new(),
            last_test_at: None,
            last_test_ok: None,
            auto_enable: false,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn politica_de_aprovacao() {
        assert_eq!(
            approval_for(&plugin("active"), Origin::User, false),
            Approval::Auto
        );
        assert_eq!(
            approval_for(&plugin("draft"), Origin::User, false),
            Approval::PerCall
        );
        assert_eq!(
            approval_for(&plugin("tested"), Origin::Validation, false),
            Approval::PerCall
        );
        assert_eq!(
            approval_for(&plugin("draft"), Origin::Ai, false),
            Approval::PerCall
        );
        assert_eq!(
            approval_for(&plugin("draft"), Origin::Ai, true),
            Approval::Auto
        );
        assert_eq!(
            approval_for(&plugin("active"), Origin::Ai, false),
            Approval::Auto
        );
    }
}
