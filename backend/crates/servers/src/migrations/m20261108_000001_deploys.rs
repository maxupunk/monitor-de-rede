//! Deploy contínuo (ADR 021).
//!
//! * `deploy_credentials` — o cofre: token do GitHub (leitura do repositório
//!   e `read:packages`), cifrado com a `ENCRYPTION_KEY`. Nunca volta à tela.
//! * `deploy_github_connections` — o GitHub App que a central criou na conta
//!   (ou organização) pelo manifest: lê o repositório, lista o que ele vê e,
//!   com permissão de escrita, abre o PR do workflow. A chave privada fica
//!   cifrada; os tokens de instalação duram uma hora e não são gravados.
//! * `deploy_applications` — um repositório: de onde vêm o compose e as
//!   imagens, e como o CI as publica (`images`, para o gerador do workflow).
//! * `deploy_targets` — onde a aplicação roda: o agente, o projeto compose, a
//!   branch que o alimenta e a política (aprovação, janela, backup, rollback).
//!   Apagar o agente não apaga o alvo nem o histórico — só o deixa sem host.
//! * `deploy_env_versions` — cada versão salva do `.env` de um alvo, cifrada.
//!   É histórico: a atual é a mais nova, e cada deploy guarda qual usou, para
//!   o rollback voltar o `.env` junto com a imagem.
//! * `deploy_runs` — cada deploy pedido, do webhook à conclusão, com o log.

use sea_orm_migration::prelude::*;

use netmonitor_core::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    #[allow(clippy::too_many_lines)]
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("deploy_credentials").await? {
            let mut stmt = table("deploy_credentials");
            stmt.col(big_pk_auto("id"))
                .col(string_len("name", 120))
                .col(string_len("registry", 255))
                .col(string_len("username", 128))
                .col(text("secret_encrypted"));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "deploy_credentials_name_unique",
                "deploy_credentials",
                &["name"],
            ))
            .await?;
        }

        if !m.has_table("deploy_github_connections").await? {
            let mut stmt = table("deploy_github_connections");
            stmt.col(big_pk_auto("id"))
                .col(string_len("account_login", 100))
                // `User` ou `Organization`: muda o endereço das telas do GitHub.
                .col(string_len("account_type", 16))
                .col(big_integer("app_id"))
                .col(string_len("app_slug", 100))
                .col(string_len("client_id", 64))
                .col(text("private_key_encrypted"))
                // Nula até o app ser instalado na conta.
                .col(big_integer_null("installation_id"))
                .col(boolean("can_write").default(false))
                // Segredo do webhook (o GitHub gera na criação do app): prova
                // que o aviso de push veio do app desta central (ADR 023).
                .col(text_null("webhook_secret_encrypted"));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "deploy_github_connections_app_unique",
                "deploy_github_connections",
                &["app_id"],
            ))
            .await?;
        }

        if !m.has_table("deploy_applications").await? {
            let mut stmt = table("deploy_applications");
            stmt.col(big_pk_auto("id"))
                .col(string_len("name", 120))
                .col(string_len("slug", 63))
                // `dono/repo` no GitHub: é o `repository` do token OIDC.
                .col(string_len("repository", 200))
                // O token do registry (e, sem conexão, também do repositório).
                .col(big_integer_null("credential_id"))
                .col(big_integer_null("github_connection_id"))
                .col(json_binary("compose_files"))
                .col(string_len("image_tag_variable", 64))
                // Imagens que o CI compila: `[{name, dockerfile, context}]`.
                .col(json_binary("images"))
                .col(string_len("rollout", 16))
                // Quem compila: `netmonitor` (o agente, ADR 023) ou
                // `github_actions` (o workflow, ADR 021).
                .col(string_len("builder", 16).default("github_actions"))
                // O agente que compila; nulo = o do primeiro alvo da revisão.
                .col(big_integer_null("build_probe_id"))
                // Estágio do Dockerfile que roda os testes antes das imagens.
                .col(string_len_null("test_target", 64))
                // `[{name, value}]` — argumentos de build (segredo não entra).
                .col(json_binary("build_args"))
                .foreign_key(&mut fk(
                    "deploy_applications",
                    "credential_id",
                    "deploy_credentials",
                    ForeignKeyAction::SetNull,
                ))
                .foreign_key(&mut fk(
                    "deploy_applications",
                    "github_connection_id",
                    "deploy_github_connections",
                    ForeignKeyAction::SetNull,
                ))
                .foreign_key(&mut fk(
                    "deploy_applications",
                    "build_probe_id",
                    "probes",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "deploy_applications_slug_unique",
                "deploy_applications",
                &["slug"],
            ))
            .await?;
            m.create_index(index(
                "deploy_applications_repository_index",
                "deploy_applications",
                &["repository"],
            ))
            .await?;
        }

        if !m.has_table("deploy_targets").await? {
            let mut stmt = table("deploy_targets");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("application_id"))
                .col(string_len("name", 120))
                .col(big_integer_null("probe_id"))
                .col(string_len("project", 63))
                // Branch (`main`) ou padrão de tag (`v*`).
                .col(string_len("reference", 120))
                .col(boolean("requires_approval").default(false))
                // Janela de manutenção, `HH:MM` no fuso da central; nula = a
                // qualquer hora.
                .col(string_len_null("window_start", 5))
                .col(string_len_null("window_end", 5))
                .col(big_integer_null("backup_connection_id"))
                .col(integer("wait_timeout_secs").default(180))
                .col(boolean("auto_rollback").default(true))
                .col(integer("keep_images").default(3))
                .col(boolean("enabled").default(true))
                .col(integer("position").default(0))
                // `warn`, `block_critical` ou `block_high` (ADR 022).
                .col(string_len("vulnerability_policy", 16).default("warn"))
                // Só imagem de origem conhecida: compilada pela central ou
                // enviada pelo workflow do próprio repositório.
                .col(boolean("require_provenance").default(false))
                .foreign_key(&mut fk(
                    "deploy_targets",
                    "application_id",
                    "deploy_applications",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "deploy_targets",
                    "probe_id",
                    "probes",
                    ForeignKeyAction::SetNull,
                ))
                .foreign_key(&mut fk(
                    "deploy_targets",
                    "backup_connection_id",
                    "database_connections",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "deploy_targets_application_name_unique",
                "deploy_targets",
                &["application_id", "name"],
            ))
            .await?;
        }

        if !m.has_table("deploy_env_versions").await? {
            let mut stmt = table("deploy_env_versions");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("target_id"))
                .col(text("content_encrypted"))
                // Só os nomes, para o histórico listar sem decifrar.
                .col(json_binary("keys"))
                .col(string_len_null("created_by", 120))
                .col(string_len_null("note", 200))
                .foreign_key(&mut fk(
                    "deploy_env_versions",
                    "target_id",
                    "deploy_targets",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(append_only(stmt.take())).await?;
            m.create_index(index(
                "deploy_env_versions_target_index",
                "deploy_env_versions",
                &["target_id", "id"],
            ))
            .await?;
        }

        if !m.has_table("deploy_build_secrets").await? {
            let mut stmt = table("deploy_build_secrets");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("application_id"))
                .col(string_len("name", 64))
                .col(text("value_encrypted"))
                .foreign_key(&mut fk(
                    "deploy_build_secrets",
                    "application_id",
                    "deploy_applications",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "deploy_build_secrets_application_name_unique",
                "deploy_build_secrets",
                &["application_id", "name"],
            ))
            .await?;
        }

        if !m.has_table("deploy_builds").await? {
            let mut stmt = table("deploy_builds");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("application_id"))
                .col(string_len_null("reference", 120))
                .col(string_len("commit_sha", 64))
                .col(text_null("commit_message"))
                // `push`, `manual` ou `rebuild`.
                .col(string_len("trigger", 16))
                .col(string_len_null("triggered_by", 120))
                // O agente que compilou (ou vai compilar).
                .col(big_integer_null("probe_id"))
                .col(string_len("status", 20))
                .col(string_len_null("stage", 16))
                .col(string_len_null("current_image", 128))
                .col(boolean("no_cache").default(false))
                // `X-GitHub-Delivery`: o mesmo aviso não compila duas vezes.
                .col(string_len_null("delivery_id", 64))
                // `[{name, repository, tag, digest, bytes}]` ao terminar.
                .col(json_binary("images"))
                .col(text_null("error"))
                .col(text_null("log"))
                .col(timestamp_with_time_zone_null("started_at"))
                .col(timestamp_with_time_zone_null("finished_at"))
                .col(big_integer_null("duration_ms"))
                .foreign_key(&mut fk(
                    "deploy_builds",
                    "application_id",
                    "deploy_applications",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "deploy_builds",
                    "probe_id",
                    "probes",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(index(
                "deploy_builds_application_index",
                "deploy_builds",
                &["application_id", "id"],
            ))
            .await?;
            m.create_index(index(
                "deploy_builds_status_index",
                "deploy_builds",
                &["status"],
            ))
            .await?;
            m.create_index(index(
                "deploy_builds_delivery_index",
                "deploy_builds",
                &["delivery_id"],
            ))
            .await?;
        }

        if !m.has_table("deploy_runs").await? {
            let mut stmt = table("deploy_runs");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("application_id"))
                .col(big_integer("target_id"))
                .col(string_len("trigger", 16))
                .col(string_len_null("triggered_by", 120))
                .col(string_len_null("reference", 120))
                .col(string_len("commit_sha", 64))
                .col(text_null("commit_message"))
                .col(string_len("image_tag", 128))
                .col(big_integer_null("env_version_id"))
                .col(string_len("status", 20))
                .col(string_len_null("stage", 16))
                // O deploy que este rollback desfaz.
                .col(big_integer_null("rollback_of_run_id"))
                // O deploy anterior do alvo que a sequência (canário) espera.
                .col(big_integer_null("after_run_id"))
                .col(big_integer_null("backup_id"))
                .col(string_len_null("approved_by", 120))
                .col(timestamp_with_time_zone_null("approved_at"))
                .col(timestamp_with_time_zone_null("scheduled_for"))
                .col(text_null("error"))
                .col(text_null("log"))
                .col(timestamp_with_time_zone_null("started_at"))
                .col(timestamp_with_time_zone_null("finished_at"))
                .col(big_integer_null("duration_ms"))
                // A compilação que produziu a imagem (ADR 023).
                .col(big_integer_null("build_id"))
                .foreign_key(&mut fk(
                    "deploy_runs",
                    "build_id",
                    "deploy_builds",
                    ForeignKeyAction::SetNull,
                ))
                .foreign_key(&mut fk(
                    "deploy_runs",
                    "application_id",
                    "deploy_applications",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "deploy_runs",
                    "target_id",
                    "deploy_targets",
                    ForeignKeyAction::Cascade,
                ))
                .foreign_key(&mut fk(
                    "deploy_runs",
                    "env_version_id",
                    "deploy_env_versions",
                    ForeignKeyAction::SetNull,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            // O histórico é sempre "deste alvo, mais recentes primeiro".
            m.create_index(index(
                "deploy_runs_target_index",
                "deploy_runs",
                &["target_id", "id"],
            ))
            .await?;
            // A fila: o despachante procura pelos estados não terminais.
            m.create_index(index(
                "deploy_runs_status_index",
                "deploy_runs",
                &["status"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(drop("deploy_runs")).await?;
        m.drop_table(drop("deploy_builds")).await?;
        m.drop_table(drop("deploy_build_secrets")).await?;
        m.drop_table(drop("deploy_env_versions")).await?;
        m.drop_table(drop("deploy_targets")).await?;
        m.drop_table(drop("deploy_applications")).await?;
        m.drop_table(drop("deploy_github_connections")).await?;
        m.drop_table(drop("deploy_credentials")).await
    }
}
