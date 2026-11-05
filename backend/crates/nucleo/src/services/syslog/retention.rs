//! Retenção do banco de logs: **por dias e por tamanho, o que vencer primeiro**.
//!
//! O corte por tempo é o que o usuário configura; o corte por tamanho é o que
//! salva o volume quando alguém liga o tópico `debug` num roteador e a taxa
//! decuplica da noite para o dia. Um sem o outro deixa um flanco aberto.
//!
//! **`DELETE` não devolve disco.** No SQLite as páginas liberadas voltam para a
//! lista livre do arquivo, que continua do mesmo tamanho; e o WAL cresce até um
//! *checkpoint*. Por isso a purga termina com `incremental_vacuum` e
//! `wal_checkpoint(TRUNCATE)` — sem esses dois, a retenção "funciona" e o disco
//! enche do mesmo jeito.

use chrono::{Duration, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};

use crate::{
    models::logs::device_logs,
    services::{maintenance::data_pruner::retention_days, shared::errors::AppResult},
};

pub const DEFAULT_RETENTION_LOGS_DAYS: i64 = 7;

/// Teto de disco do banco de logs.
///
/// Medido no SPIKE-06: 290 B por linha com os três índices. A 12 msg/s isso dá
/// ~301 MB/dia e ~2,1 GB em 7 dias — 2 GB ficaria no limite exato, e a amostra
/// medida foi a mensagem curta de login. Linha de firewall passa de 100
/// caracteres, então o teto folgado é o que faz os 7 dias valerem de verdade.
pub const DEFAULT_RETENTION_LOGS_MAX_MB: u64 = 4096;

/// Linhas apagadas por rodada no corte por tamanho.
///
/// Blocos, e não um `DELETE` só: apagar milhões de linhas numa transação segura
/// o *write lock* do arquivo por segundos — exatamente o problema que motivou
/// separar este banco do principal. Repetir o problema aqui dentro seria
/// trocar de vítima.
const CHUNK: u64 = 10_000;

/// Teto de rodadas por execução.
///
/// A purga roda de hora em hora; deixar o banco 100 MB acima do teto até a
/// próxima passada é melhor do que segurar o escritor num laço que não termina.
const MAX_CHUNKS: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LogPruneStats {
    pub by_age: u64,
    pub by_size: u64,
    pub bytes_after: u64,
}

impl LogPruneStats {
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.by_age + self.by_size
    }
}

/// Apaga o que passou da janela e o que não cabe no teto de disco.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn prune(db: &DatabaseConnection) -> AppResult<LogPruneStats> {
    prune_with(db, teto_bytes(), CHUNK).await
}

/// A purga com teto e bloco injetados — é assim que o teste exercita a ordem do
/// corte sem depender do tamanho de página do sistema de arquivos.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn prune_with(
    db: &DatabaseConnection,
    teto: u64,
    chunk: u64,
) -> AppResult<LogPruneStats> {
    let dias = retention_days("RETENTION_LOGS_DAYS", DEFAULT_RETENTION_LOGS_DAYS);
    let corte = Utc::now() - Duration::days(dias);

    let by_age = device_logs::Entity::delete_many()
        .filter(device_logs::Column::ReceivedAt.lt(corte))
        .exec(db)
        .await?
        .rows_affected;
    if by_age > 0 {
        devolve_disco(db).await;
    }

    let by_size = corta_por_tamanho(db, teto, chunk).await?;

    Ok(LogPruneStats {
        by_age,
        by_size,
        bytes_after: tamanho_bytes(db).await.unwrap_or(0),
    })
}

/// Teto em bytes, lido do ambiente com o mesmo critério do `data_pruner`:
/// valor ausente, inválido ou zerado cai no padrão.
fn teto_bytes() -> u64 {
    let megabytes = std::env::var("RETENTION_LOGS_MAX_MB")
        .ok()
        .and_then(|valor| valor.trim().parse::<u64>().ok())
        .filter(|valor| *valor > 0)
        .unwrap_or(DEFAULT_RETENTION_LOGS_MAX_MB);
    megabytes * 1024 * 1024
}

/// Apaga as linhas mais antigas, em blocos, até o banco caber no teto.
async fn corta_por_tamanho(db: &DatabaseConnection, teto: u64, chunk: u64) -> AppResult<u64> {
    let mut apagadas = 0;
    for _ in 0..MAX_CHUNKS {
        let Some(tamanho) = tamanho_bytes(db).await else {
            // Backend sem forma conhecida de medir: o corte por tempo já
            // limita o crescimento, e chutar aqui apagaria log bom.
            return Ok(apagadas);
        };
        if tamanho <= teto {
            return Ok(apagadas);
        }

        // O `DELETE ... LIMIT` não vale nos dois dialetos: no PostgreSQL não
        // existe, e no SQLite depende de `SQLITE_ENABLE_UPDATE_DELETE_LIMIT`.
        // Selecionar os ids do bloco e apagar por `IN` vale em qualquer um, e o
        // índice `device_logs_received_at_index` cobre a seleção.
        let ids: Vec<i64> = device_logs::Entity::find()
            .select_only()
            .column(device_logs::Column::Id)
            .order_by_asc(device_logs::Column::ReceivedAt)
            .limit(chunk)
            .into_tuple()
            .all(db)
            .await?;
        if ids.is_empty() {
            return Ok(apagadas);
        }

        apagadas += device_logs::Entity::delete_many()
            .filter(device_logs::Column::Id.is_in(ids))
            .exec(db)
            .await?
            .rows_affected;

        // Sem devolver as páginas, a medição da próxima volta não mudaria e o
        // laço giraria até o teto de rodadas apagando o banco inteiro.
        devolve_disco(db).await;
    }
    tracing::warn!(
        apagadas,
        "teto de rodadas da purga por tamanho atingido; o resto fica para a próxima passada"
    );
    Ok(apagadas)
}

/// Tamanho ocupado, em bytes. `None` quando o backend não sabe responder.
async fn tamanho_bytes(db: &DatabaseConnection) -> Option<u64> {
    let backend = db.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Sqlite => {
            "SELECT (page_count * page_size) AS bytes \
             FROM pragma_page_count(), pragma_page_size()"
        }
        DatabaseBackend::Postgres => {
            "SELECT pg_total_relation_size('device_logs')::bigint AS bytes"
        }
        _ => return None,
    };
    let linha = db
        .query_one_raw(Statement::from_string(backend, sql))
        .await
        .ok()??;
    linha
        .try_get::<i64>("", "bytes")
        .ok()
        .and_then(|valor| u64::try_from(valor).ok())
}

/// Devolve ao sistema de arquivos o que o `DELETE` só marcou como livre.
///
/// Só faz sentido no SQLite: o `autovacuum` do PostgreSQL já cuida disso, e
/// `VACUUM FULL` ali travaria a tabela. Falha não é fatal — o disco continua
/// ocupado até a próxima passada, que é bem melhor do que abortar a purga.
///
/// **`execute_unprepared`, e não `query_one_raw`.** O `incremental_vacuum`
/// devolve uma página por passo do statement, e o `query_one_raw` do SeaORM usa
/// `fetch_optional`: ele para no primeiro passo. Medido — com 563 páginas
/// livres, o `query_one_raw` devolvia **uma**, e o `page_count` caía de 572
/// para 571. O `PRAGMA` retornava `Ok`, o log não acusava nada, e o disco não
/// voltava. É exatamente a falha silenciosa que esta função existe para evitar.
async fn devolve_disco(db: &DatabaseConnection) {
    if db.get_database_backend() != DatabaseBackend::Sqlite {
        return;
    }
    for pragma in [
        // Exige `auto_vacuum = INCREMENTAL`, ligado na criação do arquivo
        // (`db::connect`) — depois de a primeira tabela nascer, já não dá.
        "PRAGMA incremental_vacuum;",
        "PRAGMA wal_checkpoint(TRUNCATE);",
    ] {
        if let Err(error) = db.execute_unprepared(pragma).await {
            tracing::debug!(%error, pragma, "não foi possível devolver disco do banco de logs");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::syslog::db;
    use sea_orm::{ActiveValue::Set, PaginatorTrait};
    use serial_test::serial;

    async fn banco() -> DatabaseConnection {
        std::env::remove_var("SYSLOG_DB_URL");
        db::connect("sqlite::memory:")
            .await
            .expect("banco de logs")
            .connection()
            .clone()
    }

    async fn semeia(db: &DatabaseConnection, quantas: usize, dias_atras: i64) {
        let instante = Utc::now() - Duration::days(dias_atras);
        let linhas: Vec<device_logs::ActiveModel> = (0..quantas)
            .map(|indice| device_logs::ActiveModel {
                device_id: Set(Some(1)),
                source_ip: Set("192.168.88.1".into()),
                received_at: Set(instante.into()),
                message: Set(format!("linha {indice} {}", "x".repeat(120))),
                ..Default::default()
            })
            .collect();
        for bloco in linhas.chunks(500) {
            device_logs::Entity::insert_many(bloco.to_vec())
                .exec(db)
                .await
                .expect("semeadura");
        }
    }

    #[tokio::test]
    #[serial]
    async fn o_corte_por_idade_apaga_so_o_que_passou_da_janela() {
        std::env::set_var("RETENTION_LOGS_DAYS", "7");
        std::env::remove_var("RETENTION_LOGS_MAX_MB");
        let db = banco().await;
        semeia(&db, 10, 30).await; // velhas
        semeia(&db, 5, 1).await; // recentes

        let stats = prune(&db).await.expect("purga");

        assert_eq!(stats.by_age, 10);
        assert_eq!(stats.by_size, 0);
        let restantes = device_logs::Entity::find()
            .count(&db)
            .await
            .expect("contagem");
        assert_eq!(restantes, 5, "log dentro da janela não pode sumir");
        std::env::remove_var("RETENTION_LOGS_DAYS");
    }

    #[tokio::test]
    #[serial]
    async fn o_teto_de_tamanho_vence_quando_chega_antes() {
        // Janela longa: nada sairia por idade. Teto apertado: o tamanho corta.
        std::env::set_var("RETENTION_LOGS_DAYS", "3650");
        let db = banco().await;
        semeia(&db, 4_000, 1).await;

        // O teto é derivado do tamanho real em vez de cravado: página de banco
        // varia com o sistema, e um número fixo tornaria o teste frágil onde
        // ele não deveria ser.
        let cheio = tamanho_bytes(&db).await.expect("tamanho");
        let teto = cheio / 2;

        let stats = prune_with(&db, teto, 500).await.expect("purga");

        assert_eq!(stats.by_age, 0, "nada venceu por idade");
        assert!(stats.by_size > 0, "o teto de disco tinha de cortar");
        assert!(
            stats.bytes_after <= teto,
            "sobrou {} bytes acima do teto de {teto}",
            stats.bytes_after
        );
        std::env::remove_var("RETENTION_LOGS_DAYS");
    }

    #[tokio::test]
    #[serial]
    async fn o_corte_por_tamanho_apaga_do_mais_antigo_para_o_mais_novo() {
        std::env::set_var("RETENTION_LOGS_DAYS", "3650");
        let db = banco().await;
        semeia(&db, 3_000, 10).await; // antigas
        semeia(&db, 3_000, 1).await; // recentes

        // Teto em 70% do cheio: cabe folgadamente o bloco recente inteiro, e
        // sobra corte a fazer no antigo. Se o corte fosse pelo fim, ou sem
        // ordem, o bloco recente perderia linha.
        let cheio = tamanho_bytes(&db).await.expect("tamanho");
        let stats = prune_with(&db, cheio * 7 / 10, 500).await.expect("purga");
        assert!(stats.by_size > 0);

        let meio = Utc::now() - Duration::days(5);
        let antigas = device_logs::Entity::find()
            .filter(device_logs::Column::ReceivedAt.lt(meio))
            .count(&db)
            .await
            .expect("contagem");
        let recentes = device_logs::Entity::find()
            .filter(device_logs::Column::ReceivedAt.gte(meio))
            .count(&db)
            .await
            .expect("contagem");

        assert_eq!(recentes, 3_000, "o corte não pode tocar no que é recente");
        assert!(
            antigas < 3_000,
            "o corte tem de comer pelo começo (sobraram {antigas} antigas)"
        );
        std::env::remove_var("RETENTION_LOGS_DAYS");
    }

    #[tokio::test]
    #[serial]
    async fn banco_dentro_do_teto_nao_perde_nada() {
        std::env::set_var("RETENTION_LOGS_DAYS", "3650");
        std::env::set_var("RETENTION_LOGS_MAX_MB", "4096");
        let db = banco().await;
        semeia(&db, 100, 1).await;

        let stats = prune(&db).await.expect("purga");

        assert_eq!(stats.total(), 0);
        assert_eq!(
            device_logs::Entity::find()
                .count(&db)
                .await
                .expect("contagem"),
            100
        );
        std::env::remove_var("RETENTION_LOGS_DAYS");
        std::env::remove_var("RETENTION_LOGS_MAX_MB");
    }

    #[test]
    #[serial]
    fn o_teto_invalido_cai_no_padrao_em_vez_de_desligar_a_purga() {
        for invalido in ["0", "-1", "sempre", ""] {
            std::env::set_var("RETENTION_LOGS_MAX_MB", invalido);
            assert_eq!(
                teto_bytes(),
                DEFAULT_RETENTION_LOGS_MAX_MB * 1024 * 1024,
                "aceitou {invalido:?}"
            );
        }
        std::env::remove_var("RETENTION_LOGS_MAX_MB");
    }
}
