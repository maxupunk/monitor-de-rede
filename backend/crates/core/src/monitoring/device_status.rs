//! Único ponto de escrita do status de dispositivos.

use chrono::{DateTime, Utc};
use loco_rs::app::AppContext;
use sea_orm::{sea_query::Expr, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::{
    events::EventBus,
    models::{devices, monitors},
    shared::errors::AppResult,
};
use agent_protocol::monitoring::MonitorStatus;

/// Tentativas do compare-and-set de [`apply`] antes de desistir da gravação.
const CAS_ATTEMPTS: usize = 3;

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
    let outcome = persist(&ctx.db, device, status, seen_at).await?;
    if outcome.changed {
        publish_transition(
            ctx,
            &outcome.current,
            outcome.previous_status,
            status,
            outcome.last_seen_at,
        )
        .await;
    }
    Ok(DeviceStatusTransition {
        changed: outcome.changed,
        previous_status: outcome.previous_status,
        status,
    })
}

/// O que [`persist`] gravou: a linha lida e se o status mudou por esta chamada.
struct PersistOutcome {
    current: devices::Model,
    previous_status: DeviceStatus,
    changed: bool,
    last_seen_at: Option<chrono::DateTime<chrono::FixedOffset>>,
}

/// Grava o status por compare-and-set.
///
/// O `UPDATE` só vale se o status no banco ainda for o lido. Duas checagens
/// concorrentes comparam contra o mesmo valor antigo, mas só uma afeta a linha
/// e publica `device:status` — nos dois dialetos, sem depender do isolamento
/// da transação (READ COMMITTED no PostgreSQL). Quem perde relê e tenta de novo.
async fn persist<C: ConnectionTrait>(
    db: &C,
    device: &devices::Model,
    status: DeviceStatus,
    seen_at: Option<DateTime<Utc>>,
) -> AppResult<PersistOutcome> {
    let mut outcome = PersistOutcome {
        current: device.clone(),
        previous_status: DeviceStatus::parse(&device.status),
        changed: false,
        last_seen_at: device.last_seen_at,
    };
    for _ in 0..CAS_ATTEMPTS {
        let Some(current) = devices::Entity::find_by_id(device.id).one(db).await? else {
            break;
        };
        outcome.previous_status = DeviceStatus::parse(&current.status);
        outcome.last_seen_at = seen_at.map(Into::into).or(current.last_seen_at);
        outcome.current = current;
        let changed = outcome.previous_status != status;
        if !changed && seen_at.is_none() {
            break;
        }
        if try_swap(db, &outcome.current, status, seen_at).await? {
            outcome.changed = changed;
            break;
        }
    }
    Ok(outcome)
}

/// Uma tentativa do compare-and-set: `true` se a linha ainda tinha o status
/// de `current` e foi gravada.
async fn try_swap<C: ConnectionTrait>(
    db: &C,
    current: &devices::Model,
    status: DeviceStatus,
    seen_at: Option<DateTime<Utc>>,
) -> AppResult<bool> {
    let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
    let mut update = devices::Entity::update_many()
        .col_expr(devices::Column::Status, Expr::value(status.as_str()))
        .col_expr(devices::Column::UpdatedAt, Expr::value(now))
        .filter(devices::Column::Id.eq(current.id))
        .filter(devices::Column::Status.eq(current.status.as_str()));
    if let Some(seen_at) = seen_at {
        let seen_at: chrono::DateTime<chrono::FixedOffset> = seen_at.into();
        update = update.col_expr(devices::Column::LastSeenAt, Expr::value(seen_at));
    }
    Ok(update.exec(db).await?.rows_affected == 1)
}

/// Publica a transição no contrato de eventos em tempo real.
///
/// Best-effort pelo mesmo motivo do `result_processor`: o status já está
/// gravado, e uma falha de barramento não pode desfazer isso nem propagar erro
/// para quem só pediu a atualização.
async fn publish_transition(
    ctx: &AppContext,
    device: &devices::Model,
    previous_status: DeviceStatus,
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
        "previousStatus": previous_status.as_str(),
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
    use crate::migrations::{CoreMigrator as Migrator, MigratorTrait};
    use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

    async fn banco() -> DatabaseConnection {
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    async fn dispositivo(db: &DatabaseConnection, status: &str) -> devices::Model {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
        devices::ActiveModel {
            name: Set("roteador".into()),
            r#type: Set("router".into()),
            is_monitored: Set(true),
            snmp_enabled: Set(false),
            snmp_poll_interval_seconds: Set(60),
            status: Set(status.into()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("dispositivo")
    }

    #[tokio::test]
    async fn so_a_primeira_gravacao_da_transicao_conta_como_mudanca() {
        let db = banco().await;
        let device = dispositivo(&db, "online").await;

        let primeira = persist(&db, &device, DeviceStatus::Offline, None)
            .await
            .unwrap();
        assert!(primeira.changed);
        assert_eq!(primeira.previous_status, DeviceStatus::Online);

        // A segunda checagem chega com o modelo antigo em mãos: relê e não
        // publica de novo.
        let segunda = persist(&db, &device, DeviceStatus::Offline, Some(Utc::now()))
            .await
            .unwrap();
        assert!(!segunda.changed);
        // `last_seen_at` avança mesmo sem transição.
        let gravado = devices::Entity::find_by_id(device.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(gravado.last_seen_at.is_some());
    }

    #[tokio::test]
    async fn troca_com_status_obsoleto_nao_afeta_a_linha() {
        let db = banco().await;
        let obsoleto = dispositivo(&db, "online").await;
        assert!(try_swap(&db, &obsoleto, DeviceStatus::Offline, None)
            .await
            .unwrap());
        // Outra checagem leu "online" antes da gravação acima: o UPDATE dela
        // não casa mais e ela não publica a transição duplicada.
        assert!(!try_swap(&db, &obsoleto, DeviceStatus::Offline, None)
            .await
            .unwrap());
        let gravado = devices::Entity::find_by_id(obsoleto.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(gravado.status, "offline");
    }

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
