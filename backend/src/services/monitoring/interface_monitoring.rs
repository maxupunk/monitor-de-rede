//! Observa o estado das interfaces coletadas via SNMP (§8.2).
//!
//! O serviço não decide o que é alerta: publica os fatos no feed em tempo real
//! e entrega o mesmo conjunto ao motor de alertas. Políticas como "downgrade de
//! negociação é um aviso" vivem em "Regras Configuradas" (catálogo
//! `interface_speed_downgrade`), podendo ser ajustadas ou desligadas pelo
//! operador sem alterar código.

use loco_rs::app::AppContext;
use serde_json::{json, Value};

use crate::{
    models::{device_interfaces, devices},
    services::{
        alerts::{
            contracts::{AlertEvaluationContext, AlertEvaluationScope, AlertScopeKey},
            datasets::interface_state::{self, InterfaceFacts},
            fields, manager,
        },
        events::EventBus,
        monitoring::link_speed::format_speed,
        shared::errors::AppResult,
    },
};

/// Avalia o que mudou na interface desde a coleta anterior.
///
/// # Errors
///
/// Propaga erro do banco vindo do motor de alertas.
pub async fn evaluate_interface_state(
    ctx: &AppContext,
    device: &devices::Model,
    interface: &device_interfaces::Model,
    previous_oper_status: Option<&str>,
    previous_speed: Option<i64>,
) -> AppResult<()> {
    let dataset = interface_state::build(&InterfaceFacts {
        name: &interface.name,
        oper_status: interface.oper_status.as_deref(),
        speed: interface.speed,
        previous_oper_status,
        previous_speed,
    });
    let message = interface_state::describe(&dataset);

    if interface_state::has_transition(&dataset) {
        publish_transitions(ctx, device, interface, &dataset, &message).await;
    }

    let mut data = serde_json::Map::new();
    data.insert("eventType".into(), json!("interface_state"));
    data.insert("interfaceId".into(), json!(interface.id));
    data.insert("ifIndex".into(), json!(interface.snmp_index));
    for (key, value) in &dataset {
        data.insert(key.clone(), value.clone());
    }

    manager::evaluate(
        ctx,
        &AlertEvaluationContext {
            scope: AlertEvaluationScope {
                site_id: device.site_id,
                device_id: Some(device.id),
                monitor_id: None,
            },
            scope_key: AlertScopeKey::interface(interface.id),
            target_label: format!("{} / {}", device.name, interface.name),
            dataset: dataset.clone(),
            message: Some(message),
            data,
            recovered: interface_state::is_recovery(&dataset),
            // Interface não tem "warning": ou houve transição ou voltou.
            degraded: false,
        },
    )
    .await
}

/// Feed em tempo real: os fatos observados, independentemente de alertar.
async fn publish_transitions(
    ctx: &AppContext,
    device: &devices::Model,
    interface: &device_interfaces::Model,
    dataset: &serde_json::Map<String, Value>,
    message: &str,
) {
    let Ok(bus) = EventBus::from_context(ctx) else {
        return;
    };
    let base = json!({
        "deviceId": device.id,
        "deviceName": device.name,
        "interfaceId": interface.id,
        "ifName": interface.name,
        "ifIndex": interface.snmp_index,
        "message": message,
    });
    let merged = |extra: Value| {
        let mut object = base.as_object().cloned().unwrap_or_default();
        if let Value::Object(extra) = extra {
            object.extend(extra);
        }
        Value::Object(object)
    };

    if let Some(transition) = dataset
        .get(fields::INTERFACE_STATUS_TRANSITION)
        .and_then(Value::as_str)
    {
        let payload = merged(json!({
            "previousStatus": dataset.get(fields::INTERFACE_PREVIOUS_OPER_STATUS),
            "currentStatus": dataset.get(fields::INTERFACE_OPER_STATUS),
            "transition": transition,
        }));
        emit(ctx, &bus, "interface:status_change", payload).await;
    }

    if let Some(transition) = dataset
        .get(fields::INTERFACE_SPEED_TRANSITION)
        .and_then(Value::as_str)
    {
        let previous = dataset
            .get(fields::INTERFACE_PREVIOUS_SPEED_BPS)
            .and_then(Value::as_i64);
        let current = dataset
            .get(fields::INTERFACE_SPEED_BPS)
            .and_then(Value::as_i64);
        let payload = merged(json!({
            "previousSpeed": previous,
            "currentSpeed": current,
            "previousSpeedFormatted": format_speed(previous),
            "currentSpeedFormatted": format_speed(current),
            "transition": transition,
        }));
        // Downgrade tem evento próprio: a tela o destaca de uma renegociação
        // qualquer, e o nome do evento é contrato do frontend.
        let kind = if transition == fields::interface_speed_transition::DOWNGRADE {
            "interface:speed_downgrade"
        } else {
            "interface:speed_change"
        };
        emit(ctx, &bus, kind, payload).await;
    }
}

async fn emit(ctx: &AppContext, bus: &EventBus, kind: &str, payload: Value) {
    if let Err(error) = bus.publish(&ctx.db, kind, payload).await {
        tracing::warn!(%error, event = kind, "falha ao publicar transição de interface");
    }
}
