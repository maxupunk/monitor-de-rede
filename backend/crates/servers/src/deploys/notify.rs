//! Os avisos do deploy: canais de notificação e Web Push.
//!
//! Só o que pede atenção vira aviso: aprovação pendente, sucesso, falha e
//! rollback. O Web Push da aprovação leva o botão "Revisar e publicar", que
//! abre a tela de aprovação com os commits — a decisão nunca é tomada na
//! própria notificação.

use loco_rs::app::AppContext;
use serde_json::{json, Value};

use netmonitor_core::notifications::{NotificationMessage, Severity};

use super::builds::BuildView;
use super::short_sha;
use crate::{central::Central, views::deploys::DeployRunView};

/// O que aconteceu, para o texto do aviso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    AwaitingApproval,
    Succeeded,
    Failed,
    RolledBack,
}

/// A rota da SPA que mostra o deploy.
#[must_use]
pub fn run_url(run: &DeployRunView) -> String {
    format!("/deploys?run={}", run.id)
}

/// Título, corpo e gravidade do aviso — puro, testável.
#[must_use]
pub fn message(moment: Moment, run: &DeployRunView) -> NotificationMessage {
    let version = format!(
        "#{}{}",
        short_sha(&run.commit_sha),
        run.commit_message
            .as_deref()
            .and_then(|message| message.lines().next())
            .map(|line| format!(" — {line}"))
            .unwrap_or_default()
    );
    let place = format!("{} em {}", run.application_name, run.target_name);
    let (title, body, severity) = match moment {
        Moment::AwaitingApproval => (
            format!("Deploy aguarda aprovação: {place}"),
            format!("A versão {version} está pronta para publicar."),
            Severity::Info,
        ),
        Moment::Succeeded => (
            format!("Deploy concluído: {place}"),
            format!(
                "Versão {version} no ar{}.",
                run.duration_ms
                    .map(|ms| format!(" em {}s", ms / 1000))
                    .unwrap_or_default()
            ),
            Severity::Info,
        ),
        Moment::Failed => (
            format!("Falha no deploy: {place}"),
            format!(
                "A versão {version} não subiu: {}",
                run.error.as_deref().unwrap_or("erro desconhecido")
            ),
            Severity::Critical,
        ),
        Moment::RolledBack => (
            format!("Deploy revertido: {place}"),
            format!(
                "A versão {version} falhou e a anterior voltou ao ar. {}",
                run.error.as_deref().unwrap_or_default()
            ),
            Severity::Warning,
        ),
    };
    NotificationMessage {
        title,
        body,
        severity,
        metadata: json!({
            "kind": "deploy",
            "runId": run.id,
            "applicationId": run.application_id,
            "targetId": run.target_id,
            "url": run_url(run),
        }),
    }
}

/// O payload do Web Push, com o botão quando há decisão a tomar.
#[must_use]
pub fn push_payload(moment: Moment, run: &DeployRunView, message: &NotificationMessage) -> Value {
    let url = run_url(run);
    let actions = if moment == Moment::AwaitingApproval {
        json!([{ "action": "open", "title": "Revisar e publicar", "url": url }])
    } else {
        json!([{ "action": "open", "title": "Ver deploy", "url": url }])
    };
    json!({
        "title": message.title,
        "body": message.body,
        "icon": "/pwa-192x192.png",
        "badge": "/pwa-192x192.png",
        "tag": format!("deploy-{}", run.id),
        "data": { "url": url, "runId": run.id },
        "actions": actions,
    })
}

/// Avisa pelos canais e por Web Push. Nunca falha: aviso que não chegou
/// vira log.
pub async fn send(ctx: &AppContext, moment: Moment, run: &DeployRunView) {
    let Ok(central) = Central::from_context(ctx) else {
        tracing::debug!("deploy sem serviços da central: aviso não enviado");
        return;
    };
    let message = message(moment, run);
    central.notify(ctx, &message).await;
    if let Err(error) = central
        .push_to_servers(&ctx.db, &push_payload(moment, run, &message))
        .await
    {
        tracing::warn!(run_id = run.id, %error, "Web Push do deploy falhou");
    }
}

/// Uma compilação que falhou (ADR 023): sem aviso, o push que não compila
/// só seria visto por quem abrisse a tela.
#[must_use]
pub fn build_failed(build: &BuildView) -> NotificationMessage {
    let url = format!("/deploys?build={}", build.id);
    NotificationMessage {
        title: format!("Falha na compilação: {}", build.application),
        body: format!(
            "O commit {} não compilou: {}",
            build.short_sha,
            build.error.as_deref().unwrap_or("erro desconhecido")
        ),
        severity: Severity::Critical,
        metadata: json!({
            "kind": "deploy_build",
            "buildId": build.id,
            "applicationId": build.application_id,
            "url": url,
        }),
    }
}

/// Avisa a falha de uma compilação pelos canais e pelo Web Push.
pub async fn send_build_failed(ctx: &AppContext, build: &BuildView) {
    let Ok(central) = Central::from_context(ctx) else {
        return;
    };
    let message = build_failed(build);
    central.notify(ctx, &message).await;
    let url = format!("/deploys?build={}", build.id);
    let payload = json!({
        "title": message.title,
        "body": message.body,
        "icon": "/pwa-192x192.png",
        "badge": "/pwa-192x192.png",
        "tag": format!("deploy-build-{}", build.id),
        "data": { "url": url, "buildId": build.id },
        "actions": [{ "action": "open", "title": "Ver compilação", "url": url }],
    });
    if let Err(error) = central.push_to_servers(&ctx.db, &payload).await {
        tracing::warn!(build = build.id, %error, "Web Push da compilação falhou");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploys::{RunStatus, RunTrigger};

    fn run() -> DeployRunView {
        DeployRunView {
            id: 7,
            application_id: 1,
            application_name: "SGCv4".into(),
            repository: "a/b".into(),
            target_id: 2,
            target_name: "Produção".into(),
            trigger: RunTrigger::Webhook,
            triggered_by: Some("maxuel".into()),
            reference: Some("main".into()),
            commit_sha: "a1b2c3d4e5f6".into(),
            short_sha: "a1b2c3d".into(),
            commit_message: Some("feat: NF-e\n\ncorpo".into()),
            image_tag: "a1b2c3d4e5f6".into(),
            env_version_id: None,
            status: RunStatus::AwaitingApproval,
            stage: None,
            rollback_of_run_id: None,
            after_run_id: None,
            backup_id: None,
            approved_by: None,
            approved_at: None,
            scheduled_for: None,
            error: Some("healthcheck da API expirou".into()),
            created_at: String::new(),
            started_at: None,
            finished_at: None,
            duration_ms: Some(28_000),
            build_id: None,
        }
    }

    #[test]
    fn aprovacao_leva_o_botao_para_a_tela() {
        let run = run();
        let message = message(Moment::AwaitingApproval, &run);
        assert_eq!(message.title, "Deploy aguarda aprovação: SGCv4 em Produção");
        assert!(message.body.contains("#a1b2c3d — feat: NF-e"));
        assert!(!message.body.contains("corpo"));
        let payload = push_payload(Moment::AwaitingApproval, &run, &message);
        assert_eq!(payload["actions"][0]["title"], "Revisar e publicar");
        assert_eq!(payload["data"]["url"], "/deploys?run=7");
    }

    #[test]
    fn falha_e_critica_e_diz_o_motivo() {
        let message = message(Moment::Failed, &run());
        assert_eq!(message.severity, Severity::Critical);
        assert!(message.body.contains("healthcheck da API expirou"));
        let message = super::message(Moment::Succeeded, &run());
        assert!(message.body.contains("em 28s"));
    }
}
