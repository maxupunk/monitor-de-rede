//! Catálogo das tabelas do esquema e limpeza para os testes.
//!
//! [`CREATION_ORDER`] é a ordem de criação da §6 do roadmap — a mesma ordem em
//! que as migrations rodam, porque cada tabela só pode nascer depois dos alvos
//! das suas FKs. Manter a lista num lugar só evita o problema clássico de o
//! `truncate` dos testes esquecer uma tabela nova e um teste passar a depender
//! de sujeira deixada pelo anterior.
//!
//! O `Hooks::truncate` do Loco recebe entidades SeaORM, que só existem depois
//! de `cargo loco db entities` (Fase 1). Enquanto o esquema é construído, a
//! limpeza é feita por nome, pulando tabelas ainda inexistentes — assim a lista
//! já pode estar completa desde a Fase 0.

use std::collections::HashSet;

use loco_rs::Result;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

/// As 26 tabelas do esquema, na ordem de criação da §6 (pais antes de filhos).
pub const CREATION_ORDER: [&str; 26] = [
    "users",
    "sites",
    "probes",
    "networks",
    "devices",
    "device_interfaces",
    "device_links",
    "monitors",
    "monitor_results",
    "monitor_results_hourly",
    "metrics",
    "discovery_runs",
    "discovery_results",
    "alert_rules",
    "alert_events",
    "vpn_servers",
    "vpn_peers",
    "dns_servers",
    "event_outbox",
    // Fase 4 do roadmap de alertas inteligentes: o diário de notificações.
    // Depois de `alert_rules` e `devices`, para quem ela referencia já existir.
    "notification_outbox",
    "probe_tasks",
    "system_settings",
    // Fase 3 do roadmap: janelas de manutenção. Depois de `sites`,
    // `devices` e `users`, para as FKs já existirem.
    "maintenance_windows",
    // Fase 3 do roadmap: trilha de auditoria. Depois de `users` pela FK.
    "audit_logs",
    // Notificações Web Push (PWA). Depois de `users` pela FK.
    "push_subscriptions",
    // Opcional (§6 #23 / §10). **Não migrada:** a §10.2 optou por
    // `loco_rs::auth::JWT`, que não guarda token no banco. Fica listada porque
    // a limpeza pula tabelas inexistentes e a Fase 6 ainda pode voltar atrás.
    "auth_tokens",
];

/// Tabela de controle do migrator. Precisa ser esvaziada entre testes para que
/// o `auto_migrate` do Loco reaplique as migrations sem erro de UNIQUE.
pub const MIGRATIONS_TABLE: &str = "seaql_migrations";

/// Ordem segura de limpeza: filhos antes dos pais, para não violar FK em bancos
/// que as checam (Postgres sempre; SQLite quando `foreign_keys=ON`).
#[must_use]
pub fn truncate_order() -> Vec<&'static str> {
    CREATION_ORDER.iter().rev().copied().collect()
}

/// Tabelas que existem de fato no banco conectado.
///
/// # Errors
///
/// Propaga falha de consulta ao catálogo do banco.
pub async fn existing_tables<C: ConnectionTrait>(db: &C) -> Result<HashSet<String>> {
    let backend = db.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Sqlite => {
            "SELECT name AS table_name FROM sqlite_master WHERE type = 'table'"
        }
        DatabaseBackend::Postgres => {
            "SELECT table_name::text AS table_name FROM information_schema.tables \
             WHERE table_schema = current_schema()"
        }
        // `DatabaseBackend` é `non_exhaustive`; MySQL e qualquer backend futuro
        // caem no catálogo padrão do SQL. Só SQLite e Postgres são suportados
        // pelo projeto (§13), então isto é rede de segurança, não promessa.
        _ => {
            "SELECT table_name AS table_name FROM information_schema.tables \
             WHERE table_schema = database()"
        }
    };

    // `*_raw` recebe um `Statement` pronto; `query_all` espera um builder do
    // sea-query, que não serve para SQL escrito à mão.
    let rows = db
        .query_all_raw(Statement::from_string(backend, sql))
        .await?;
    let mut names = HashSet::new();
    for row in rows {
        names.insert(row.try_get::<String>("", "table_name")?);
    }
    Ok(names)
}

/// Apaga todas as linhas das tabelas do esquema, ignorando as que ainda não
/// foram criadas.
///
/// Usa `DELETE FROM` e não `TRUNCATE`: o SQLite não tem `TRUNCATE`, e no
/// Postgres o `DELETE` dispensa o `CASCADE` quando a ordem já é a correta.
///
/// # Errors
///
/// Propaga falha do banco em qualquer uma das tabelas.
pub async fn truncate_all<C: ConnectionTrait>(db: &C) -> Result<()> {
    let existing = existing_tables(db).await?;
    let backend = db.get_database_backend();

    for table in truncate_order() {
        if !existing.contains(table) {
            continue;
        }
        db.execute_raw(Statement::from_string(
            backend,
            format!("DELETE FROM \"{table}\""),
        ))
        .await?;
    }
    // Limpa o controle do migrator por último, depois que todo o schema está
    // vazio — assim o `auto_migrate` dos testes reaplica as migrations sem
    // conflito de versão.
    if existing.contains(MIGRATIONS_TABLE) {
        db.execute_raw(Statement::from_string(
            backend,
            format!("DELETE FROM \"{MIGRATIONS_TABLE}\""),
        ))
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cobre_as_26_tabelas_sem_repetir() {
        assert_eq!(CREATION_ORDER.len(), 26);
        let unicas: HashSet<&&str> = CREATION_ORDER.iter().collect();
        assert_eq!(unicas.len(), 26, "há nome de tabela repetido");
    }

    #[test]
    fn limpeza_comeca_pelos_filhos() {
        let ordem = truncate_order();
        let posicao = |nome: &str| ordem.iter().position(|t| *t == nome).unwrap();
        // `devices` referencia `sites` e `networks`; tem de sumir antes deles.
        assert!(posicao("devices") < posicao("sites"));
        assert!(posicao("devices") < posicao("networks"));
        // `monitor_results` e seu rollup referenciam `monitors`.
        assert!(posicao("monitor_results") < posicao("monitors"));
        assert!(posicao("monitor_results_hourly") < posicao("monitors"));
        assert!(posicao("monitor_results_hourly") < posicao("monitor_results"));
        // `notification_outbox` referencia `alert_rules` e `devices`.
        assert!(posicao("notification_outbox") < posicao("alert_rules"));
        assert!(posicao("notification_outbox") < posicao("devices"));
        // `users` é a raiz: sempre por último.
        assert_eq!(*ordem.last().unwrap(), "users");
    }
}
