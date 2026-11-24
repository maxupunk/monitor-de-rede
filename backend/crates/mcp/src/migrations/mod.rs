//! Migrations do servidor MCP (`mcp_tokens`). Só referencia a tabela `users`
//! do núcleo.

pub use netmonitor_core::migrations::{MigrationTrait, MigratorTrait};

mod m20261124_000001_mcp_tokens;

/// As migrations deste crate.
#[must_use]
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261124_000001_mcp_tokens::Migration)]
}

#[cfg(test)]
pub(crate) struct TestMigrator;

#[cfg(test)]
impl MigratorTrait for TestMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        [netmonitor_core::migrations::migrations(), migrations()]
            .into_iter()
            .flatten()
            .collect()
    }
}
