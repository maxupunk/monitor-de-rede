//! Registry de imagens (ADR 022).
//!
//! * `registry_repositories` — `aplicacao/imagem` ou um namespace livre;
//! * `registry_blobs` — o conteúdo, uma vez só por digest (o arquivo mora no
//!   `BlobStore`); `registry_repository_blobs` diz quais repositórios podem
//!   ler cada blob — conhecer um digest não dá acesso a ele;
//! * `registry_manifests` — o manifest (o texto exato, cujo digest é a
//!   identidade), a origem e o resultado da varredura; `registry_tags` aponta
//!   nomes para manifests;
//! * `registry_uploads` — envios em andamento (o arquivo parcial no storage);
//! * `registry_tokens` — tokens de acesso pessoal (só o hash);
//! * `registry_findings`, `registry_advisories`, `registry_feeds`,
//!   `registry_exceptions` — a varredura: achados por manifest, a cópia local
//!   do OSV.dev por ecossistema e as exceções aceitas.

use sea_orm_migration::prelude::*;

use netmonitor_core::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    #[allow(clippy::too_many_lines)]
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if !m.has_table("registry_repositories").await? {
            let mut stmt = table("registry_repositories");
            stmt.col(big_pk_auto("id"))
                .col(string_len("name", 255))
                // A aplicação de deploy dona do namespace (sem FK: apagar a
                // aplicação não apaga imagens; a retenção cuida delas).
                .col(big_integer_null("application_id"));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "registry_repositories_name_unique",
                "registry_repositories",
                &["name"],
            ))
            .await?;
        }

        if !m.has_table("registry_blobs").await? {
            let mut stmt = table("registry_blobs");
            stmt.col(big_pk_auto("id"))
                .col(string_len("digest", 80))
                .col(big_integer("size"));
            m.create_table(append_only(stmt.take())).await?;
            m.create_index(unique(
                "registry_blobs_digest_unique",
                "registry_blobs",
                &["digest"],
            ))
            .await?;
        }

        if !m.has_table("registry_repository_blobs").await? {
            let mut stmt = table("registry_repository_blobs");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("repository_id"))
                .col(string_len("digest", 80))
                .foreign_key(&mut fk(
                    "registry_repository_blobs",
                    "repository_id",
                    "registry_repositories",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(append_only(stmt.take())).await?;
            m.create_index(unique(
                "registry_repository_blobs_unique",
                "registry_repository_blobs",
                &["repository_id", "digest"],
            ))
            .await?;
            m.create_index(index(
                "registry_repository_blobs_digest_index",
                "registry_repository_blobs",
                &["digest"],
            ))
            .await?;
        }

        if !m.has_table("registry_manifests").await? {
            let mut stmt = table("registry_manifests");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("repository_id"))
                .col(string_len("digest", 80))
                .col(string_len("media_type", 120))
                .col(string_len_null("artifact_type", 120))
                // O manifest que este descreve (attestation, assinatura).
                .col(string_len_null("subject_digest", 80))
                // O texto exato: o digest é dele.
                .col(text("content"))
                .col(big_integer("size"))
                // Manifest + config + camadas (sem deduplicar entre versões).
                .col(big_integer("total_size"))
                // `linux/amd64`, quando é a imagem de uma plataforma.
                .col(string_len_null("platform", 64))
                // De onde veio: a compilação da central ou o envio.
                .col(json_binary_null("provenance"))
                .col(string_len_null("pushed_by", 120))
                // Fixada à mão: a retenção não a apaga.
                .col(boolean("pinned").default(false))
                .col(integer_null("sbom_packages"))
                // `pending`, `done`, `no_sbom`, `failed`.
                .col(string_len_null("scan_status", 16))
                // Contagem por severidade.
                .col(json_binary_null("scan"))
                .col(timestamp_with_time_zone_null("scanned_at"))
                .foreign_key(&mut fk(
                    "registry_manifests",
                    "repository_id",
                    "registry_repositories",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "registry_manifests_repository_digest_unique",
                "registry_manifests",
                &["repository_id", "digest"],
            ))
            .await?;
            m.create_index(index(
                "registry_manifests_subject_index",
                "registry_manifests",
                &["repository_id", "subject_digest"],
            ))
            .await?;
        }

        if !m.has_table("registry_tags").await? {
            let mut stmt = table("registry_tags");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("repository_id"))
                .col(string_len("name", 128))
                .col(string_len("manifest_digest", 80))
                .foreign_key(&mut fk(
                    "registry_tags",
                    "repository_id",
                    "registry_repositories",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "registry_tags_repository_name_unique",
                "registry_tags",
                &["repository_id", "name"],
            ))
            .await?;
        }

        if !m.has_table("registry_uploads").await? {
            let mut stmt = table("registry_uploads");
            stmt.col(string_len("id", 36).primary_key())
                .col(big_integer("repository_id"))
                .col(big_integer("offset"))
                .foreign_key(&mut fk(
                    "registry_uploads",
                    "repository_id",
                    "registry_repositories",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
        }

        if !m.has_table("registry_tokens").await? {
            let mut stmt = table("registry_tokens");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("user_id"))
                .col(string_len("name", 120))
                .col(string_len("token_hash", 64))
                // Os últimos caracteres, para reconhecer o token na lista.
                .col(string_len("hint", 8))
                // `{repositories: [...], actions: [...]}`.
                .col(json_binary("scopes"))
                .col(timestamp_with_time_zone("expires_at"))
                .col(timestamp_with_time_zone_null("last_used_at"))
                .col(string_len_null("last_used_ip", 64))
                .col(timestamp_with_time_zone_null("revoked_at"))
                .foreign_key(&mut fk(
                    "registry_tokens",
                    "user_id",
                    "users",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "registry_tokens_hash_unique",
                "registry_tokens",
                &["token_hash"],
            ))
            .await?;
        }

        if !m.has_table("registry_findings").await? {
            let mut stmt = table("registry_findings");
            stmt.col(big_pk_auto("id"))
                .col(big_integer("manifest_id"))
                .col(string_len("advisory", 64))
                .col(json_binary("aliases"))
                .col(string_len("package", 255))
                .col(string_len("ecosystem", 64))
                .col(string_len("installed", 128))
                .col(string_len_null("fixed", 128))
                // `critical`, `high`, `medium`, `low` ou `unknown`.
                .col(string_len("severity", 16))
                .col(text_null("summary"))
                .foreign_key(&mut fk(
                    "registry_findings",
                    "manifest_id",
                    "registry_manifests",
                    ForeignKeyAction::Cascade,
                ));
            m.create_table(append_only(stmt.take())).await?;
            m.create_index(index(
                "registry_findings_manifest_index",
                "registry_findings",
                &["manifest_id"],
            ))
            .await?;
        }

        if !m.has_table("registry_advisories").await? {
            let mut stmt = table("registry_advisories");
            stmt.col(big_pk_auto("id"))
                .col(string_len("ecosystem", 64))
                .col(string_len("package", 255))
                .col(string_len("advisory", 64))
                .col(timestamp_with_time_zone("modified"))
                // O registro OSV inteiro (faixas, severidade, resumo).
                .col(json_binary("data"));
            m.create_table(append_only(stmt.take())).await?;
            m.create_index(unique(
                "registry_advisories_unique",
                "registry_advisories",
                &["ecosystem", "package", "advisory"],
            ))
            .await?;
        }

        if !m.has_table("registry_feeds").await? {
            let mut stmt = table("registry_feeds");
            stmt.col(big_pk_auto("id"))
                .col(string_len("ecosystem", 64))
                .col(timestamp_with_time_zone_null("synced_at"))
                // O `modified` mais novo já importado.
                .col(timestamp_with_time_zone_null("cursor"))
                .col(big_integer("entries").default(0))
                .col(text_null("error"));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(unique(
                "registry_feeds_ecosystem_unique",
                "registry_feeds",
                &["ecosystem"],
            ))
            .await?;
        }

        if !m.has_table("registry_exceptions").await? {
            let mut stmt = table("registry_exceptions");
            stmt.col(big_pk_auto("id"))
                // Nula: vale para todas as aplicações.
                .col(big_integer_null("application_id"))
                .col(string_len("advisory", 64))
                .col(text("reason"))
                .col(timestamp_with_time_zone("expires_at"))
                .col(string_len("created_by", 120));
            m.create_table(with_timestamps(stmt.take())).await?;
            m.create_index(index(
                "registry_exceptions_advisory_index",
                "registry_exceptions",
                &["advisory"],
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for name in [
            "registry_exceptions",
            "registry_feeds",
            "registry_advisories",
            "registry_findings",
            "registry_tokens",
            "registry_uploads",
            "registry_tags",
            "registry_manifests",
            "registry_repository_blobs",
            "registry_blobs",
            "registry_repositories",
        ] {
            m.drop_table(drop(name)).await?;
        }
        Ok(())
    }
}
