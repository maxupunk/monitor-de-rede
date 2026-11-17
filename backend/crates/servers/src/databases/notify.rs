//! Avisos de falha no backup ou na restauração de bancos de dados: canais de
//! notificação (Telegram, Discord, e-mail, webhook), Web Push para quem tem
//! acesso à área Servidores e barramento SSE (`backup:failed`).

use loco_rs::app::AppContext;
use serde_json::json;

use netmonitor_core::{
    events::EventBus,
    notifications::{NotificationMessage, Severity},
};

use crate::central::Central;

const BACKUP_URL: &str = "/backup";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Backup,
    Restore,
}

impl FailureKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Backup => "database_backup",
            Self::Restore => "database_restore",
        }
    }
}

fn trigger_label(trigger: Option<&str>) -> &'static str {
    match trigger {
        Some("scheduled") => "automático",
        Some("pre_deploy") => "pré-deploy",
        _ => "manual",
    }
}

/// Título, corpo e metadados da falha — função pura e testável.
#[must_use]
pub fn failure_message(
    kind: FailureKind,
    connection_name: &str,
    connection_id: i64,
    destination_name: Option<&str>,
    trigger: Option<&str>,
    error: &str,
) -> NotificationMessage {
    let destination = destination_name
        .filter(|name| !name.trim().is_empty())
        .map(|name| format!(" (destino \"{name}\")"))
        .unwrap_or_default();
    let (title, body) = match kind {
        FailureKind::Backup => (
            format!("Falha no backup de banco: {connection_name}"),
            format!(
                "O backup {} de \"{connection_name}\"{destination} falhou: {error}",
                trigger_label(trigger)
            ),
        ),
        FailureKind::Restore => (
            format!("Falha na restauração de banco: {connection_name}"),
            format!("A restauração em \"{connection_name}\"{destination} falhou: {error}"),
        ),
    };
    NotificationMessage {
        title,
        body,
        severity: Severity::Critical,
        metadata: json!({
            "kind": kind.as_str(),
            "area": "servers",
            "connectionId": connection_id,
            "destinationName": destination_name,
            "trigger": trigger,
            "url": BACKUP_URL,
            "tag": format!("{}-{connection_id}", kind.as_str()),
            "actions": [{ "action": "open", "title": "Abrir Backup", "url": BACKUP_URL }],
        }),
    }
}

/// Publica `backup:failed` no SSE e envia pelos canais da central + Web Push.
pub async fn send_failure(
    ctx: &AppContext,
    kind: FailureKind,
    connection_name: &str,
    connection_id: i64,
    destination_name: Option<&str>,
    trigger: Option<&str>,
    error: &str,
) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        let payload = json!({
            "kind": kind.as_str(),
            "connectionId": connection_id,
            "name": connection_name,
            "destinationName": destination_name,
            "trigger": trigger,
            "message": error,
            "url": BACKUP_URL,
        });
        if let Err(err) = bus.publish(&ctx.db, "backup:failed", payload).await {
            tracing::warn!(connection_id, %err, "falha ao publicar backup:failed");
        }
    }
    let Ok(central) = Central::from_context(ctx) else {
        tracing::debug!("serviços da central indisponíveis: aviso de backup não enviado");
        return;
    };
    let message = failure_message(
        kind,
        connection_name,
        connection_id,
        destination_name,
        trigger,
        error,
    );
    central.notify(ctx, &message).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falha_de_backup_e_critica_e_aponta_para_a_tela_de_backup() {
        let msg = failure_message(
            FailureKind::Backup,
            "ERP Produção",
            12,
            Some("Bucket S3"),
            Some("scheduled"),
            "autenticação recusada",
        );
        assert_eq!(msg.severity, Severity::Critical);
        assert_eq!(msg.title, "Falha no backup de banco: ERP Produção");
        assert!(msg.body.contains("automático"));
        assert!(msg.body.contains("Bucket S3"));
        assert!(msg.body.contains("autenticação recusada"));
        assert_eq!(msg.metadata["area"], "servers");
        assert_eq!(msg.metadata["url"], "/backup");
        assert_eq!(msg.metadata["actions"][0]["title"], "Abrir Backup");
    }

    #[test]
    fn falha_de_restauracao_identifica_a_conexao() {
        let msg = failure_message(
            FailureKind::Restore,
            "Loja",
            5,
            None,
            Some("manual"),
            "soma de verificação não confere",
        );
        assert_eq!(msg.title, "Falha na restauração de banco: Loja");
        assert!(msg.body.contains("soma de verificação"));
        assert_eq!(msg.metadata["kind"], "database_restore");
    }
}
