//! Regra global para diferenciar ICMP filtrado de equipamento indisponível.

use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DatabaseBackend, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

fn insert_sql(backend: DatabaseBackend) -> String {
    let condition = r#"{"field":"reachabilityCause","operator":"eq","value":"icmp_filtered"}"#;
    let condition = if backend == DatabaseBackend::Postgres {
        format!("'{condition}'::jsonb")
    } else {
        format!("'{condition}'")
    };
    format!(
        "INSERT INTO alert_rules (site_id, device_id, monitor_id, name, type, template_key, condition, severity, duration_seconds, recovery_window_seconds, flap_threshold, flap_window_seconds, notification_cooldown_seconds, inhibit_when_parent_down, enabled, created_at, updated_at) \
         SELECT NULL, NULL, NULL, 'ICMP filtrado ou desativado', 'icmp_filtered', 'icmp_filtered', {condition}, 'warning', 0, 120, 5, 900, 900, TRUE, TRUE, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP \
         WHERE EXISTS (SELECT 1 FROM alert_rules) \
           AND NOT EXISTS (SELECT 1 FROM alert_rules WHERE template_key = 'icmp_filtered')"
    )
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = db.get_database_backend();
        db.execute_raw(Statement::from_string(backend, insert_sql(backend)))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = db.get_database_backend();
        db.execute_raw(Statement::from_string(
            backend,
            "DELETE FROM alert_rules WHERE template_key = 'icmp_filtered' AND site_id IS NULL AND device_id IS NULL AND monitor_id IS NULL".to_string(),
        ))
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::{
        sea_orm::{ConnectionTrait, Database},
        MigratorTrait,
    };

    async fn schema_antes_desta_migration() -> sea_orm_migration::sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let previous = <u32 as std::convert::TryFrom<usize>>::try_from(
            crate::Migrator::migrations().len() - 1,
        )
        .unwrap();
        crate::Migrator::up(&db, Some(previous)).await.unwrap();
        db
    }

    async fn count(db: &impl ConnectionTrait, predicate: &str) -> i64 {
        db.query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT COUNT(*) AS total FROM alert_rules {predicate}"),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "total")
        .unwrap()
    }

    #[tokio::test]
    async fn banco_novo_sem_regras_fica_para_o_ensure_defaults() {
        let db = schema_antes_desta_migration().await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        assert_eq!(count(&db, "").await, 0);
    }

    #[tokio::test]
    async fn banco_existente_recebe_uma_regra_e_a_reaplicacao_e_idempotente() {
        let db = schema_antes_desta_migration().await;
        db.execute_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "INSERT INTO alert_rules (name, type, condition, severity, enabled, created_at, updated_at) VALUES ('existente', 'custom', '{}', 'warning', TRUE, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)".to_string(),
        ))
        .await
        .unwrap();
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        Migration.up(&manager).await.unwrap();
        assert_eq!(count(&db, "WHERE template_key = 'icmp_filtered'").await, 1);
    }

    #[test]
    fn json_usa_o_tipo_correto_em_cada_dialeto() {
        assert!(!insert_sql(DatabaseBackend::Sqlite).contains("::jsonb"));
        assert!(insert_sql(DatabaseBackend::Postgres).contains("::jsonb"));
    }
}
