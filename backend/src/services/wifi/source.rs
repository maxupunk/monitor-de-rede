//! De onde vem a telemetria de cada equipamento: o plugin ativo e instalado
//! que declara o bloco `wifi` no manifesto. Fabricante novo é plugin novo —
//! nada aqui conhece slug.

use std::collections::BTreeMap;

use loco_rs::app::AppContext;
use sea_orm::ConnectionTrait;
use serde_json::Value;

use crate::services::{
    plugins::{
        invoke::{self, RunStatus, ServiceRunRequest},
        manifest::WifiRoles,
        runs::Origin,
        service,
    },
    shared::errors::{AppError, AppResult},
};

/// Quem pede a leitura: o operador, pela tela, ou o vigia dos clientes
/// vigiados. Vai para o histórico do plugin e para a auditoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requester {
    pub user_id: Option<i64>,
    pub origin: Origin,
}

impl Requester {
    #[must_use]
    pub const fn user(user_id: Option<i64>) -> Self {
        Self {
            user_id,
            origin: Origin::User,
        }
    }

    /// A leitura periódica, sem operador por trás.
    #[must_use]
    pub const fn watch() -> Self {
        Self {
            user_id: None,
            origin: Origin::WifiWatch,
        }
    }
}

/// O plugin Wi-Fi de um equipamento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiSource {
    pub slug: String,
    pub roles: WifiRoles,
}

/// O plugin Wi-Fi do equipamento, se houver.
///
/// # Errors
///
/// Erro do banco ou manifesto gravado fora do formato.
pub async fn for_device<C: ConnectionTrait>(
    db: &C,
    device_id: i64,
) -> AppResult<Option<WifiSource>> {
    Ok(by_device(db, Some(&[device_id])).await?.remove(&device_id))
}

/// Todos os equipamentos com plugin Wi-Fi, pelo id.
///
/// # Errors
///
/// Erro do banco ou manifesto gravado fora do formato.
pub async fn all<C: ConnectionTrait>(db: &C) -> AppResult<BTreeMap<i64, WifiSource>> {
    by_device(db, None).await
}

/// Um plugin por equipamento — o exclusivo do equipamento vale mais que o de
/// modelo, como no teste de enlace.
async fn by_device<C: ConnectionTrait>(
    db: &C,
    device_ids: Option<&[i64]>,
) -> AppResult<BTreeMap<i64, WifiSource>> {
    let mut found: BTreeMap<i64, (bool, WifiSource)> = BTreeMap::new();
    for (device_id, plugin) in service::installed_active(db, device_ids).await? {
        let Some(roles) = service::package_of(&plugin)?.manifest.wifi else {
            continue;
        };
        let exclusive = plugin.device_id.is_some();
        let keep = found
            .get(&device_id)
            .is_none_or(|(current_exclusive, _)| exclusive && !current_exclusive);
        if keep {
            let source = WifiSource {
                slug: plugin.slug,
                roles,
            };
            found.insert(device_id, (exclusive, source));
        }
    }
    Ok(found
        .into_iter()
        .map(|(device_id, (_, source))| (device_id, source))
        .collect())
}

impl WifiSource {
    /// Roda um papel no equipamento — a mesma execução da tela (auditada, no
    /// histórico do plugin) — e devolve a saída.
    ///
    /// # Errors
    ///
    /// O que a execução recusar (credencial, agente fora) ou a falha da ação
    /// no equipamento, com a mensagem dela.
    pub async fn run(
        &self,
        ctx: &AppContext,
        device_id: i64,
        action: &str,
        params: Value,
        requester: Requester,
        reason: String,
    ) -> AppResult<Value> {
        let outcome = invoke::execute(
            ctx,
            ServiceRunRequest {
                device_id,
                plugin_slug: self.slug.clone(),
                action: action.to_owned(),
                params,
                origin: requester.origin,
                user_id: requester.user_id,
                reason: Some(reason),
                progress: None,
                cancel: None,
            },
        )
        .await?;
        match outcome.status {
            RunStatus::Succeeded => Ok(outcome.output.unwrap_or(Value::Null)),
            RunStatus::Cancelled => Err(AppError::service_unavailable("A leitura foi cancelada.")),
            RunStatus::Failed => {
                Err(AppError::service_unavailable(outcome.error.unwrap_or_else(
                    || "O equipamento não respondeu à leitura.".into(),
                )))
            }
        }
    }
}
