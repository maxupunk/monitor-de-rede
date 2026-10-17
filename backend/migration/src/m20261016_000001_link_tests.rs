//! Teste de enlace (ADR 015 §5): testes, pernas e agendamentos.
//!
//! * `link_test_schedules` — repetição de um teste (intervalo em minutos,
//!   janela de horário opcional). Vem primeiro: `link_tests.schedule_id`
//!   aponta para ela.
//! * `link_tests` — um teste pedido (pela tela ou por um agendamento): o
//!   tipo, o alvo, a especificação, o estado e o resumo.
//! * `link_test_legs` — as pernas do teste, cada uma com as etapas (`stages`,
//!   JSON), a vazão de cada sentido, as amostras e a **limpeza pendente**
//!   (`cleanup`): o `handle` do servidor que o equipamento subiu, gravado
//!   antes de usá-lo para que o boot o desfaça se o processo cair no meio.
//!
//! Sem FK para `devices`: o histórico guarda o nome e o IP de cada ponta no
//! próprio JSON da perna e sobrevive à exclusão do equipamento.

use sea_orm_migration::prelude::*;

use crate::shared::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("link_test_schedules").await? {
            let mut stmt = table("link_test_schedules");
            stmt.col(big_pk_auto("id"))
                .col(string_len("name", 120))
                .col(string_len("kind", 16))
                .col(big_integer_null("link_id"))
                .col(big_integer_null("source_device_id"))
                .col(big_integer_null("target_device_id"))
                .col(json_binary("spec"))
                .col(integer("interval_minutes"))
                .col(string_len_null("window_start", 5))
                .col(string_len_null("window_end", 5))
                .col(boolean("enabled").default(true))
                .col(timestamp_with_time_zone_null("last_run_at"))
                .col(timestamp_with_time_zone_null("next_run_at"))
                .col(big_integer_null("last_test_id"))
                .col(string_len_null("last_status", 16))
                .col(big_integer_null("user_id"))
                .foreign_key(&mut fk(
                    "link_test_schedules",
                    "user_id",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(index(
                "link_test_schedules_enabled_next_index",
                "link_test_schedules",
                &["enabled", "next_run_at"],
            ))
            .await?;
        }

        if !m.has_table("link_tests").await? {
            let mut stmt = table("link_tests");
            stmt.col(big_pk_auto("id"))
                .col(string_len("kind", 16))
                .col(string_len("status", 16))
                .col(string_len("origin", 16))
                .col(big_integer_null("schedule_id"))
                .col(big_integer_null("link_id"))
                .col(big_integer_null("source_device_id"))
                .col(big_integer_null("target_device_id"))
                // Todos os equipamentos das pernas, como `,1,5,9,`: o filtro
                // "testes deste dispositivo" vira um `LIKE` que vale igual no
                // SQLite e no PostgreSQL, sem consultar JSON.
                .col(text("device_ids").default(""))
                .col(json_binary("spec"))
                .col(json_binary_null("summary"))
                .col(text_null("error"))
                .col(big_integer_null("user_id"))
                .col(timestamp_with_time_zone_null("started_at"))
                .col(timestamp_with_time_zone_null("finished_at"))
                .foreign_key(&mut fk(
                    "link_tests",
                    "schedule_id",
                    "link_test_schedules",
                    ForeignKeyAction::SetNull,
                ))
                .foreign_key(&mut fk(
                    "link_tests",
                    "user_id",
                    "users",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(index("link_tests_status_index", "link_tests", &["status"]))
                .await?;
            m.create_index(index(
                "link_tests_created_index",
                "link_tests",
                &["created_at"],
            ))
            .await?;
        }

        if !m.has_table("link_test_legs").await? {
            let mut stmt = table("link_test_legs");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("test_id"))
                .col(integer("seq"))
                .col(string_len("kind", 16))
                .col(json_binary("a"))
                .col(json_binary("b"))
                .col(big_integer_null("edge_id"))
                .col(string_len_null("engine", 16))
                .col(string_len_null("server_side", 4))
                .col(integer("duration_s"))
                .col(boolean("runnable"))
                .col(json_binary("issues"))
                .col(string_len("status", 16))
                .col(json_binary("stages"))
                .col(double_null("forward_bps"))
                .col(double_null("reverse_bps"))
                .col(big_integer_null("retransmits"))
                .col(text_null("error"))
                .col(json_binary_null("samples"))
                .col(json_binary_null("cleanup"))
                .col(timestamp_with_time_zone_null("started_at"))
                .col(timestamp_with_time_zone_null("finished_at"))
                .foreign_key(&mut fk(
                    "link_test_legs",
                    "test_id",
                    "link_tests",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "link_test_legs_test_seq_unique",
                "link_test_legs",
                &["test_id", "seq"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("link_test_legs")).await?;
        m.drop_table(drop("link_tests")).await?;
        m.drop_table(drop("link_test_schedules")).await?;
        Ok(())
    }
}
