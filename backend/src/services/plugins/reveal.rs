//! Revelar um segredo do equipamento para quem pediu — ex.: a senha atual de
//! uma rede Wi-Fi, para o operador conferir ao editar.
//!
//! É a exceção controlada à regra "segredo do equipamento não sai dele", e
//! por isso tem um caminho só seu:
//!
//! * só ações de leitura marcadas `reveals: true`; a execução comum
//!   ([`super::runs::prepare`]) as recusa — a IA, o lote da frota, a
//!   validação funcional e a aba nunca as rodam;
//! * roda na hora e o valor volta **só na resposta** a quem pediu: não grava
//!   execução, saída nem transcrição, e não publica nada no SSE (que chega a
//!   todas as abas abertas);
//! * só com o plugin ativo e instalado no equipamento (sem aprovação passo a
//!   passo pendurada numa requisição);
//! * a auditoria registra **que** o segredo foi revelado, por quem e onde —
//!   nunca o valor.

use std::sync::Arc;

use loco_rs::app::AppContext;
use serde_json::{Map, Value};
use tokio_util::sync::CancellationToken;

use super::{
    credentials,
    effect::Effect,
    gate::AllowAll,
    params,
    runs::{self, device_ip, transport_for},
    runtime::{self, ExecutionContext, Limits},
    service::{self, status},
};
use crate::{
    models::devices,
    services::{
        audit::{AuditAction, AuditActor, AuditEntryInput, AuditService, ResourceType},
        shared::errors::{AppError, AppResult},
    },
};

/// O que a execução comum responde quando alguém tenta rodar uma ação que
/// revela segredo fora deste caminho.
pub const ONLY_HERE: &str =
    "Esta ação revela um segredo do equipamento e só roda pela tela de edição, para quem pediu — nunca em lote, pela IA ou no histórico.";

/// Só os parâmetros que a ação conhece: a tela manda os valores do formulário
/// de edição (nome da rede…) e cada ação pega o que precisa.
fn known_params(schema: Option<&Value>, given: &Value) -> Value {
    let Some(properties) = schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
    else {
        return Value::Object(Map::new());
    };
    let picked = given
        .as_object()
        .map(|given| {
            given
                .iter()
                .filter(|(name, _)| properties.contains_key(*name))
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    Value::Object(picked)
}

/// Roda a ação que revela e devolve a saída dela.
///
/// # Errors
///
/// Ação que não revela, plugin inativo ou não instalado, parâmetros
/// inválidos, credencial ou transporte indisponível, ou a falha da ação.
pub async fn reveal(
    ctx: &AppContext,
    device: &devices::Model,
    plugin_id: i64,
    action_id: &str,
    given: &Value,
    user_id: i64,
) -> AppResult<Value> {
    let plugin = service::find(&ctx.db, plugin_id).await?;
    let package = service::package_of(&plugin)?;
    let action = package
        .manifest
        .action(action_id)
        .ok_or_else(|| AppError::not_found(format!("O plugin não tem a ação `{action_id}`.")))?
        .clone();
    if !action.reveals || action.effect != Effect::Read {
        return Err(AppError::business_rule(
            "Esta ação não revela segredo; use a execução normal.",
        ));
    }
    if plugin.status != status::ACTIVE {
        return Err(AppError::business_rule(
            "Ative o plugin para ler segredos do equipamento.",
        ));
    }
    if plugin.device_id.is_some_and(|only| only != device.id)
        || !service::is_installed(&ctx.db, device.id, plugin.id).await?
    {
        return Err(AppError::business_rule(
            "O plugin não está instalado neste equipamento.",
        ));
    }
    let ip = device_ip(device)?;
    let params_value = params::validate(
        action.params.as_ref(),
        &known_params(action.params.as_ref(), given),
    )
    .map_err(|errors| AppError::validation(errors.join("; ")))?;

    let resolved = credentials::resolve(&ctx.db, device.id, &package.manifest.transports).await?;
    let transport = transport_for(ctx, resolved.via_probe_id).await?;
    let extras = runs::load_extras(&ctx.db, &plugin, &package, device.id).await?;
    let outcome = runtime::execute(
        &package.script,
        &action.id,
        params_value,
        ExecutionContext {
            extras,
            device: runs::device_info(device),
            transports: package.manifest.transports.clone(),
            action_effect: Effect::Read,
            reason: None,
            credentials: resolved.credentials,
            transport,
            gate: Arc::new(AllowAll),
            cancel: CancellationToken::new(),
            // Sem observador: nenhum passo vai para o SSE.
            observer: None,
            limits: Limits::default(),
        },
    )
    .await;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor {
                user_id: Some(user_id),
                ..AuditActor::default()
            },
            AuditEntryInput {
                action: AuditAction::Execute,
                resource_type: ResourceType::Plugin,
                resource_id: Some(plugin.id),
                resource_label: Some(plugin.name.clone()),
                description: Some(format!(
                    "O operador revelou um segredo (`{}`) de {} ({ip}){}",
                    action.id,
                    device.name,
                    if outcome.output.is_ok() {
                        ""
                    } else {
                        " — sem sucesso"
                    }
                )),
                changes: None,
            },
        )
        .await;
    outcome.output.map_err(AppError::business_rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn so_os_parametros_que_a_acao_conhece_seguem() {
        let schema = json!({ "type": "object", "properties": { "ssid": { "type": "string" } } });
        let given = json!({ "ssid": "Loja", "encryption": "psk2", "band_2g": true });
        assert_eq!(
            known_params(Some(&schema), &given),
            json!({ "ssid": "Loja" })
        );
        assert_eq!(known_params(None, &given), json!({}));
    }
}
