//! O agente local (ADR 014): o Docker desta central, atendido por um agente
//! num container ao lado dela, pelo mesmo canal dos agentes remotos.
//!
//! O processo da API não monta o `docker.sock` — quem monta é o agente, sob a
//! política local dele (`AGENT_ALLOW`) e com a guarda dos containers
//! protegidos. Um comprometimento da API não vira root no host.
//!
//! **Zero-config.** No boot a central cadastra (ou reaproveita) o agente
//! local e grava o token dele em `LOCAL_AGENT_TOKEN_FILE`, num volume que só
//! os dois containers montam; o agente lê o mesmo arquivo (`AGENT_TOKEN_FILE`)
//! a cada tentativa de conexão. Não existe token padrão conhecido: sem o
//! volume, ninguém autentica como agente local.
//!
//! Sem a variável o agente local não existe — é o caso do desenvolvimento
//! fora do Docker — e a tela de Docker lista só os agentes remotos.

use std::path::{Path, PathBuf};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};

use crate::{
    models::{monitors, probes},
    services::{
        docker::hosts::HostKey,
        shared::{
            crypto::{random_token, sha256_hex},
            errors::{AppError, AppResult},
            secret_file,
        },
        telemetry,
    },
};

use super::service::{is_agent, ROLE_AGENT};

/// Nome com que o agente local nasce; o usuário pode renomear.
pub const LOCAL_AGENT_NAME: &str = "Este servidor";

/// Chave com que a central gravava o próprio histórico antes do agente local.
const LEGACY_HOST_KEY: &str = "local";

/// Onde a central grava o token do agente local.
#[must_use]
pub fn token_file() -> Option<PathBuf> {
    std::env::var("LOCAL_AGENT_TOKEN_FILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// O agente é o local desta central?
#[must_use]
pub fn is_local(probe: &probes::Model) -> bool {
    probe
        .configuration
        .as_ref()
        .and_then(|value| value.get("local"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// O agente local cadastrado, se houver.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn find<C: ConnectionTrait>(db: &C) -> AppResult<Option<probes::Model>> {
    Ok(probes::Entity::find()
        .filter(probes::Column::Status.ne(probes::STATUS_REVOKED))
        .order_by_asc(probes::Column::Id)
        .all(db)
        .await?
        .into_iter()
        .find(|probe| is_agent(probe) && is_local(probe)))
}

/// Recusa mexer no ciclo de vida do agente local: ele é da instalação, e o
/// próximo boot o recriaria de qualquer forma.
///
/// # Errors
///
/// `business_rule` quando o agente é o local.
pub fn ensure_managed_by_user(probe: &probes::Model, action: &str) -> AppResult<()> {
    if is_local(probe) {
        return Err(AppError::business_rule(format!(
            "O agente local é da instalação desta central e não pode ser {action}: ele sobe \
             com o docker-compose.yml, ao lado da API"
        )));
    }
    Ok(())
}

/// Cadastra ou reaproveita o agente local e deixa o token dele no arquivo.
///
/// Idempotente: um token já gravado que ainda autentica é mantido — o agente
/// conectado não cai a cada boot da API. Também adota o que era da central
/// antes do agente local: monitores de container sem probe e o histórico de
/// métricas gravado como `local`.
///
/// # Errors
///
/// Erro do banco ou falha ao gravar o arquivo do token.
pub async fn ensure<C: ConnectionTrait>(db: &C, token_path: &Path) -> AppResult<probes::Model> {
    let existing = find(db).await?;
    let saved_token = secret_file::read(token_path);
    let token_valid = |probe: &probes::Model| {
        saved_token
            .as_deref()
            .is_some_and(|token| sha256_hex(token) == probe.token_hash)
    };

    let probe = match existing {
        Some(probe) if token_valid(&probe) => probe,
        Some(probe) => {
            let token = random_token();
            write_token(token_path, &token)?;
            let mut active: probes::ActiveModel = probe.into();
            active.token_hash = Set(sha256_hex(&token));
            active.update(db).await?
        }
        None => {
            let token = random_token();
            write_token(token_path, &token)?;
            probes::ActiveModel {
                name: Set(LOCAL_AGENT_NAME.to_string()),
                token_hash: Set(sha256_hex(&token)),
                status: Set("pending".into()),
                registered_at: Set(Some(Utc::now().into())),
                configuration: Set(Some(json!({
                    "role": ROLE_AGENT,
                    "local": true,
                    "enforceTunnelIp": false,
                }))),
                ..Default::default()
            }
            .insert(db)
            .await?
        }
    };

    adopt_central_resources(db, &probe).await?;
    Ok(probe)
}

fn write_token(path: &Path, token: &str) -> AppResult<()> {
    secret_file::write(path, token).map_err(|error| {
        AppError::Internal(anyhow::anyhow!(
            "não foi possível gravar o token do agente local em {}: {error}",
            path.display()
        ))
    })
}

/// O Docker da central agora é o do agente local: os monitores de container
/// sem probe passam a ser dele, e o histórico antigo segue para a chave dele.
async fn adopt_central_resources<C: ConnectionTrait>(
    db: &C,
    probe: &probes::Model,
) -> AppResult<()> {
    monitors::Entity::update_many()
        .col_expr(
            monitors::Column::ProbeId,
            sea_orm::sea_query::Expr::value(probe.id),
        )
        .filter(monitors::Column::Type.eq("container"))
        .filter(monitors::Column::ProbeId.is_null())
        .exec(db)
        .await?;
    telemetry::store::rename_host(db, LEGACY_HOST_KEY, &HostKey::agent(probe.id).to_string())
        .await?;
    Ok(())
}
