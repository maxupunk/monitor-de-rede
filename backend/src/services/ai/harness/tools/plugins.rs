//! Plugins de dispositivo pelo chat: reconhecer o equipamento, explorá-lo por
//! SSH/HTTP, escrever, testar, validar e executar plugins.
//!
//! Toda ferramenta que toca o equipamento ou grava plugin é
//! [`ToolKind::DeviceAccess`]: a IA propõe com `reason` e `effect`, o usuário
//! confirma cada chamada — ou ligou o "Aceitar automaticamente" para aquela
//! conversa e aquele equipamento, ciente do risco. As de consulta (guia,
//! listagem, leitura do plugin, testes com fixtures) são passivas: não saem
//! da central.
//!
//! Operar plugin é coisa de administrador (a mesma régua das rotas HTTP);
//! quem conversa sem esse perfil recebe o motivo como dado.

use std::str::FromStr;

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

use super::{lookup::find_device, AiToolHandler, ToolArgs, ToolGroup, ToolKind, ToolOutput};
use crate::{
    models::{devices, plugins as plugin_rows, users},
    services::{
        ai::knowledge::PLUGIN_AUTHORING_GUIDE,
        plugins::{
            actions, auto_accept,
            effect::{self, Effect},
            fingerprint,
            package::PluginPackage,
            runs::{self, AdHocRequest, AdHocSpec, Origin, RunSpec, VALIDATION_ACTION},
            service::{self, source},
        },
        shared::{
            errors::{AppError, AppResult},
            text::truncate_chars,
        },
        users::Role,
    },
};

/// Quanto da saída do equipamento volta para a IA.
const OUTPUT_FOR_MODEL: usize = 6_000;

fn device_schema() -> Value {
    json!({ "type": "string", "description": "Dispositivo: id, nome ou IP" })
}

fn reason_schema() -> Value {
    json!({
        "type": "string",
        "description": "Por que este acesso é necessário, numa frase — o usuário lê antes de aprovar"
    })
}

fn effect_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["read", "write"],
        "description": "read = só lê; write = altera o equipamento. Declare com honestidade: o sistema reclassifica e vale o mais restritivo"
    })
}

async fn device_arg(ctx: &AppContext, args: &ToolArgs) -> Result<devices::Model, ToolOutput> {
    let Some(identifier) = args.text("device") else {
        return Err(ToolOutput::not_found(
            "Informe o dispositivo (id, nome ou IP).",
        ));
    };
    match find_device(&ctx.db, &identifier).await {
        Ok(Some(device)) => Ok(device),
        Ok(None) => Err(ToolOutput::not_found(format!(
            "Dispositivo '{identifier}' não encontrado (ou ambíguo)."
        ))),
        Err(error) => Err(ToolOutput::not_found(error.to_string())),
    }
}

/// Plugin por id ou slug. Com slug repetido, o ativo mais recente.
async fn plugin_arg(ctx: &AppContext, args: &ToolArgs) -> Result<plugin_rows::Model, ToolOutput> {
    let Some(identifier) = args.text("plugin") else {
        return Err(ToolOutput::not_found("Informe o plugin (id ou slug)."));
    };
    if let Ok(id) = identifier.parse::<i64>() {
        if let Ok(model) = service::find(&ctx.db, id).await {
            return Ok(model);
        }
    }
    let candidates = plugin_rows::Entity::find()
        .filter(plugin_rows::Column::Slug.eq(identifier.clone()))
        .order_by_desc(plugin_rows::Column::Id)
        .all(&ctx.db)
        .await
        .unwrap_or_default();
    candidates
        .iter()
        .find(|model| model.status == service::status::ACTIVE)
        .or_else(|| candidates.first())
        .cloned()
        .ok_or_else(|| ToolOutput::not_found(format!("Plugin '{identifier}' não encontrado.")))
}

fn declared_effect(args: &ToolArgs) -> Effect {
    if args.text("effect").as_deref() == Some("read") {
        Effect::Read
    } else {
        Effect::Write
    }
}

/// Só administrador opera plugin — a mesma regra das rotas HTTP.
async fn require_admin(ctx: &AppContext, args: &ToolArgs) -> AppResult<i64> {
    let user_id = args
        .actor()
        .user_id
        .ok_or_else(|| AppError::forbidden("Usuário não identificado."))?;
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::forbidden("Usuário não encontrado."))?;
    let role = Role::from_str(&user.role)?;
    if role.can_manage_plugins() {
        Ok(user_id)
    } else {
        Err(AppError::forbidden(
            "Só administradores acessam equipamentos e operam plugins.",
        ))
    }
}

async fn auto_accept_active(ctx: &AppContext, args: &ToolArgs, device_id: i64) -> bool {
    match args.conversation() {
        Some(key) => auto_accept::active(&ctx.db, key, device_id)
            .await
            .ok()
            .flatten()
            .is_some(),
        None => false,
    }
}

fn effect_label(effect: Effect) -> &'static str {
    match effect {
        Effect::Read => "leitura",
        Effect::Write => "ALTERA O EQUIPAMENTO",
    }
}

fn describe_device(device: &devices::Model) -> String {
    format!(
        "{} ({})",
        device.name,
        device.ip_address.as_deref().unwrap_or("sem IP")
    )
}

async fn device_label(ctx: &AppContext, args: &ToolArgs) -> String {
    match device_arg(ctx, args).await {
        Ok(device) => describe_device(&device),
        Err(_) => args.text("device").unwrap_or_else(|| "?".into()),
    }
}

fn reason_line(args: &ToolArgs) -> String {
    format!(
        "Motivo: {}",
        args.text("reason")
            .unwrap_or_else(|| "(a IA não informou)".into())
    )
}

/// O desfecho de uma execução, no formato que a IA lê.
fn run_result(run: &crate::models::plugin_runs::Model) -> Value {
    let transcript: Vec<Value> = serde_json::from_value::<Vec<Value>>(run.transcript.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|entry| {
            json!({
                "kind": entry.get("kind"),
                "request": entry.get("request"),
                "status": entry.get("status"),
                "output": entry
                    .get("output")
                    .and_then(Value::as_str)
                    .map(|text| truncate_chars(text, 1_500)),
            })
        })
        .collect();
    json!({
        "run_id": run.id,
        "status": run.status,
        "output": run.output.as_ref().map(|output| {
            let text = output.to_string();
            if text.chars().count() > OUTPUT_FOR_MODEL {
                Value::String(truncate_chars(&text, OUTPUT_FOR_MODEL))
            } else {
                output.clone()
            }
        }),
        "error": run.error,
        "transcript": transcript,
    })
}

// --- Consulta ---------------------------------------------------------------

pub struct AuthoringGuide;

#[async_trait]
impl AiToolHandler for AuthoringGuide {
    fn name(&self) -> &'static str {
        "get_plugin_authoring_guide"
    }

    fn description(&self) -> &'static str {
        "Skill de autoria de plugins de dispositivo: fluxo obrigatório, formato do pacote, API do script (Rhai), testes e boas práticas. Leia ANTES de criar ou alterar um plugin."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Devices
    }

    async fn execute(&self, _ctx: &AppContext, _args: &ToolArgs) -> AppResult<ToolOutput> {
        Ok(ToolOutput::data(json!({ "guide": PLUGIN_AUTHORING_GUIDE })))
    }
}

pub struct ListDevicePlugins;

#[async_trait]
impl AiToolHandler for ListDevicePlugins {
    fn name(&self) -> &'static str {
        "list_device_plugins"
    }

    fn description(&self) -> &'static str {
        "Plugins aplicáveis a um dispositivo, com compatibilidade (validated/likely/possible/incompatible), ações, status, credenciais cadastradas (sem senha) e agentes. Use antes de criar um plugin novo."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "device": device_schema() },
            "required": ["device"]
        })
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Devices
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let device = match device_arg(ctx, args).await {
            Ok(device) => device,
            Err(output) => return Ok(output),
        };
        let view = actions::device_view(ctx, device.id).await?;
        Ok(ToolOutput::data(json!({
            "device": describe_device(&device),
            "platform": view.platform,
            "firmware": view.firmware,
            "plugins": view.plugins.iter().map(|item| json!({
                "id": item.plugin.id,
                "slug": item.plugin.slug,
                "name": item.plugin.name,
                "version": item.plugin.version,
                "status": item.plugin.status,
                "compat": item.compat,
                "installed": item.installed,
                "reasons": item.reasons,
                "actions": item.plugin.actions.iter()
                    .map(|action| format!("{} ({}, {})", action.id, action.title, action.effect.as_str()))
                    .collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "credentials": view.credentials.iter().map(|credential| json!({
                "kind": credential.kind,
                "username": credential.username,
                "storage": credential.storage,
                "port": credential.port,
                "ready": credential.has_stored_secret || credential.session_active,
            })).collect::<Vec<_>>(),
            "agents": view.agents,
        })))
    }
}

pub struct GetPlugin;

#[async_trait]
impl AiToolHandler for GetPlugin {
    fn name(&self) -> &'static str {
        "get_plugin"
    }

    fn description(&self) -> &'static str {
        "Um plugin completo: manifesto, script, uso (como usar cada ação), compatibilidade validada, testes, status e risco da revisão de segurança."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "plugin": { "type": "string", "description": "Id ou slug do plugin" } },
            "required": ["plugin"]
        })
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Devices
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let model = match plugin_arg(ctx, args).await {
            Ok(model) => model,
            Err(output) => return Ok(output),
        };
        let detail = service::detail(&ctx.db, model.id).await?;
        Ok(ToolOutput::data(json!({
            "id": model.id,
            "status": model.status,
            "source": model.source,
            "scope": model.scope,
            "risk": detail.summary.risk,
            "package": detail.package,
        })))
    }
}

pub struct RunPluginTests;

#[async_trait]
impl AiToolHandler for RunPluginTests {
    fn name(&self) -> &'static str {
        "run_plugin_tests"
    }

    fn description(&self) -> &'static str {
        "Roda os testes unitários de um plugin (script de verdade contra as respostas gravadas nas fixtures, sem tocar o equipamento) e devolve problemas de validação e falhas."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "plugin": { "type": "string", "description": "Id ou slug do plugin" } },
            "required": ["plugin"]
        })
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Devices
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let model = match plugin_arg(ctx, args).await {
            Ok(model) => model,
            Err(output) => return Ok(output),
        };
        let (saved, report) = service::run_tests(&ctx.db, model.id).await?;
        Ok(ToolOutput::data(test_summary(&saved, &report)))
    }
}

fn test_summary(
    model: &plugin_rows::Model,
    report: &crate::services::plugins::testing::TestReport,
) -> Value {
    json!({
        "plugin_id": model.id,
        "status": model.status,
        "passed": report.passed,
        "total": report.total,
        "failed": report.failed,
        "problems": report.problems,
        // Não reprovam, mas a tela fica ruim: corrija e rode de novo.
        "usability": report.hints.iter().map(|hint| format!("{}: {}", hint.at, hint.message)).collect::<Vec<_>>(),
        "failures": report.cases.iter().filter(|case| !case.passed).map(|case| json!({
            "test": case.name,
            "message": case.message,
            "output": case.output,
            "calls": case.transcript.iter().map(|entry| format!("{}: {}", entry.kind, entry.request)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

// --- Acesso ao equipamento (sempre com o usuário) ----------------------------

pub struct FingerprintDevice;

#[async_trait]
impl AiToolHandler for FingerprintDevice {
    fn name(&self) -> &'static str {
        "fingerprint_device"
    }

    fn description(&self) -> &'static str {
        "Reconhece o equipamento sem credencial e sem alterar nada: portas de gerência abertas, banner SSH, título e servidor da página web, plataforma deduzida e plugins que casam. Acessa o equipamento: pede aprovação."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "device": device_schema(), "reason": reason_schema() },
            "required": ["device", "reason"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::DeviceAccess
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        Ok(format!(
            "Sondar {}: portas {:?}, banner SSH e página web (GET /). Efeito: leitura.\n{}",
            device_label(ctx, args).await,
            fingerprint::MANAGEMENT_PORTS,
            reason_line(args)
        ))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        require_admin(ctx, args).await?;
        let device = match device_arg(ctx, args).await {
            Ok(device) => device,
            Err(output) => return Ok(output),
        };
        let found = fingerprint::fingerprint(&ctx.db, &device).await?;
        Ok(ToolOutput::data(
            serde_json::to_value(found).unwrap_or_default(),
        ))
    }
}

pub struct DeviceSshExec;

#[async_trait]
impl AiToolHandler for DeviceSshExec {
    fn name(&self) -> &'static str {
        "device_ssh_exec"
    }

    fn description(&self) -> &'static str {
        "Executa UM comando no equipamento por SSH (ou Telnet) com a credencial cadastrada — para explorar enquanto escreve um plugin. Prefira comandos de leitura. Use {{password}} se precisar da senha (ex.: sudo -S). Cada chamada pede aprovação do usuário."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": device_schema(),
                "command": { "type": "string", "description": "Comando exato, uma linha" },
                "transport": { "type": "string", "enum": ["ssh", "telnet"], "description": "Padrão: ssh" },
                "effect": effect_schema(),
                "reason": reason_schema()
            },
            "required": ["device", "command", "effect", "reason"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::DeviceAccess
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let command = args.text("command").unwrap_or_default();
        let effect = declared_effect(args).max(effect::classify_command(&command));
        Ok(format!(
            "Executar via {} em {}:\n$ {command}\nEfeito: {}\n{}",
            args.text("transport")
                .unwrap_or_else(|| "ssh".into())
                .to_uppercase(),
            device_label(ctx, args).await,
            effect_label(effect),
            reason_line(args)
        ))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let user_id = require_admin(ctx, args).await?;
        let device = match device_arg(ctx, args).await {
            Ok(device) => device,
            Err(output) => return Ok(output),
        };
        let command = args.required_text("command", "Informe o comando.")?;
        let request = if args.text("transport").as_deref() == Some("telnet") {
            AdHocRequest::Telnet { command }
        } else {
            AdHocRequest::Ssh { command }
        };
        let run = runs::run_ad_hoc(
            ctx,
            AdHocSpec {
                device,
                request,
                effect: declared_effect(args),
                reason: args.text("reason"),
                user_id: Some(user_id),
            },
        )
        .await?;
        Ok(ToolOutput::data(run_result(&run)))
    }
}

pub struct DeviceHttpRequest;

#[async_trait]
impl AiToolHandler for DeviceHttpRequest {
    fn name(&self) -> &'static str {
        "device_http_request"
    }

    fn description(&self) -> &'static str {
        "Uma requisição HTTP à interface web do equipamento (caminho relativo, ex.: /cgi-bin/luci). Cookies de login são guardados só nesta chamada. Use {{username}}/{{password}} no form de login e login=true nele. Cada chamada pede aprovação do usuário."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": device_schema(),
                "method": { "type": "string", "description": "GET, POST, PUT, DELETE… (padrão GET)" },
                "path": { "type": "string", "description": "Caminho relativo começando com /" },
                "form": { "type": "object", "description": "Campos de formulário (application/x-www-form-urlencoded)" },
                "json": { "description": "Corpo JSON" },
                "body": { "type": "string", "description": "Corpo cru" },
                "headers": { "type": "object", "description": "Cabeçalhos extras" },
                "https": { "type": "boolean" },
                "login": { "type": "boolean", "description": "true só no POST de autenticação" },
                "basic_auth": { "type": "boolean", "description": "Envia a credencial HTTP como Basic Auth" },
                "effect": effect_schema(),
                "reason": reason_schema()
            },
            "required": ["device", "path", "effect", "reason"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::DeviceAccess
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let method = args.text("method").unwrap_or_else(|| "GET".into());
        let request = AdHocRequest::Http {
            request: json!({ "method": method, "login": args.flag("login") }),
        };
        let effect = declared_effect(args).max(request.classified_effect());
        Ok(format!(
            "Requisição HTTP em {}:\n{} {}{}\nEfeito: {}\n{}",
            device_label(ctx, args).await,
            method.to_uppercase(),
            args.text("path").unwrap_or_else(|| "/".into()),
            if args.flag("login") { " (login)" } else { "" },
            effect_label(effect),
            reason_line(args)
        ))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let user_id = require_admin(ctx, args).await?;
        let device = match device_arg(ctx, args).await {
            Ok(device) => device,
            Err(output) => return Ok(output),
        };
        let path = args.required_text("path", "Informe o caminho (path).")?;
        let mut request = json!({
            "method": args.text("method").unwrap_or_else(|| "GET".into()),
            "path": path,
            "login": args.flag("login"),
            "basic_auth": args.flag("basic_auth"),
        });
        for key in ["form", "json", "body", "headers", "https"] {
            if let Some(value) = args.raw(key) {
                request[key] = value.clone();
            }
        }
        let run = runs::run_ad_hoc(
            ctx,
            AdHocSpec {
                device,
                request: AdHocRequest::Http { request },
                effect: declared_effect(args),
                reason: args.text("reason"),
                user_id: Some(user_id),
            },
        )
        .await?;
        Ok(ToolOutput::data(run_result(&run)))
    }
}

pub struct SavePluginDraft;

#[async_trait]
impl AiToolHandler for SavePluginDraft {
    fn name(&self) -> &'static str {
        "save_plugin_draft"
    }

    fn description(&self) -> &'static str {
        "Grava um plugin como RASCUNHO (nunca ativo) e roda os testes unitários na hora. Para alterar um existente, informe plugin_id (suba a versão). exclusive_device grava como exclusivo de um equipamento. Pede aprovação do usuário."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "package": { "type": "object", "description": "O pacote completo: format, manifest, script, usage, tests (ver o guia)" },
                "plugin_id": { "type": "integer", "description": "Atualizar este plugin em vez de criar" },
                "exclusive_device": { "type": "string", "description": "Dispositivo (id, nome ou IP) quando o plugin é só dele" },
                "device": { "type": "string", "description": "Dispositivo em que o plugin está sendo criado (contexto do modo automático)" },
                "reason": reason_schema()
            },
            "required": ["package", "reason"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::DeviceAccess
    }

    async fn preview(&self, _ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let package = args.raw("package").cloned().unwrap_or_default();
        let manifest = package.get("manifest").cloned().unwrap_or_default();
        let actions: Vec<String> = manifest
            .get("actions")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .map(|action| {
                        format!(
                            "{} ({})",
                            action.get("title").and_then(Value::as_str).unwrap_or("?"),
                            action.get("effect").and_then(Value::as_str).unwrap_or("?")
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(format!(
            "{} o plugin \"{}\" v{} como rascunho{}. Ações: {}.\nUm rascunho só roda com aprovação a cada acesso, até você testá-lo e ativá-lo.\n{}",
            if args.integer("plugin_id").is_some() { "Atualizar" } else { "Salvar" },
            manifest.get("name").and_then(Value::as_str).unwrap_or("?"),
            manifest.get("version").and_then(Value::as_str).unwrap_or("?"),
            args.text("exclusive_device")
                .map(|device| format!(" exclusivo de {device}"))
                .unwrap_or_default(),
            actions.join(", "),
            reason_line(args)
        ))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        require_admin(ctx, args).await?;
        let Some(raw) = args.raw("package") else {
            return Ok(ToolOutput::not_found("Informe o pacote (package)."));
        };
        let package: PluginPackage = match serde_json::from_value(raw.clone()) {
            Ok(package) => package,
            Err(error) => {
                return Ok(ToolOutput::not_found(format!(
                    "Pacote fora do formato: {error}. Leia get_plugin_authoring_guide."
                )))
            }
        };
        let exclusive = match args.text("exclusive_device") {
            Some(identifier) => match find_device(&ctx.db, &identifier).await? {
                Some(device) => Some(device.id),
                None => {
                    return Ok(ToolOutput::not_found(format!(
                        "Dispositivo '{identifier}' não encontrado."
                    )))
                }
            },
            None => None,
        };
        let saved = match args.integer("plugin_id") {
            Some(id) => service::update(&ctx.db, id, &package).await,
            None => service::create(&ctx.db, &package, exclusive, source::AI).await,
        };
        let saved = match saved {
            Ok(saved) => saved,
            // Erro de validação volta como dado: a IA corrige e salva de novo.
            Err(
                error @ (AppError::Validation(_)
                | AppError::Conflict(_)
                | AppError::BusinessRule(_)),
            ) => return Ok(ToolOutput::not_found(error.to_string())),
            Err(error) => return Err(error),
        };
        let (tested, report) = service::run_tests(&ctx.db, saved.id).await?;
        Ok(ToolOutput::data(test_summary(&tested, &report)))
    }
}

pub struct RunPluginAction;

#[async_trait]
impl AiToolHandler for RunPluginAction {
    fn name(&self) -> &'static str {
        "run_plugin_action"
    }

    fn description(&self) -> &'static str {
        "Executa uma ação de um plugin no equipamento real e espera o resultado. Plugin que não está ativo pede aprovação a cada acesso durante a execução. Pede aprovação do usuário."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": device_schema(),
                "plugin": { "type": "string", "description": "Id ou slug do plugin" },
                "action": { "type": "string", "description": "Id da ação" },
                "params": { "type": "object", "description": "Parâmetros da ação, conforme o esquema do manifesto" },
                "reason": reason_schema()
            },
            "required": ["device", "plugin", "action", "reason"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::DeviceAccess
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        let action_id = args.text("action").unwrap_or_default();
        let (plugin_name, action_line, active) = match plugin_arg(ctx, args).await {
            Ok(model) => {
                let package = service::package_of(&model)?;
                let line = package.manifest.action(&action_id).map_or_else(
                    || format!("{action_id} (ação inexistente)"),
                    |action| format!("{} — efeito: {}", action.title, effect_label(action.effect)),
                );
                (
                    model.name.clone(),
                    line,
                    model.status == service::status::ACTIVE,
                )
            }
            Err(_) => (
                args.text("plugin").unwrap_or_else(|| "?".into()),
                action_id,
                false,
            ),
        };
        Ok(format!(
            "Executar \"{action_line}\" do plugin {plugin_name} em {}.\nParâmetros: {}{}\n{}",
            device_label(ctx, args).await,
            args.raw("params")
                .map_or_else(|| "nenhum".into(), ToString::to_string),
            if active {
                String::new()
            } else {
                "\nO plugin ainda não está ativo: cada acesso ao equipamento pedirá aprovação."
                    .into()
            },
            reason_line(args)
        ))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let user_id = require_admin(ctx, args).await?;
        let device = match device_arg(ctx, args).await {
            Ok(device) => device,
            Err(output) => return Ok(output),
        };
        let plugin = match plugin_arg(ctx, args).await {
            Ok(plugin) => plugin,
            Err(output) => return Ok(output),
        };
        let action = args.required_text("action", "Informe a ação.")?;
        run_plugin(ctx, args, device, plugin, action, user_id).await
    }
}

async fn run_plugin(
    ctx: &AppContext,
    args: &ToolArgs,
    device: devices::Model,
    plugin: plugin_rows::Model,
    action: String,
    user_id: i64,
) -> AppResult<ToolOutput> {
    let auto = auto_accept_active(ctx, args, device.id).await;
    let approval = runs::approval_for(&plugin, Origin::Ai, auto);
    let spec = RunSpec {
        plugin,
        device,
        action,
        params: args.raw("params").cloned().unwrap_or(Value::Null),
        origin: Origin::Ai,
        user_id: Some(user_id),
        reason: args.text("reason"),
        approval,
        batch_id: None,
    };
    let prepared = match runs::prepare(ctx, spec).await {
        Ok(prepared) => prepared,
        Err(
            error @ (AppError::Validation(_)
            | AppError::BusinessRule(_)
            | AppError::Conflict(_)
            | AppError::NotFound(_)),
        ) => return Ok(ToolOutput::not_found(error.to_string())),
        Err(error) => return Err(error),
    };
    let run = runs::run(ctx, prepared).await?;
    Ok(ToolOutput::data(run_result(&run)))
}

pub struct ValidatePlugin;

#[async_trait]
impl AiToolHandler for ValidatePlugin {
    fn name(&self) -> &'static str {
        "validate_plugin"
    }

    fn description(&self) -> &'static str {
        "Roda os testes funcionais do plugin no equipamento real (só ações de leitura) e registra o resultado na lista de compatibilidade (modelo/firmware validados). Pede aprovação do usuário."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "device": device_schema(),
                "plugin": { "type": "string", "description": "Id ou slug do plugin" },
                "reason": reason_schema()
            },
            "required": ["device", "plugin", "reason"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::DeviceAccess
    }

    async fn preview(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<String> {
        Ok(format!(
            "Validar o plugin {} em {}: roda os testes funcionais (só leitura) e registra a compatibilidade.\n{}",
            args.text("plugin").unwrap_or_else(|| "?".into()),
            device_label(ctx, args).await,
            reason_line(args)
        ))
    }

    async fn execute(&self, ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let user_id = require_admin(ctx, args).await?;
        let device = match device_arg(ctx, args).await {
            Ok(device) => device,
            Err(output) => return Ok(output),
        };
        let plugin = match plugin_arg(ctx, args).await {
            Ok(plugin) => plugin,
            Err(output) => return Ok(output),
        };
        run_plugin(
            ctx,
            args,
            device,
            plugin,
            VALIDATION_ACTION.to_owned(),
            user_id,
        )
        .await
    }
}

/// O dispositivo a que uma chamada de acesso se refere — é por ele que o modo
/// automático é procurado.
pub async fn target_device_id(ctx: &AppContext, arguments: &Value) -> Option<i64> {
    let args = ToolArgs::from_value(arguments.clone());
    let identifier = args
        .text("device")
        .or_else(|| args.text("exclusive_device"))?;
    find_device(&ctx.db, &identifier)
        .await
        .ok()
        .flatten()
        .map(|device| device.id)
}
