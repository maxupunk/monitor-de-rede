//! O aviso de push do GitHub App (ADR 023, RC-01/02).
//!
//! A assinatura (`X-Hub-Signature-256`, HMAC-SHA256 com o segredo que o
//! GitHub gerou para o app) é conferida **antes** de ler o corpo — contra o
//! segredo de cada conexão desta central; o aviso que não bate com nenhuma é
//! descartado. Só compila push em branch ou tag que algum alvo ativo da
//! aplicação acompanha; branch apagada e qualquer outro evento são ignorados.

use hmac::{Hmac, Mac};
use loco_rs::app::AppContext;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use ts_rs::TS;

use netmonitor_core::shared::errors::{AppError, AppResult};

use super::{request, Ask, BuildTrigger};
use crate::deploys::{
    applications, connections,
    refs::{self, GitRef},
};

pub const SIGNATURE_HEADER: &str = "x-hub-signature-256";
pub const EVENT_HEADER: &str = "x-github-event";
pub const DELIVERY_HEADER: &str = "x-github-delivery";

/// O corpo bate com a assinatura?
#[must_use]
pub fn signed_by(secret: &str, body: &[u8], signature: &str) -> bool {
    let Some(hex) = signature.trim().strip_prefix("sha256=") else {
        return false;
    };
    let Ok(expected) = hex::decode(hex) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&expected).is_ok()
}

#[derive(Debug, Deserialize)]
struct Payload {
    #[serde(rename = "ref")]
    git_ref: String,
    after: String,
    #[serde(default)]
    deleted: bool,
    repository: Repository,
    #[serde(default)]
    head_commit: Option<Commit>,
    #[serde(default)]
    pusher: Option<Pusher>,
}

#[derive(Debug, Deserialize)]
struct Repository {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct Commit {
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Pusher {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployPushResponse"
)]
pub struct PushResponse {
    pub message: String,
    #[ts(type = "number[]")]
    pub builds: Vec<i64>,
}

fn ignored(message: impl Into<String>) -> PushResponse {
    PushResponse {
        message: message.into(),
        builds: Vec::new(),
    }
}

/// Trata um aviso do GitHub App.
///
/// # Errors
///
/// `Unauthorized` sem assinatura válida; `Validation` com corpo inválido.
pub async fn handle(
    ctx: &AppContext,
    event: &str,
    delivery: Option<&str>,
    signature: &str,
    body: &[u8],
) -> AppResult<PushResponse> {
    let mut connection = None;
    for (row, secret) in connections::webhook_secrets(&ctx.db).await? {
        if signed_by(&secret, body, signature) {
            connection = Some(row);
            break;
        }
    }
    let connection =
        connection.ok_or_else(|| AppError::unauthorized("Assinatura do GitHub inválida"))?;

    match event {
        "ping" => return Ok(ignored("pong")),
        "push" => {}
        other => return Ok(ignored(format!("Evento {other} ignorado"))),
    }
    let payload: Payload = serde_json::from_slice(body)
        .map_err(|error| AppError::validation(format!("Aviso de push inválido: {error}")))?;
    if payload.deleted || payload.after.chars().all(|c| c == '0') {
        return Ok(ignored("Branch ou tag apagada: nada a compilar"));
    }
    let Ok(reference) = GitRef::parse(&payload.git_ref) else {
        return Ok(ignored("Só branch ou tag compila"));
    };
    let commit = payload.after.to_lowercase();
    agent_protocol::docker::stacks::validate_commit(&commit)
        .map_err(|error| AppError::validation(error.to_string()))?;

    let mut created = Vec::new();
    for application in applications::by_repository(&ctx.db, &payload.repository.full_name).await? {
        if application.builder != "netmonitor"
            || application.github_connection_id != Some(connection.id)
        {
            continue;
        }
        let followed = applications::targets_of(&ctx.db, application.id)
            .await?
            .iter()
            .any(|target| target.enabled && refs::matches(&target.reference, &reference));
        if !followed {
            continue;
        }
        if let Some(build) = request(
            ctx,
            Ask {
                application_id: application.id,
                reference: Some(payload.git_ref.clone()),
                commit_sha: commit.clone(),
                commit_message: payload
                    .head_commit
                    .as_ref()
                    .and_then(|head| head.message.clone()),
                trigger: BuildTrigger::Push,
                triggered_by: payload
                    .pusher
                    .as_ref()
                    .and_then(|pusher| pusher.name.clone()),
                delivery_id: delivery.map(ToString::to_string),
                no_cache: false,
            },
        )
        .await?
        {
            created.push(build.id);
        }
    }
    Ok(PushResponse {
        message: if created.is_empty() {
            format!(
                "Nenhuma aplicação compila {} deste repositório",
                reference.name()
            )
        } else {
            format!("{} compilação(ões) na fila", created.len())
        },
        builds: created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assinatura_do_github() {
        // Exemplo da documentação do GitHub ("Validating webhook deliveries").
        let secret = "It's a Secret to Everybody";
        let body = b"Hello, World!";
        let signature = "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";
        assert!(signed_by(secret, body, signature));
        assert!(!signed_by("outro", body, signature));
        assert!(!signed_by(secret, b"Hello, World?", signature));
        assert!(!signed_by(secret, body, "sha1=abc"));
    }
}
