//! A aprovação de cada acesso ao equipamento.
//!
//! O runtime pergunta ao [`AccessGate`] antes de **toda** chamada. Quem decide
//! a política é quem monta a execução (ver `runs`):
//!
//! | situação | gate |
//! |---|---|
//! | teste unitário (fixtures) | [`AllowAll`] |
//! | plugin ativo, ação disparada pelo usuário | [`AllowAll`] (o clique é a intenção; escrita já foi confirmada na tela) |
//! | plugin em rascunho/teste, ou IA sem modo automático | [`InteractiveGate`] — cada chamada pausa e espera "aprovar" |
//! | IA com "aceitar automaticamente" válido | [`AllowAll`], com tudo registrado |
//!
//! A pendência vai para a tela como snapshot do SSE global
//! (`plugin:approval_required`): uma aba aberta depois ainda a recebe, e nada
//! de polling (AGENTS §9). A resposta chega por `POST` e acorda a execução.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::Duration,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::{effect::Effect, manifest::TransportKind};
use crate::services::{
    events::EventBus,
    shared::errors::{AppError, AppResult},
};

/// Quanto uma chamada espera pela resposta do operador.
pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub const APPROVAL_REQUIRED_EVENT: &str = "plugin:approval_required";
pub const APPROVAL_RESOLVED_EVENT: &str = "plugin:approval_resolved";

/// O que o runtime quer fazer, do jeito que o operador lê.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct AccessRequest {
    pub transport: TransportKind,
    /// `opkg install luci-app-sqm`, `POST /cgi-bin/luci` — já sem segredos.
    pub summary: String,
    pub effect: Effect,
    /// Por que (declarado pela IA ou pela ação do plugin).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<String>,
}

#[async_trait]
pub trait AccessGate: Send + Sync {
    /// # Errors
    ///
    /// Acesso negado, expirado ou cancelado.
    async fn authorize(&self, request: &AccessRequest) -> AppResult<()>;
}

/// Libera tudo. O registro no transcript continua acontecendo.
pub struct AllowAll;

#[async_trait]
impl AccessGate for AllowAll {
    async fn authorize(&self, _request: &AccessRequest) -> AppResult<()> {
        Ok(())
    }
}

/// Pendências abertas, por chave (`<run>:<seq>`).
fn pending() -> &'static Mutex<HashMap<String, oneshot::Sender<bool>>> {
    static PENDING: OnceLock<Mutex<HashMap<String, oneshot::Sender<bool>>>> = OnceLock::new();
    PENDING.get_or_init(Mutex::default)
}

fn lock_pending() -> std::sync::MutexGuard<'static, HashMap<String, oneshot::Sender<bool>>> {
    pending()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Responde a uma pendência. `false` quando ela não existe mais (expirou,
/// foi respondida em outra aba ou a execução acabou).
#[must_use]
pub fn resolve(key: &str, approved: bool) -> bool {
    lock_pending()
        .remove(key)
        .is_some_and(|sender| sender.send(approved).is_ok())
}

/// Quem pergunta e sobre o quê, para a tela montar o diálogo.
#[derive(Debug, Clone)]
pub struct ApprovalContext {
    pub run_id: i64,
    pub device_id: i64,
    pub device_name: String,
    pub plugin_name: String,
    pub action: String,
}

/// Pausa cada chamada até o operador aprovar.
pub struct InteractiveGate {
    context: ApprovalContext,
    bus: Option<EventBus>,
    cancel: CancellationToken,
    sequence: AtomicU64,
    timeout: Duration,
}

impl InteractiveGate {
    #[must_use]
    pub fn new(context: ApprovalContext, bus: Option<EventBus>, cancel: CancellationToken) -> Self {
        Self {
            context,
            bus,
            cancel,
            sequence: AtomicU64::new(0),
            timeout: APPROVAL_TIMEOUT,
        }
    }

    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn announce(&self, key: &str, request: &AccessRequest) {
        if let Some(bus) = &self.bus {
            bus.publish_snapshot(
                APPROVAL_REQUIRED_EVENT,
                key,
                json!({
                    "key": key,
                    "runId": self.context.run_id,
                    "deviceId": self.context.device_id,
                    "deviceName": self.context.device_name,
                    "pluginName": self.context.plugin_name,
                    "action": self.context.action,
                    "request": request,
                    "expiresAt": (chrono::Utc::now()
                        + chrono::Duration::from_std(self.timeout).unwrap_or_default())
                    .to_rfc3339(),
                }),
            );
        }
    }

    fn settle(&self, key: &str, approved: bool) {
        lock_pending().remove(key);
        if let Some(bus) = &self.bus {
            bus.forget_snapshots(key);
            bus.publish_ephemeral(
                APPROVAL_RESOLVED_EVENT,
                json!({ "key": key, "runId": self.context.run_id, "approved": approved }),
            );
        }
    }
}

#[async_trait]
impl AccessGate for InteractiveGate {
    async fn authorize(&self, request: &AccessRequest) -> AppResult<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let key = format!("{}:{sequence}", self.context.run_id);
        let (sender, receiver) = oneshot::channel();
        lock_pending().insert(key.clone(), sender);
        self.announce(&key, request);

        let answer = tokio::select! {
            answer = receiver => answer.unwrap_or(false),
            () = self.cancel.cancelled() => false,
            () = tokio::time::sleep(self.timeout) => {
                self.settle(&key, false);
                return Err(AppError::business_rule(
                    "Ninguém aprovou o acesso a tempo; a execução foi interrompida.",
                ));
            }
        };
        self.settle(&key, answer);
        if answer {
            Ok(())
        } else {
            Err(AppError::forbidden(format!(
                "Acesso negado pelo operador: {}",
                request.summary
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AccessRequest {
        AccessRequest {
            transport: TransportKind::Ssh,
            summary: "uptime".into(),
            effect: Effect::Read,
            reason: Some("ver há quanto tempo está ligado".into()),
        }
    }

    fn gate(run_id: i64, cancel: CancellationToken) -> InteractiveGate {
        InteractiveGate::new(
            ApprovalContext {
                run_id,
                device_id: 1,
                device_name: "Roteador".into(),
                plugin_name: "Teste".into(),
                action: "detect".into(),
            },
            None,
            cancel,
        )
    }

    #[tokio::test]
    async fn aprovacao_libera_e_negacao_barra() {
        let gate = std::sync::Arc::new(gate(9_001, CancellationToken::new()));
        let aprovado = {
            let gate = gate.clone();
            tokio::spawn(async move { gate.authorize(&request()).await })
        };
        tokio::task::yield_now().await;
        while !resolve("9001:1", true) {
            tokio::task::yield_now().await;
        }
        assert!(aprovado.await.unwrap().is_ok());

        let negado = {
            let gate = gate.clone();
            tokio::spawn(async move { gate.authorize(&request()).await })
        };
        while !resolve("9001:2", false) {
            tokio::task::yield_now().await;
        }
        assert!(negado.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn sem_resposta_expira() {
        let gate = gate(9_002, CancellationToken::new()).with_timeout(Duration::from_millis(20));
        let error = gate.authorize(&request()).await.unwrap_err();
        assert!(error.to_string().contains("a tempo"));
        assert!(!resolve("9002:1", true), "a pendência expirada some");
    }

    #[tokio::test]
    async fn cancelar_nega() {
        let cancel = CancellationToken::new();
        let gate = gate(9_003, cancel.clone());
        cancel.cancel();
        assert!(gate.authorize(&request()).await.is_err());
    }
}
