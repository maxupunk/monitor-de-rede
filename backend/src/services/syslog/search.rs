//! Busca na mensagem, atrás de um trait com três implementações.
//!
//! **`LIKE` e índice de texto são complementares, não substitutos.** Medido no
//! SPIKE-06 com 1 M de linhas e janela de 7 dias:
//!
//! | termo | `LIKE` | FTS5 |
//! |---|---|---|
//! | denso (casa em 12% das linhas) | 567 µs | **450 ms** |
//! | esparso (não casa com nada) | **847 ms** | 164 µs |
//!
//! Os dois perdem em lados opostos, e o motivo é o mesmo nos dois casos: o
//! `LIMIT 51`. Com termo denso, o `LIKE` percorre `received_at` ao contrário e
//! enche a página nas primeiras linhas — sai cedo; o índice de texto, não:
//! ele precisa materializar as 125 mil linhas que casam antes de ordenar. Com
//! termo esparso é o inverso: o `LIKE` só prova a ausência varrendo a janela
//! inteira, enquanto a sondagem do índice responde "vazio" de imediato.
//!
//! Daí a estratégia: **sondar o índice com um teto e decidir**. Até
//! [`DENSITY_LIMIT`] casamentos o filtro é a lista de ids do índice; acima
//! disso o termo é denso e o `LIKE` volta a ser o caminho rápido. Os dois
//! ramos ficam abaixo de 1 ms.
//!
//! **A escolha é feita em tempo de execução**, não em compilação: o índice pode
//! não existir (SQLite sem FTS5, banco anterior à migration, Postgres sem
//! permissão). O `LIKE` é o fundo do poço que sempre funciona.

use async_trait::async_trait;
use sea_orm::{
    sea_query::{Expr, ExprTrait, LikeExpr, SimpleExpr},
    ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement,
};

use crate::models::logs::device_logs;

/// Acima de quantos casamentos o termo é considerado denso.
///
/// Dez mil ids cabem numa cláusula `IN` sem estourar o limite de parâmetros
/// (a lista vai como literal, não como bind) e ainda representam um conjunto
/// que o banco filtra rápido. Acima disso, quem sai na frente é o `LIKE`.
pub const DENSITY_LIMIT: usize = 10_000;

/// Como uma busca textual vira condição SQL.
#[async_trait]
pub trait LogSearch: Send + Sync {
    /// Nome curto, para log e diagnóstico.
    fn name(&self) -> &'static str;

    /// A condição que filtra pela mensagem.
    ///
    /// Recebe a conexão porque a estratégia do índice precisa sondar antes de
    /// decidir — ver a nota do módulo.
    async fn condition(&self, db: &DatabaseConnection, termo: &str) -> SimpleExpr;
}

/// `LIKE '%termo%'` — vale nos dois dialetos, sem índice nenhum.
pub struct LikeSearch;

impl LikeSearch {
    /// `%` e `_` do usuário são escapados: sem isso, procurar `pppoe_client`
    /// casaria com `pppoe-client` e `pppoeXclient`, porque o `_` do SQL vale
    /// por qualquer caractere — e sublinhado é corriqueiro em texto de log.
    #[must_use]
    pub fn expr(termo: &str) -> SimpleExpr {
        let escapado = termo
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        Expr::col(device_logs::Column::Message)
            .like(LikeExpr::new(format!("%{escapado}%")).escape('\\'))
    }
}

#[async_trait]
impl LogSearch for LikeSearch {
    fn name(&self) -> &'static str {
        "like"
    }

    async fn condition(&self, _db: &DatabaseConnection, termo: &str) -> SimpleExpr {
        Self::expr(termo)
    }
}

/// FTS5 de conteúdo externo (SQLite), com desvio para `LIKE` em termo denso.
pub struct Fts5Search;

#[async_trait]
impl LogSearch for Fts5Search {
    fn name(&self) -> &'static str {
        "fts5"
    }

    async fn condition(&self, db: &DatabaseConnection, termo: &str) -> SimpleExpr {
        let sql = format!(
            "SELECT rowid FROM device_logs_fts WHERE device_logs_fts MATCH {} LIMIT {}",
            literal(&frase_fts5(termo)),
            DENSITY_LIMIT + 1
        );
        match sonda_ids(db, DatabaseBackend::Sqlite, &sql).await {
            Some(ids) if ids.len() <= DENSITY_LIMIT => id_in(ids),
            // Denso, ou o índice recusou a consulta: o `LIKE` cobre os dois.
            _ => LikeSearch::expr(termo),
        }
    }
}

/// `tsvector` + índice GIN (PostgreSQL), com a mesma decisão por densidade.
pub struct TsVectorSearch;

#[async_trait]
impl LogSearch for TsVectorSearch {
    fn name(&self) -> &'static str {
        "tsvector"
    }

    async fn condition(&self, db: &DatabaseConnection, termo: &str) -> SimpleExpr {
        // `simple`, e não `portuguese`: a configuração de idioma aplica
        // *stemming*, e mensagem de roteador é vocabulário técnico em inglês
        // misturado com identificadores. Reduzir `ether1` ao radical de alguma
        // língua só produziria casamento errado.
        let sql = format!(
            "SELECT id FROM device_logs \
             WHERE to_tsvector('simple', message) @@ plainto_tsquery('simple', {}) LIMIT {}",
            literal(termo),
            DENSITY_LIMIT + 1
        );
        match sonda_ids(db, DatabaseBackend::Postgres, &sql).await {
            Some(ids) if ids.len() <= DENSITY_LIMIT => id_in(ids),
            _ => LikeSearch::expr(termo),
        }
    }
}

/// Lista de ids como condição. Vazia vira `1 = 0` — sem isso, um `IN ()` seria
/// SQL inválido em vez de "nenhum resultado".
fn id_in(ids: Vec<i64>) -> SimpleExpr {
    if ids.is_empty() {
        return Expr::val(1).eq(0);
    }
    Expr::col(device_logs::Column::Id).is_in(ids)
}

/// Sonda o índice. `None` quando a consulta falha — o chamador cai no `LIKE`.
async fn sonda_ids(
    db: &DatabaseConnection,
    backend: DatabaseBackend,
    sql: &str,
) -> Option<Vec<i64>> {
    let linhas = db
        .query_all_raw(Statement::from_string(backend, sql.to_owned()))
        .await
        .ok()?;
    Some(
        linhas
            .iter()
            .filter_map(|linha| {
                linha
                    .try_get_by_index::<i64>(0)
                    .ok()
                    .or_else(|| linha.try_get_by_index::<i32>(0).ok().map(i64::from))
            })
            .collect(),
    )
}

/// Literal SQL com aspas simples escapadas.
///
/// A sondagem é montada como texto porque o `MATCH` do FTS5 não aceita
/// parâmetro em toda posição. O termo vem do usuário, então o escape aqui é o
/// que separa uma busca de uma injeção.
fn literal(valor: &str) -> String {
    format!("'{}'", valor.replace('\'', "''"))
}

/// Transforma o termo do usuário numa consulta FTS5.
///
/// Duas decisões embutidas:
///
/// **Aspas.** A sintaxe do `MATCH` tem operadores (`AND`, `OR`, `NOT`, `*`,
/// `:`, `-`) e um termo cru pode virar consulta inválida. Entre aspas, tudo
/// vira frase literal; aspas internas são dobradas, que é o escape da
/// linguagem.
///
/// **`*` no fim.** O índice casa por **token**, não por substring: sem o
/// prefixo, procurar `pppo` não acharia `pppoe`, e quem digita meia palavra
/// concluiria que não há nada.
#[must_use]
pub fn frase_fts5(termo: &str) -> String {
    format!("\"{}\"*", termo.replace('"', "\"\""))
}

/// Escolhe a melhor busca disponível para esta conexão.
///
/// Sonda o catálogo em vez de confiar na migration: o índice pode ter falhado
/// silenciosamente na criação (SQLite sem FTS5), e nesse caso usar a consulta
/// otimizada produziria erro de SQL em toda busca.
pub async fn select(db: &DatabaseConnection) -> Box<dyn LogSearch> {
    match db.get_database_backend() {
        DatabaseBackend::Sqlite if tem_tabela_fts(db).await => Box::new(Fts5Search),
        DatabaseBackend::Postgres if tem_indice_gin(db).await => Box::new(TsVectorSearch),
        _ => Box::new(LikeSearch),
    }
}

async fn tem_tabela_fts(db: &DatabaseConnection) -> bool {
    consulta_existe(
        db,
        DatabaseBackend::Sqlite,
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'device_logs_fts'",
    )
    .await
}

async fn tem_indice_gin(db: &DatabaseConnection) -> bool {
    consulta_existe(
        db,
        DatabaseBackend::Postgres,
        "SELECT indexname FROM pg_indexes WHERE indexname = 'device_logs_message_fts_index'",
    )
    .await
}

async fn consulta_existe(db: &DatabaseConnection, backend: DatabaseBackend, sql: &str) -> bool {
    db.query_one_raw(Statement::from_string(backend, sql))
        .await
        .is_ok_and(|linha| linha.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{models::logs::device_logs, services::syslog::db};
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, QueryFilter};
    use serial_test::serial;

    async fn banco_com(mensagens: &[&str]) -> DatabaseConnection {
        std::env::remove_var("SYSLOG_DB_URL");
        let db = db::connect("sqlite::memory:")
            .await
            .expect("banco")
            .connection()
            .clone();
        for mensagem in mensagens {
            device_logs::ActiveModel {
                source_ip: Set("10.0.0.1".into()),
                received_at: Set(chrono::Utc::now().into()),
                message: Set((*mensagem).to_owned()),
                ..Default::default()
            }
            .insert(&db)
            .await
            .expect("insere");
        }
        db
    }

    async fn quantas(db: &DatabaseConnection, termo: &str) -> usize {
        let condicao = select(db).await.condition(db, termo).await;
        device_logs::Entity::find()
            .filter(condicao)
            .all(db)
            .await
            .expect("busca")
            .len()
    }

    #[test]
    fn a_frase_do_fts5_neutraliza_a_sintaxe_de_consulta() {
        // Sem as aspas, `NOT` e `*` seriam operadores e `a: b` seria erro de
        // sintaxe — a busca falharia com erro de SQL em vez de não achar nada.
        assert_eq!(frase_fts5("erro"), "\"erro\"*");
        assert_eq!(frase_fts5("login NOT admin"), "\"login NOT admin\"*");
        assert_eq!(frase_fts5("campo: valor"), "\"campo: valor\"*");
        assert_eq!(frase_fts5("diz \"ok\""), "\"diz \"\"ok\"\"\"*");
    }

    #[test]
    fn o_literal_escapa_a_aspa_simples() {
        // É o que separa uma busca de uma injeção: a sondagem é montada como
        // texto porque o `MATCH` não aceita parâmetro em toda posição.
        assert_eq!(literal("erro"), "'erro'");
        assert_eq!(literal("o'brien"), "'o''brien'");
        assert_eq!(
            literal("'; DROP TABLE device_logs; --"),
            "'''; DROP TABLE device_logs; --'"
        );
    }

    #[tokio::test]
    #[serial]
    async fn o_sqlite_migrado_escolhe_fts5() {
        let db = banco_com(&[]).await;
        assert_eq!(select(&db).await.name(), "fts5");
    }

    #[tokio::test]
    #[serial]
    async fn sem_o_indice_a_busca_cai_no_like() {
        // Banco sem a migration do índice: a escolha degrada sozinha, senão
        // toda busca viraria erro de SQL.
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco cru");
        assert_eq!(select(&db).await.name(), "like");
    }

    #[tokio::test]
    #[serial]
    async fn o_indice_casa_por_token_e_por_prefixo() {
        let db = banco_com(&[
            "interface ether1 link down",
            "pppoe-client desconectado",
            "login failure for admin",
        ])
        .await;

        assert_eq!(quantas(&db, "ether1").await, 1, "token inteiro");
        assert_eq!(quantas(&db, "ethe").await, 1, "prefixo");
        assert_eq!(quantas(&db, "pppoe").await, 1);
        assert_eq!(quantas(&db, "login failure").await, 1, "frase");
        // O caso que custava 847 ms com `LIKE` numa janela larga.
        assert_eq!(quantas(&db, "inexistente").await, 0);
    }

    #[tokio::test]
    #[serial]
    async fn a_injecao_pelo_termo_de_busca_nao_passa() {
        let db = banco_com(&["linha viva"]).await;
        // Se o escape falhasse, a tabela sumiria e a asserção seguinte
        // explodiria em vez de contar zero.
        assert_eq!(quantas(&db, "'; DROP TABLE device_logs; --").await, 0);
        assert_eq!(quantas(&db, "viva").await, 1, "a tabela continua de pé");
    }

    #[tokio::test]
    #[serial]
    async fn termo_denso_volta_para_o_like_sem_mudar_o_resultado() {
        // Acima do teto de densidade a estratégia troca. O que o usuário vê
        // não pode mudar por causa disso — só a velocidade.
        let mensagens: Vec<String> = (0..(DENSITY_LIMIT + 50))
            .map(|indice| format!("recorrente {indice}"))
            .collect();
        let refs: Vec<&str> = mensagens.iter().map(String::as_str).collect();
        let db = banco_com(&refs).await;

        let total = quantas(&db, "recorrente").await;
        assert_eq!(
            total,
            DENSITY_LIMIT + 50,
            "o desvio para o LIKE perdeu linhas"
        );
    }
}
