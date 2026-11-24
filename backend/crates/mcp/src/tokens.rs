//! Ciclo de vida de tokens pessoais do servidor MCP (`nmm_*`).
//!
//! O segredo bruto (`nmm_` + 32 bytes aleatórios em hex) é exibido uma única
//! vez na criação; o banco armazena somente `sha256_hex` e os últimos 4
//! caracteres (`hint`). Todas as escritas usam `shared::transaction::begin_write`.

use std::collections::{BTreeSet, HashMap};

use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use netmonitor_core::{
    models::{_entities::users as users_entity, mcp_tokens, users},
    roles::AreaAccess,
    shared::{
        errors::{AppError, AppResult},
        personal_token,
        transaction::begin_write,
    },
};

use crate::{
    dtos::{CreateMcpTokenInput, McpTokenView},
    scopes::McpScope,
};

pub const PREFIX: &str = "nmm_";
pub const DEFAULT_TTL_DAYS: i64 = 90;
pub const MAX_TTL_DAYS: i64 = 366;

/// Formata a identificação do dono exibida na listagem de tokens.
#[must_use]
pub fn format_owner(user: &users::Model) -> String {
    let name = user.name.trim();
    if name.is_empty() {
        user.email.clone()
    } else {
        format!("{name} ({})", user.email)
    }
}

/// Lê os escopos gravados na coluna JSON do token.
#[must_use]
pub fn scopes_of(token: &mcp_tokens::Model) -> Vec<McpScope> {
    token
        .scopes
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter_map(McpScope::parse)
        .collect()
}

/// Lê os IDs de aplicações de deploy permitidos (`None` = todas).
#[must_use]
pub fn allowed_application_ids_of(token: &mcp_tokens::Model) -> Option<Vec<i64>> {
    token.allowed_application_ids.as_ref().and_then(|value| {
        value.as_array().map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_i64)
                .filter(|id| *id > 0)
                .collect()
        })
    })
}

/// Cria um novo token MCP validando escopos contra o `AreaAccess` ao vivo do
/// chamador (sem escalação de privilégio).
///
/// # Errors
///
/// `Validation` para nome/TTL/escopo inválido; `Forbidden` se algum escopo
/// solicitado exceder o papel do chamador.
pub async fn create(
    db: &DatabaseConnection,
    user: &users::Model,
    access: &AreaAccess,
    input: &CreateMcpTokenInput,
) -> AppResult<(mcp_tokens::Model, String)> {
    let name = personal_token::validate_name(&input.name)?;
    let ttl_days = input.ttl_days.unwrap_or(DEFAULT_TTL_DAYS);
    personal_token::validate_ttl(ttl_days, MAX_TTL_DAYS)?;

    if input.scopes.is_empty() {
        return Err(AppError::validation(
            "Selecione ao menos um escopo para o token MCP.",
        ));
    }

    let mut unique_scopes = BTreeSet::new();
    for raw in &input.scopes {
        let scope = McpScope::parse(raw)
            .ok_or_else(|| AppError::validation(format!("Escopo MCP desconhecido: '{raw}'.")))?;
        if !scope.granted_by_access(access) {
            return Err(AppError::forbidden(format!(
                "Seu perfil não permite criar token com o escopo '{}'.",
                scope.id()
            )));
        }
        unique_scopes.insert(scope);
    }

    let ordered_scopes: Vec<&str> = McpScope::ALL
        .into_iter()
        .filter(|scope| unique_scopes.contains(scope))
        .map(McpScope::id)
        .collect();

    let allowed_apps_json = match &input.allowed_application_ids {
        Some(ids) => {
            let mut normalized: Vec<i64> = ids.iter().copied().filter(|id| *id > 0).collect();
            normalized.sort_unstable();
            normalized.dedup();
            if normalized.is_empty() {
                return Err(AppError::validation(
                    "Informe ao menos uma aplicação válida ou deixe sem restrição.",
                ));
            }
            Some(
                serde_json::to_value(normalized)
                    .map_err(|error| AppError::Internal(error.into()))?,
            )
        }
        None => None,
    };

    let issued = personal_token::issue(PREFIX);
    let now = Utc::now();

    let tx = begin_write(db).await?;
    let row = mcp_tokens::ActiveModel {
        user_id: Set(user.id),
        name: Set(name.to_owned()),
        token_hash: Set(issued.hash),
        hint: Set(issued.hint),
        scopes: Set(serde_json::to_value(&ordered_scopes)
            .map_err(|error| AppError::Internal(error.into()))?),
        allowed_application_ids: Set(allowed_apps_json),
        expires_at: Set((now + Duration::days(ttl_days)).into()),
        last_used_at: Set(None),
        last_used_ip: Set(None),
        revoked_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(&tx)
    .await?;
    tx.commit().await?;

    Ok((row, issued.secret))
}

/// Lista tokens (todos para admin; apenas os do usuário quando `user_id` é `Some`).
///
/// # Errors
///
/// Propaga falhas do banco.
pub async fn list(db: &DatabaseConnection, user_id: Option<i64>) -> AppResult<Vec<McpTokenView>> {
    let mut query = mcp_tokens::Entity::find().order_by_desc(mcp_tokens::Column::Id);
    if let Some(uid) = user_id {
        query = query.filter(mcp_tokens::Column::UserId.eq(uid));
    }
    let rows = query.all(db).await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let owner_ids: Vec<i64> = rows
        .iter()
        .map(|row| row.user_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let owners: HashMap<i64, String> = users_entity::Entity::find()
        .filter(users_entity::Column::Id.is_in(owner_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|user| {
            let label = format_owner(&user);
            (user.id, label)
        })
        .collect();

    Ok(rows
        .iter()
        .map(|row| {
            let owner = owners.get(&row.user_id).cloned().unwrap_or_default();
            McpTokenView::from_model(row, owner)
        })
        .collect())
}

/// Revoga um token. Só o dono ou um administrador (`owner = None`).
///
/// # Errors
///
/// `NotFound` quando o token não existe ou não pertence ao chamador.
pub async fn revoke(
    db: &DatabaseConnection,
    id: i64,
    owner: Option<i64>,
) -> AppResult<mcp_tokens::Model> {
    let tx = begin_write(db).await?;
    let token = mcp_tokens::Entity::find_by_id(id)
        .one(&tx)
        .await?
        .filter(|token| owner.is_none_or(|uid| uid == token.user_id))
        .ok_or_else(|| AppError::not_found("Token MCP não encontrado."))?;
    // Revogar de novo não reescreve quando foi revogado.
    if token.revoked_at.is_some() {
        return Ok(token);
    }
    let mut active: mcp_tokens::ActiveModel = token.into();
    active.revoked_at = Set(Some(Utc::now().into()));
    let updated = active.update(&tx).await?;
    tx.commit().await?;
    Ok(updated)
}

/// De quanto em quanto tempo o "último uso" é regravado: um agente faz várias
/// chamadas por minuto, e cada uma seria uma escrita (no SQLite, disputando a
/// trava com o resto da central).
const USAGE_STAMP_INTERVAL_SECS: i64 = 60;

/// O "último uso" precisa ser regravado? Sim na primeira vez, quando mudou o
/// IP ou quando o registro tem mais de [`USAGE_STAMP_INTERVAL_SECS`].
fn needs_usage_stamp(
    last_used_at: Option<chrono::DateTime<Utc>>,
    last_ip: Option<&str>,
    ip: Option<&str>,
    now: chrono::DateTime<Utc>,
) -> bool {
    match last_used_at {
        None => true,
        Some(at) => last_ip != ip || (now - at).num_seconds() >= USAGE_STAMP_INTERVAL_SECS,
    }
}

/// Autentica um token bruto, valida revogação/expiração/usuário ativo e
/// atualiza `last_used_at` e `last_used_ip` via `begin_write` (no máximo uma
/// vez por minuto por token, ou quando o IP muda).
///
/// # Errors
///
/// Propaga falhas do banco.
pub async fn authenticate(
    db: &DatabaseConnection,
    secret: &str,
    ip: Option<String>,
) -> AppResult<Option<(mcp_tokens::Model, users::Model)>> {
    let Some(hash) = personal_token::lookup_hash(PREFIX, secret) else {
        return Ok(None);
    };
    let Some(token) = mcp_tokens::Entity::find()
        .filter(mcp_tokens::Column::TokenHash.eq(hash))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    if !personal_token::is_usable(token.revoked_at, token.expires_at, Utc::now()) {
        return Ok(None);
    }
    let Some(user) = users::Entity::find_by_id(token.user_id).one(db).await? else {
        return Ok(None);
    };
    if !user.active {
        return Ok(None);
    }

    let now = Utc::now();
    if !needs_usage_stamp(
        token.last_used_at.map(|at| at.with_timezone(&Utc)),
        token.last_used_ip.as_deref(),
        ip.as_deref(),
        now,
    ) {
        return Ok(Some((token, user)));
    }
    let tx = begin_write(db).await?;
    let mut active: mcp_tokens::ActiveModel = token.into();
    active.last_used_at = Set(Some(now.into()));
    active.last_used_ip = Set(ip);
    let updated = active.update(&tx).await?;
    tx.commit().await?;

    Ok(Some((updated, user)))
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectOptions, Database};
    use uuid::Uuid;

    use super::*;
    use crate::migrations::{MigratorTrait, TestMigrator};

    #[test]
    fn ultimo_uso_regravado_no_maximo_por_minuto() {
        let now = Utc::now();
        let ip = Some("10.8.0.2");
        assert!(needs_usage_stamp(None, None, ip, now));
        let recent = Some(now - chrono::Duration::seconds(10));
        assert!(!needs_usage_stamp(recent, ip, ip, now));
        assert!(needs_usage_stamp(recent, ip, Some("10.8.0.3"), now));
        let old = Some(now - chrono::Duration::seconds(USAGE_STAMP_INTERVAL_SECS));
        assert!(needs_usage_stamp(old, ip, ip, now));
    }

    async fn memory_db() -> DatabaseConnection {
        let db = Database::connect(
            ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco");
        TestMigrator::up(&db, None).await.expect("migrations");
        db
    }

    async fn insert_user(db: &DatabaseConnection, role: &str, active: bool) -> users::Model {
        let pid = Uuid::new_v4();
        users::ActiveModel {
            pid: Set(pid),
            email: Set(format!("{role}-{pid}@test.local")),
            password: Set("hash".into()),
            api_key: Set(format!("key-{pid}")),
            name: Set(format!("User {role}")),
            active: Set(active),
            role: Set(role.into()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("usuário")
    }

    #[tokio::test]
    async fn ciclo_de_vida_e_rejeicao_de_escalacao() {
        let db = memory_db().await;
        let admin = insert_user(&db, "admin", true).await;
        let viewer = insert_user(&db, "viewer", true).await;
        let admin_access = AreaAccess::of_user(&admin).expect("access");
        let viewer_access = AreaAccess::of_user(&viewer).expect("access");

        // Viewer não pode criar token com deploys:write
        let escalation = create(
            &db,
            &viewer,
            &viewer_access,
            &CreateMcpTokenInput {
                name: "escalacao".into(),
                scopes: vec!["deploys:write".into()],
                allowed_application_ids: None,
                ttl_days: Some(30),
            },
        )
        .await;
        assert!(matches!(escalation, Err(AppError::Forbidden(_))));

        // Admin cria token restrito a apps [2, 5]
        let (row, secret) = create(
            &db,
            &admin,
            &admin_access,
            &CreateMcpTokenInput {
                name: "antigravity".into(),
                scopes: vec!["deploys:read".into(), "deploys:write".into()],
                allowed_application_ids: Some(vec![5, 2, 5]),
                ttl_days: None,
            },
        )
        .await
        .expect("criar token");
        assert!(secret.starts_with(PREFIX));
        assert_eq!(row.hint.len(), 4);
        assert!(secret.ends_with(&row.hint));
        assert_eq!(allowed_application_ids_of(&row), Some(vec![2, 5]));

        // Autenticação atualiza last_used_at e last_used_ip
        let (authed, authed_user) = authenticate(&db, &secret, Some("10.8.0.5".into()))
            .await
            .expect("auth")
            .expect("token válido");
        assert_eq!(authed_user.id, admin.id);
        assert_eq!(authed.last_used_ip.as_deref(), Some("10.8.0.5"));
        assert!(authed.last_used_at.is_some());

        // Outro usuário não consegue revogar
        assert!(matches!(
            revoke(&db, row.id, Some(viewer.id)).await,
            Err(AppError::NotFound(_))
        ));

        // Dono revoga e token para de autenticar
        revoke(&db, row.id, Some(admin.id)).await.expect("revogar");
        assert!(authenticate(&db, &secret, None)
            .await
            .expect("auth pós-revogação")
            .is_none());
    }
}
