//! Tabelas do assistente (ADR 017, Fase 13). Só têm FK para tabelas do
//! núcleo; o ponto de montagem as junta depois das do núcleo.

pub use netmonitor_core::migrations::{MigrationTrait, MigratorTrait};

mod m20260922_000001_ai_conversations;

/// As migrations da área, na ordem de criação.
#[must_use]
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20260922_000001_ai_conversations::Migration)]
}

#[cfg(test)]
netmonitor_core::area_test_migrator!(TestMigrator, migrations);
