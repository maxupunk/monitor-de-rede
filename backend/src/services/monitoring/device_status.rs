//! Único ponto de escrita do status de dispositivos.

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, Set};

use crate::{
    models::{devices, monitors},
    services::{events::EventBus, monitoring::contracts::MonitorStatus, shared::errors::AppResult},
};

/// Estado agregado que o frontend usa para um equipamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    Online,
    Offline,
    Warning,
    Unknown,
}

impl DeviceStatus {
    /// Lê um valor persistido sem dar peso a lixo histórico desconhecido.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "online" => Self::Online,
            "offline" => Self::Offline,
            "warning" => Self::Warning,
            _ => Self::Unknown,
        }
    }

    /// Forma estável persistida e entregue ao frontend.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Offline => "offline",
            Self::Warning => "warning",
            Self::Unknown => "unknown",
        }
    }
}

/// Resultado de uma atualização agregada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceStatusTransition {
    pub changed: bool,
    pub previous_status: DeviceStatus,
    pub status: DeviceStatus,
}

/// Agrega todos os monitores habilitados e preserva o fallback sem observação.
#[must_use]
pub fn aggregate(statuses: &[MonitorStatus], fallback: DeviceStatus) -> DeviceStatus {
    let conclusive: Vec<_> = statuses
        .iter()
        .copied()
        .filter(|status| !matches!(status, MonitorStatus::Unknown | MonitorStatus::Disabled))
        .collect();
    if conclusive.is_empty() {
        return fallback;
    }
    let has_up = conclusive.contains(&MonitorStatus::Up);
    let has_down = conclusive.contains(&MonitorStatus::Down);
    if has_up && has_down {
        DeviceStatus::Warning
    } else if has_down {
        DeviceStatus::Offline
    } else if conclusive.contains(&MonitorStatus::Warning) {
        DeviceStatus::Warning
    } else {
        DeviceStatus::Online
    }
}

/// Recalcula o estado de um dispositivo a partir de seus monitores habilitados.
pub async fn refresh_from_monitors(
    ctx: &AppContext,
    device: &devices::Model,
    observed_status: Option<DeviceStatus>,
    seen_at: Option<DateTime<Utc>>,
) -> AppResult<DeviceStatusTransition> {
    let rows = monitors::Entity::find_enabled_for_device(device.id)
        .all(&ctx.db)
        .await?;
    let statuses: Vec<_> = rows
        .iter()
        .map(|monitor| match monitor.status.as_str() {
            "up" => MonitorStatus::Up,
            "down" => MonitorStatus::Down,
            "warning" => MonitorStatus::Warning,
            "disabled" => MonitorStatus::Disabled,
            _ => MonitorStatus::Unknown,
        })
        .collect();
    let fallback = DeviceStatus::parse(&device.status);
    let next = if statuses.is_empty() {
        observed_status.unwrap_or(fallback)
    } else {
        aggregate(&statuses, fallback)
    };
    apply(ctx, device, next, seen_at).await
}

/// Persiste o estado decidido e publica `device:status` **apenas na transição**.
///
/// `last_seen_at` é telemetria — o dispositivo continua sendo visto a cada ciclo
/// — e por isso avança sem gerar evento. Emitir a cada gravação republicaria
/// "offline ➔ offline" a cada varredura, que é justamente o ruído que
/// concentrar a decisão aqui elimina.
pub async fn apply(
    ctx: &AppContext,
    device: &devices::Model,
    status: DeviceStatus,
    seen_at: Option<DateTime<Utc>>,
) -> AppResult<DeviceStatusTransition> {
    let previous_status = DeviceStatus::parse(&device.status);
    let changed = previous_status != status;
    let mut last_seen_at = device.last_seen_at;
    if changed || seen_at.is_some() {
        let mut active: devices::ActiveModel = device.clone().into();
        active.status = Set(status.as_str().to_string());
        if let Some(seen_at) = seen_at {
            last_seen_at = Some(seen_at.into());
            active.last_seen_at = Set(last_seen_at);
        }
        active.update(&ctx.db).await?;
    }
    if changed {
        publish_transition(ctx, device, status, last_seen_at).await;
    }
    Ok(DeviceStatusTransition {
        changed,
        previous_status,
        status,
    })
}

/// Publica a transição no contrato de eventos em tempo real.
///
/// Best-effort pelo mesmo motivo do `result_processor`: o status já está
/// gravado, e uma falha de barramento não pode desfazer isso nem propagar erro
/// para quem só pediu a atualização.
async fn publish_transition(
    ctx: &AppContext,
    device: &devices::Model,
    status: DeviceStatus,
    last_seen_at: Option<chrono::DateTime<chrono::FixedOffset>>,
) {
    let Ok(events) = EventBus::from_context(ctx) else {
        return;
    };
    // `id` e `deviceId` carregam o mesmo valor: `stores/devices.ts` lê
    // `data.id ?? data.deviceId` e o feed de eventos lê `d.id`.
    let payload = serde_json::json!({
        "id": device.id,
        "deviceId": device.id,
        "name": device.name,
        "status": status.as_str(),
        "previousStatus": DeviceStatus::parse(&device.status).as_str(),
        "ipAddress": device.ip_address,
        "lastSeenAt": last_seen_at.map(|value| value.to_rfc3339()),
        "changedAt": Utc::now().to_rfc3339(),
    });
    if let Err(error) = events.publish(&ctx.db, "device:status", payload).await {
        tracing::warn!(%error, device_id = device.id, "falha ao publicar device:status");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inconclusivos_nao_trocam_o_status() {
        assert_eq!(
            aggregate(
                &[MonitorStatus::Unknown, MonitorStatus::Disabled],
                DeviceStatus::Online
            ),
            DeviceStatus::Online
        );
        assert_eq!(
            aggregate(
                &[MonitorStatus::Up, MonitorStatus::Down],
                DeviceStatus::Online
            ),
            DeviceStatus::Warning
        );
        assert_eq!(
            aggregate(&[MonitorStatus::Down], DeviceStatus::Online),
            DeviceStatus::Offline
        );
    }
}
