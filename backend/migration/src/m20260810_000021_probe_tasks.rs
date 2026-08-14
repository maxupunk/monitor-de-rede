//! §6 #21 — `probe_tasks`.
//!
//! Fila de tarefas dos probes.
//!
//! Mesmo problema — e mesma solução — da `event_outbox`: quem enfileira é o
//! scheduler e quem entrega é o processo HTTP, que responde ao
//! `GET /api/probes/tasks`. Enquanto a fila viveu num mapa estático em memória,
//! o scheduler empilhava tarefas no próprio processo e o probe consultava uma
//! fila sempre vazia: nenhum monitor atribuído a probe rodava sozinho, ficando
//! eternamente em `unknown`.
//!
//! `monitor_id` é único: um monitor tem no máximo uma tarefa pendente. Com probe
//! offline a linha é substituída a cada ciclo em vez de acumular, e quando ele
//! volta executa uma checagem atual por monitor — não uma avalanche de tarefas
//! vencidas. O UNIQUE é também a defesa contra ciclos concorrentes do scheduler
//! (ADR 005).

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let mut stmt = table("probe_tasks");
        stmt.col(big_pk_auto("id"))
            .col(big_integer("probe_id"))
            .col(big_integer("monitor_id"))
            // Identificador que o probe devolve junto do resultado.
            .col(string("task_id"))
            .col(string("type"))
            .col(integer("timeout_ms"))
            .col(json_binary("payload"))
            .foreign_key(&mut fk(
                "probe_tasks",
                "probe_id",
                "probes",
                ForeignKeyAction::Cascade,
            ))
            .foreign_key(&mut fk(
                "probe_tasks",
                "monitor_id",
                "monitors",
                ForeignKeyAction::Cascade,
            ));

        m.create_table(append_only(stmt.take())).await?;

        m.create_index(unique(
            "probe_tasks_monitor_id_unique",
            "probe_tasks",
            &["monitor_id"],
        ))
        .await?;

        // A entrega é `where(probe_id) order by id` — `created_at` só é lido para
        // descartar tarefas vencidas, em memória, e por isso não entra no índice.
        m.create_index(index(
            "probe_tasks_probe_id_id_index",
            "probe_tasks",
            &["probe_id", "id"],
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("probe_tasks")).await?;
        Ok(())
    }
}
