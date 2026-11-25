//! O modo "Aceitar automaticamente" da IA.
//!
//! Com ele ligado, os acessos da IA a **um** equipamento, **numa** conversa,
//! deixam de pedir aprovação um a um. Só liga com o termo aceito, e o aceite
//! fica registrado (quem, quando, qual versão do termo, até quando). Vale por
//! no máximo [`TTL`], e o operador desliga a qualquer momento.

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};

use netmonitor_core::{
    models::{devices, plugin_auto_accept},
    shared::errors::{AppError, AppResult},
};

/// Muda quando o texto do termo muda: aceite de versão antiga não vale.
pub const TERMS_VERSION: &str = "1";

pub const TERMS: &str = "Ao ligar o modo \"Aceitar automaticamente\", a IA passa a acessar este \
equipamento e a executar comandos nele SEM pedir sua confirmação a cada passo.\n\n\
• A IA pode alucinar: interpretar errado a saída do equipamento, inventar comandos ou \
parâmetros e executar alterações incorretas.\n\
• Uma alteração errada pode derrubar a rede, cortar o seu acesso de gerência ou deixar o \
equipamento inutilizável até uma restauração manual.\n\
• O NetMonitor e seus mantenedores NÃO se responsabilizam por qualquer dano, perda de \
configuração, indisponibilidade ou prejuízo causado pelas ações executadas pela IA no \
equipamento.\n\
• Recomenda-se fazer backup da configuração antes e acompanhar a execução. Você pode \
desligar o modo automático a qualquer momento.\n\n\
Todo acesso continua registrado, com o motivo informado pela IA.";

/// Validade máxima de um aceite.
pub const TTL: chrono::Duration = chrono::Duration::hours(2);

fn validate_key(conversation_key: &str) -> AppResult<&str> {
    let key = conversation_key.trim();
    if key.is_empty() || key.len() > 64 {
        return Err(AppError::validation("Conversa inválida."));
    }
    Ok(key)
}

/// O aceite em vigor, se houver — com `user_id`, só o que essa pessoa deu
/// (o mesmo critério que a IA usa para dispensar a confirmação).
///
/// # Errors
///
/// Erro do banco.
pub async fn active<C: ConnectionTrait>(
    db: &C,
    conversation_key: &str,
    device_id: i64,
    user_id: Option<i64>,
) -> AppResult<Option<plugin_auto_accept::Model>> {
    let Ok(key) = validate_key(conversation_key) else {
        return Ok(None);
    };
    let mut query = plugin_auto_accept::Entity::find()
        .filter(plugin_auto_accept::Column::ConversationKey.eq(key))
        .filter(plugin_auto_accept::Column::DeviceId.eq(device_id));
    if let Some(user_id) = user_id {
        query = query.filter(plugin_auto_accept::Column::UserId.eq(user_id));
    }
    Ok(query
        .filter(plugin_auto_accept::Column::TermsVersion.eq(TERMS_VERSION))
        .filter(plugin_auto_accept::Column::RevokedAt.is_null())
        .filter(plugin_auto_accept::Column::ExpiresAt.gt(Utc::now()))
        .order_by_desc(plugin_auto_accept::Column::Id)
        .one(db)
        .await?)
}

/// Verifica se existe aceite em vigor concedido explicitamente por `user_id`.
///
/// # Errors
///
/// Erro do banco.
pub async fn active_for_user<C: ConnectionTrait>(
    db: &C,
    conversation_key: &str,
    device_id: i64,
    user_id: i64,
) -> AppResult<bool> {
    Ok(active(db, conversation_key, device_id, Some(user_id))
        .await?
        .is_some())
}

/// Liga o modo automático para a conversa × dispositivo.
///
/// # Errors
///
/// Termo não aceito, conversa inválida ou dispositivo inexistente.
pub async fn accept<C: ConnectionTrait>(
    db: &C,
    conversation_key: &str,
    device_id: i64,
    user_id: Option<i64>,
    accept_terms: bool,
) -> AppResult<plugin_auto_accept::Model> {
    if !accept_terms {
        return Err(AppError::validation(
            "É preciso marcar \"Estou ciente\" para ligar o modo automático.",
        ));
    }
    let key = validate_key(conversation_key)?;
    if devices::Entity::find_by_id(device_id)
        .one(db)
        .await?
        .is_none()
    {
        return Err(AppError::not_found("Dispositivo não encontrado."));
    }
    revoke(db, key, device_id, user_id).await?;
    Ok(plugin_auto_accept::ActiveModel {
        conversation_key: Set(key.to_owned()),
        device_id: Set(device_id),
        user_id: Set(user_id),
        terms_version: Set(TERMS_VERSION.to_owned()),
        expires_at: Set((Utc::now() + TTL).into()),
        revoked_at: Set(None),
        ..Default::default()
    }
    .insert(db)
    .await?)
}

/// Desliga. Idempotente. Com `user_id`, só o aceite dessa pessoa; `None`
/// (administrador) desliga o de todos.
///
/// # Errors
///
/// Erro do banco.
pub async fn revoke<C: ConnectionTrait>(
    db: &C,
    conversation_key: &str,
    device_id: i64,
    user_id: Option<i64>,
) -> AppResult<()> {
    let Ok(key) = validate_key(conversation_key) else {
        return Ok(());
    };
    let mut update = plugin_auto_accept::Entity::update_many();
    if let Some(user_id) = user_id {
        update = update.filter(plugin_auto_accept::Column::UserId.eq(user_id));
    }
    update
        .col_expr(
            plugin_auto_accept::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(chrono::DateTime::<chrono::FixedOffset>::from(
                Utc::now(),
            )),
        )
        .filter(plugin_auto_accept::Column::ConversationKey.eq(key))
        .filter(plugin_auto_accept::Column::DeviceId.eq(device_id))
        .filter(plugin_auto_accept::Column::RevokedAt.is_null())
        .exec(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::{MigratorTrait, TestMigrator as Migrator};
    use sea_orm::Database;

    #[tokio::test]
    async fn aceite_vale_so_para_a_conversa_e_o_equipamento() {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:".to_owned());
        options.max_connections(1).min_connections(1);
        let db = Database::connect(options).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let device = devices::ActiveModel {
            name: Set("Roteador".into()),
            r#type: Set("router".into()),
            status: Set("online".into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        assert!(accept(&db, "conversa-1", device.id, None, false)
            .await
            .is_err());
        accept(&db, "conversa-1", device.id, None, true)
            .await
            .unwrap();
        assert!(active(&db, "conversa-1", device.id, None)
            .await
            .unwrap()
            .is_some());
        assert!(active(&db, "conversa-2", device.id, None)
            .await
            .unwrap()
            .is_none());
        assert!(active(&db, "conversa-1", device.id + 1, None)
            .await
            .unwrap()
            .is_none());

        revoke(&db, "conversa-1", device.id, None).await.unwrap();
        assert!(active(&db, "conversa-1", device.id, None)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn aceite_por_usuario_vale_apenas_para_quem_aceitou() {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:".to_owned());
        options.max_connections(1).min_connections(1);
        let db = Database::connect(options).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let user = netmonitor_core::models::_entities::users::ActiveModel {
            pid: Set(uuid::Uuid::new_v4()),
            email: Set("admin@teste.local".into()),
            password: Set("hash".into()),
            api_key: Set("key-1".into()),
            name: Set("Admin".into()),
            active: Set(true),
            role: Set("admin".into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let device = devices::ActiveModel {
            name: Set("Roteador".into()),
            r#type: Set("router".into()),
            status: Set("online".into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        accept(&db, "conversa-u", device.id, Some(user.id), true)
            .await
            .unwrap();
        assert!(active_for_user(&db, "conversa-u", device.id, user.id)
            .await
            .unwrap());
        assert!(!active_for_user(&db, "conversa-u", device.id, user.id + 1)
            .await
            .unwrap());
        assert!(!active_for_user(&db, "outra", device.id, user.id)
            .await
            .unwrap());

        // A tela vê o mesmo que a IA: o aceite de outra pessoa não aparece.
        assert!(active(&db, "conversa-u", device.id, Some(user.id + 1))
            .await
            .unwrap()
            .is_none());
        // Outra pessoa não desliga o aceite alheio; quem aceitou, sim.
        revoke(&db, "conversa-u", device.id, Some(user.id + 1))
            .await
            .unwrap();
        assert!(active_for_user(&db, "conversa-u", device.id, user.id)
            .await
            .unwrap());
        revoke(&db, "conversa-u", device.id, Some(user.id))
            .await
            .unwrap();
        assert!(!active_for_user(&db, "conversa-u", device.id, user.id)
            .await
            .unwrap());
    }
}
