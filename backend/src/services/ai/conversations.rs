//! Conversas salvas do Assistente IA, uma lista por usuário.
//!
//! Toda operação recebe o dono: uma conversa de outro usuário responde como
//! inexistente, sem revelar que o id existe. As mensagens são guardadas como
//! a tela as mostra (texto, ferramentas, gráficos) e nunca consultadas por
//! dentro — o backend só valida forma e tamanho.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde_json::Value;

use crate::{
    dtos::ai::{AiConversationDetail, AiConversationInput, AiConversationSummary},
    models::ai_conversations,
    services::shared::errors::{AppError, AppResult},
};

/// Conversas guardadas por usuário; ao passar disso, as mais antigas saem.
pub const MAX_CONVERSATIONS_PER_USER: u64 = 100;
/// Tamanho máximo do JSON de mensagens de uma conversa (gráficos incluídos).
pub const MAX_MESSAGES_BYTES: usize = 2 * 1024 * 1024;
/// Caracteres do título; a coluna comporta 160.
pub const MAX_TITLE_CHARS: usize = 120;
const DEFAULT_TITLE: &str = "Nova conversa";

/// Conversa validada, pronta para gravar.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidConversation {
    pub title: String,
    pub messages: Value,
    pub message_count: i32,
}

/// Título limpo: espaços colapsados, corte por caracteres, padrão se vazio.
#[must_use]
pub fn normalize_title(raw: &str) -> String {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return DEFAULT_TITLE.to_string();
    }
    if collapsed.chars().count() <= MAX_TITLE_CHARS {
        return collapsed;
    }
    let mut cut: String = collapsed.chars().take(MAX_TITLE_CHARS - 1).collect();
    cut.push('…');
    cut
}

/// # Errors
///
/// Mensagens que não são uma lista ou que passam de [`MAX_MESSAGES_BYTES`].
pub fn validate(input: AiConversationInput) -> AppResult<ValidConversation> {
    let Value::Array(list) = &input.messages else {
        return Err(AppError::validation(
            "As mensagens da conversa devem ser uma lista",
        ));
    };
    let message_count = i32::try_from(list.len())
        .map_err(|_| AppError::validation("Conversa com mensagens demais"))?;
    let size = serde_json::to_vec(&input.messages)
        .map_err(|error| AppError::Internal(anyhow::Error::new(error)))?
        .len();
    if size > MAX_MESSAGES_BYTES {
        return Err(AppError::validation(format!(
            "Conversa grande demais para salvar ({} KB; limite de {} KB). Comece uma conversa nova.",
            size / 1024,
            MAX_MESSAGES_BYTES / 1024
        )));
    }
    Ok(ValidConversation {
        title: normalize_title(&input.title),
        messages: input.messages,
        message_count,
    })
}

fn summary(row: &ai_conversations::Model) -> AiConversationSummary {
    AiConversationSummary {
        id: row.id,
        title: row.title.clone(),
        updated_at: row.updated_at.to_rfc3339(),
        message_count: row.message_count,
    }
}

fn not_found() -> AppError {
    AppError::not_found("Conversa não encontrada")
}

async fn owned<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    id: i64,
) -> AppResult<ai_conversations::Model> {
    ai_conversations::Entity::find_by_id(id)
        .filter(ai_conversations::Column::UserId.eq(user_id))
        .one(db)
        .await?
        .ok_or_else(not_found)
}

/// Conversas do usuário, mais recentes primeiro, sem as mensagens.
///
/// # Errors
///
/// Erro do banco.
pub async fn list<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> AppResult<Vec<AiConversationSummary>> {
    let rows: Vec<(i64, String, sea_orm::prelude::DateTimeWithTimeZone, i32)> =
        ai_conversations::Entity::find()
            .select_only()
            .columns([
                ai_conversations::Column::Id,
                ai_conversations::Column::Title,
                ai_conversations::Column::UpdatedAt,
                ai_conversations::Column::MessageCount,
            ])
            .filter(ai_conversations::Column::UserId.eq(user_id))
            .order_by_desc(ai_conversations::Column::UpdatedAt)
            .order_by_desc(ai_conversations::Column::Id)
            .into_tuple()
            .all(db)
            .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, title, updated_at, message_count)| AiConversationSummary {
                id,
                title,
                updated_at: updated_at.to_rfc3339(),
                message_count,
            },
        )
        .collect())
}

/// # Errors
///
/// Conversa inexistente ou de outro usuário, ou erro do banco.
pub async fn get<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    id: i64,
) -> AppResult<AiConversationDetail> {
    let row = owned(db, user_id, id).await?;
    Ok(AiConversationDetail {
        id: row.id,
        title: row.title,
        updated_at: row.updated_at.to_rfc3339(),
        messages: row.messages,
    })
}

/// Apaga as conversas do usuário que passaram do teto, das mais antigas.
///
/// O corte é feito em memória, não com `OFFSET`: o SQLite não aceita `OFFSET`
/// sem `LIMIT`, e aqui a lista tem no máximo o teto mais uma.
async fn prune<C: ConnectionTrait>(db: &C, user_id: i64) -> AppResult<()> {
    let ids: Vec<i64> = ai_conversations::Entity::find()
        .select_only()
        .column(ai_conversations::Column::Id)
        .filter(ai_conversations::Column::UserId.eq(user_id))
        .order_by_desc(ai_conversations::Column::UpdatedAt)
        .order_by_desc(ai_conversations::Column::Id)
        .into_tuple()
        .all(db)
        .await?;
    let excess: Vec<i64> = ids
        .into_iter()
        .skip(MAX_CONVERSATIONS_PER_USER as usize)
        .collect();
    if !excess.is_empty() {
        ai_conversations::Entity::delete_many()
            .filter(ai_conversations::Column::Id.is_in(excess))
            .exec(db)
            .await?;
    }
    Ok(())
}

/// # Errors
///
/// Conversa inválida ou erro do banco.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    input: AiConversationInput,
) -> AppResult<AiConversationSummary> {
    let valid = validate(input)?;
    let row = ai_conversations::ActiveModel {
        user_id: Set(user_id),
        title: Set(valid.title),
        messages: Set(valid.messages),
        message_count: Set(valid.message_count),
        ..Default::default()
    }
    .insert(db)
    .await?;
    prune(db, user_id).await?;
    Ok(summary(&row))
}

/// Substitui título e mensagens.
///
/// # Errors
///
/// Conversa inexistente ou de outro usuário, inválida, ou erro do banco.
pub async fn update<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    id: i64,
    input: AiConversationInput,
) -> AppResult<AiConversationSummary> {
    let valid = validate(input)?;
    let mut active: ai_conversations::ActiveModel = owned(db, user_id, id).await?.into();
    active.title = Set(valid.title);
    active.messages = Set(valid.messages);
    active.message_count = Set(valid.message_count);
    let row = active.update(db).await?;
    Ok(summary(&row))
}

/// # Errors
///
/// Conversa inexistente ou de outro usuário, ou erro do banco.
pub async fn delete<C: ConnectionTrait>(db: &C, user_id: i64, id: i64) -> AppResult<()> {
    let removed = ai_conversations::Entity::delete_many()
        .filter(ai_conversations::Column::Id.eq(id))
        .filter(ai_conversations::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    if removed.rows_affected == 0 {
        return Err(not_found());
    }
    Ok(())
}

/// Quantas conversas o usuário tem guardadas.
///
/// # Errors
///
/// Erro do banco.
pub async fn count<C: ConnectionTrait>(db: &C, user_id: i64) -> AppResult<u64> {
    Ok(ai_conversations::Entity::find()
        .filter(ai_conversations::Column::UserId.eq(user_id))
        .count(db)
        .await?)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn entrada(title: &str, messages: Value) -> AiConversationInput {
        AiConversationInput {
            title: title.into(),
            messages,
        }
    }

    #[test]
    fn titulo_colapsa_espacos_corta_e_tem_padrao() {
        assert_eq!(normalize_title("  ping   no\ngateway "), "ping no gateway");
        assert_eq!(normalize_title("   "), DEFAULT_TITLE);
        let longo = normalize_title(&"ç".repeat(300));
        assert_eq!(longo.chars().count(), MAX_TITLE_CHARS);
        assert!(longo.ends_with('…'));
    }

    #[test]
    fn mensagens_precisam_ser_lista_e_caber_no_limite() {
        let ok = validate(entrada(
            "x",
            json!([{ "role": "user" }, { "role": "assistant" }]),
        ))
        .unwrap();
        assert_eq!(ok.message_count, 2);

        assert!(validate(entrada("x", json!({ "role": "user" }))).is_err());

        let enorme = json!(["a".repeat(MAX_MESSAGES_BYTES)]);
        assert!(validate(entrada("x", enorme)).is_err());
    }
}
