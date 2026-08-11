//! Peças compartilhadas pelas migrations do esquema (§6 do roadmap).
//!
//! **Por que não usar `loco_rs::schema::create_table` com o parâmetro `refs`.**
//! O helper do Loco deriva a ação da FK da nulabilidade da coluna: anulável →
//! `SET NULL`, obrigatória → `CASCADE`. O esquema do AdonisJS não segue essa
//! regra e não pode seguir — `probes.site_id`, `networks.site_id`,
//! `devices.site_id`, `monitors.device_id` e as três FKs de `alert_rules` são
//! **anuláveis com `CASCADE`**. Apagar um site tem que apagar o que pertence a
//! ele; anular o vínculo deixaria órfãos invisíveis na interface.
//!
//! Então as FKs são declaradas à mão, com `ForeignKey::create()` **dentro do
//! `CREATE TABLE`** — e não por `ALTER TABLE` depois, porque o SQLite não
//! aceita adicionar constraint de FK a uma tabela existente, e a §15 Fase 1
//! exige que a migração rode nos dois bancos.
//!
//! O resto continua sendo o padrão do Loco: `ColType` para os tipos e
//! `timestamps_tz` para as colunas de tempo.

use loco_rs::schema::timestamps_tz;
use sea_orm_migration::prelude::*;

/// Construtores de coluna do SeaORM (`string`, `big_integer_null`,
/// `json_binary`, …). O `ColType` do Loco não serve aqui: o `to_def` que o
/// converte em `ColumnDef` é privado no crate.
pub use sea_orm_migration::schema::*;

/// Abre um `CREATE TABLE`. Os `created_at`/`updated_at` entram no fim, via
/// [`with_timestamps`] ou [`append_only`].
pub fn table(name: &str) -> TableCreateStatement {
    Table::create()
        .table(Alias::new(name))
        .if_not_exists()
        .take()
}

/// Fecha a tabela com `created_at` + `updated_at` — o padrão do Loco.
///
/// Diferença deliberada do Adonis, registrada na §6: lá o `updated_at` é
/// anulável e sem default. Aqui é `NOT NULL DEFAULT now()`, porque uma linha
/// sempre tem um instante de última escrita e `null` nesse campo só produz
/// ramo morto no código que a lê.
pub fn with_timestamps(stmt: TableCreateStatement) -> TableCreateStatement {
    timestamps_tz(stmt)
}

/// Fecha uma tabela **append-only** com apenas `created_at`.
///
/// Vale para `monitor_results`, `metrics`, `zabbix_template_items`,
/// `event_outbox` e `probe_tasks`. Não é economia estética: as duas primeiras
/// são as tabelas de maior volume do sistema e recebem inserção em rajada a
/// cada coleta. Uma coluna de 8 bytes que nunca é lida, multiplicada por
/// milhões de linhas, é largura de banda de escrita jogada fora.
pub fn append_only(stmt: TableCreateStatement) -> TableCreateStatement {
    let mut stmt = stmt;
    stmt.col(
        ColumnDef::new(Alias::new("created_at"))
            .timestamp_with_time_zone()
            .not_null()
            .default(Expr::current_timestamp())
            .take(),
    );
    stmt.take()
}

/// Chave estrangeira com ação de `ON DELETE` explícita.
///
/// O nome segue a convenção do Loco (`fk-{filha}-{coluna}-to-{pai}`) para que
/// `remove_reference` consiga derrubá-la. Os nomes de **índice**, esses sim,
/// são os do Adonis — ver [`index`].
pub fn fk(
    child: &str,
    column: &str,
    parent: &str,
    on_delete: ForeignKeyAction,
) -> ForeignKeyCreateStatement {
    ForeignKey::create()
        .name(format!("fk-{child}-{column}-to-{parent}"))
        .from(Alias::new(child), Alias::new(column))
        .to(Alias::new(parent), Alias::new("id"))
        .on_delete(on_delete)
        .on_update(ForeignKeyAction::Cascade)
        .take()
}

/// Índice **com o nome do Adonis**. A §6 exige que os nomes sejam idênticos: a
/// escolha de cada índice está justificada por escrito nas migrations atuais, e
/// o nome é o que liga o índice à justificativa.
pub fn index(name: &str, table: &str, columns: &[&str]) -> IndexCreateStatement {
    let mut stmt = Index::create();
    stmt.name(name).table(Alias::new(table));
    for column in columns {
        stmt.col(Alias::new(*column));
    }
    stmt.take()
}

/// Índice único, mesma regra de nome do [`index`].
pub fn unique(name: &str, table: &str, columns: &[&str]) -> IndexCreateStatement {
    let mut stmt = index(name, table, columns);
    stmt.unique();
    stmt.take()
}

/// `DROP TABLE` do `down()`.
pub fn drop(name: &str) -> TableDropStatement {
    Table::drop().table(Alias::new(name)).if_exists().take()
}
