//! Avisos de falha no backup do próprio NetMonitor: canais externos, Web Push
//! e barramento SSE (`backup:failed`).

use loco_rs::app::AppContext;
use serde_json::json;

use crate::services::{
    events::EventBus,
    notifications::{
        contracts::{NotificationMessage, Severity},
        service::NotificationService,
    },
};

const BACKUP_URL: &str = "/backup";

/// Monta a mensagem de falha do backup da configuração do NetMonitor.
#[must_use]
pub fn system_backup_failed_message(
    destination_name: Option<&str>,
    trigger: &str,
    error: &str,
) -> NotificationMessage {
    let how = if trigger == "scheduled" {
        "automático"
    } else {
        "manual"
    };
    let where_label = destination_name
        .filter(|name| !name.trim().is_empty())
        .map(|name| format!(" no destino \"{name}\""))
        .unwrap_or_default();
    NotificationMessage {
        title: "Falha no backup do NetMonitor".to_string(),
        body: format!("O backup {how}{where_label} falhou: {error}"),
        severity: Severity::Critical,
        metadata: json!({
            "kind": "system_backup",
            "trigger": trigger,
            "destinationName": destination_name,
            "url": BACKUP_URL,
            "tag": "backup-system-failed",
            "actions": [{ "action": "open", "title": "Abrir Backup", "url": BACKUP_URL }],
        }),
    }
}

/// Publica `backup:failed` no SSE e envia pelos canais de notificação + Web Push.
pub async fn notify_system_backup_failed(
    ctx: &AppContext,
    destination_name: Option<&str>,
    trigger: &str,
    error: &str,
) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        let payload = json!({
            "kind": "system_backup",
            "name": "NetMonitor",
            "destinationName": destination_name,
            "trigger": trigger,
            "message": error,
            "url": BACKUP_URL,
        });
        if let Err(err) = bus.publish(&ctx.db, "backup:failed", payload).await {
            tracing::warn!(%err, "falha ao publicar backup:failed do sistema");
        }
    }
    let message = system_backup_failed_message(destination_name, trigger, error);
    NotificationService::with_default_channels()
        .notify(ctx, &message)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mensagem_de_falha_traz_gatilho_destino_e_atalho() {
        let msg = system_backup_failed_message(Some("NAS Principal"), "scheduled", "disco cheio");
        assert_eq!(msg.severity, Severity::Critical);
        assert_eq!(msg.title, "Falha no backup do NetMonitor");
        assert!(msg.body.contains("automático"));
        assert!(msg.body.contains("NAS Principal"));
        assert!(msg.body.contains("disco cheio"));
        assert_eq!(msg.metadata["url"], "/backup");
        assert_eq!(msg.metadata["actions"][0]["title"], "Abrir Backup");
    }
}
