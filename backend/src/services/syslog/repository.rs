//! Consulta paginada do banco de logs.
//!
//! **Paginação por cursor de keyset, não por `OFFSET`.** A tela é uma rolagem
//! infinita sobre milhões de linhas: `OFFSET 50000` faz o banco contar 50 000
//! linhas antes de devolver a primeira, e o custo cresce com a rolagem. Pior,
//! `OFFSET` sobre uma tabela que recebe inserção o tempo todo repete e pula
//! linhas — cada nova mensagem desloca a janela inteira. O keyset sobre
//! `(received_at, id)` não sofre de nenhum dos dois.
//!
//! **A janela de tempo é obrigatória.** `LIKE '%termo%'` não usa índice: em
//! 7 M de linhas é varredura de ~1,5 GB, com o banco travado para o escritor
//! durante a leitura longa. Com `from` valendo 24 h por padrão e a janela com
//! teto, o índice `device_logs_received_at_index` limita a varredura antes de o
//! `LIKE` entrar — e a busca full-text da Fase 5 vira otimização de conforto,
//! não resgate de uma tela quebrada.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    sea_query::{Expr, ExprTrait, LikeExpr},
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};

use crate::{
    models::logs::device_logs,
    services::shared::errors::{AppError, AppResult},
};

/// Janela usada quando o cliente não manda `from`.
pub const DEFAULT_WINDOW_HOURS: i64 = 24;

/// Maior janela aceita. Além disso o `LIKE` deixa de ser viável e a resposta
/// deixaria de caber num tempo de tela.
pub const MAX_WINDOW_DAYS: i64 = 7;

/// Linhas por página quando o cliente não pede.
pub const DEFAULT_LIMIT: u64 = 50;

/// Teto por página. Protege o banco de um `?limit=100000`.
pub const MAX_LIMIT: u64 = 200;

/// Posição na ordenação `(received_at DESC, id DESC)`.
///
/// `id` desempata: duas mensagens do mesmo milissegundo — comuns numa rajada —
/// ficariam ambíguas só com o horário, e a página seguinte pularia ou repetiria
/// linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub received_at: DateTime<Utc>,
    pub id: i64,
}

impl Cursor {
    /// Serializa como base64 opaco.
    ///
    /// Opaco de propósito: o cliente não deve montar cursor à mão, e o formato
    /// interno pode mudar quando a Fase 5 trocar a ordenação sem quebrar
    /// contrato.
    #[must_use]
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(format!(
            "{}:{}",
            self.received_at.timestamp_micros(),
            self.id
        ))
    }

    /// # Errors
    ///
    /// Cursor ilegível vira 422 com mensagem em português — o cliente mandou
    /// algo que não saiu daqui.
    pub fn decode(texto: &str) -> AppResult<Self> {
        let erro = || AppError::Validation("Cursor de paginação inválido.".into());
        let bytes = URL_SAFE_NO_PAD.decode(texto).map_err(|_| erro())?;
        let texto = String::from_utf8(bytes).map_err(|_| erro())?;
        let (micros, id) = texto.split_once(':').ok_or_else(erro)?;
        let micros: i64 = micros.parse().map_err(|_| erro())?;
        let id: i64 = id.parse().map_err(|_| erro())?;
        Ok(Self {
            received_at: DateTime::from_timestamp_micros(micros).ok_or_else(erro)?,
            id,
        })
    }
}

/// Os filtros como o cliente os mandou, antes de normalizar.
///
/// Struct em vez de nove parâmetros soltos: a lista cresce a cada filtro novo,
/// e `Option<i16>` três vezes seguidas é convite a trocar `severity` por
/// `facility` na chamada sem o compilador notar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogFilters {
    pub device_id: Option<i64>,
    pub severity: Option<i16>,
    pub facility: Option<i16>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub q: Option<String>,
    pub cursor: Option<Cursor>,
    pub limit: Option<u64>,
}

/// Os filtros da tela, já validados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogQuery {
    pub device_id: Option<i64>,
    /// Severidade **numérica máxima**: `3` traz erro, crítico, alerta e
    /// emergência. No syslog o número menor é o mais grave, então "erro e
    /// acima" é `<= 3`.
    pub severity: Option<i16>,
    pub facility: Option<i16>,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub q: Option<String>,
    pub cursor: Option<Cursor>,
    pub limit: u64,
}

impl LogQuery {
    /// Normaliza a janela e o limite pedidos pelo cliente.
    ///
    /// Silenciosamente aperta o que passar do teto em vez de recusar: uma tela
    /// que pede 30 dias deve ver 7, não um erro.
    #[must_use]
    pub fn normalize(filtros: LogFilters, agora: DateTime<Utc>) -> Self {
        let to = filtros.to.unwrap_or(agora);
        let from = filtros
            .from
            .unwrap_or_else(|| to - Duration::hours(DEFAULT_WINDOW_HOURS));
        // Intervalo invertido é erro de digitação na tela, não pedido de zero
        // resultados: troca a ordem em vez de devolver lista vazia sem explicar.
        let (from, to) = if from > to { (to, from) } else { (from, to) };
        let teto = to - Duration::days(MAX_WINDOW_DAYS);
        let from = if from < teto { teto } else { from };

        Self {
            device_id: filtros.device_id,
            severity: filtros.severity,
            facility: filtros.facility,
            from,
            to,
            q: filtros
                .q
                .map(|texto| texto.trim().to_owned())
                .filter(|texto| !texto.is_empty()),
            cursor: filtros.cursor,
            limit: filtros.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
        }
    }
}

/// Uma página de logs e o cursor da seguinte.
#[derive(Debug, Clone)]
pub struct LogPage {
    pub rows: Vec<device_logs::Model>,
    pub next_cursor: Option<Cursor>,
}

/// Busca uma página.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn search(db: &DatabaseConnection, query: &LogQuery) -> AppResult<LogPage> {
    let mut condicao = Condition::all()
        .add(device_logs::Column::ReceivedAt.gte(query.from))
        .add(device_logs::Column::ReceivedAt.lte(query.to));

    if let Some(device_id) = query.device_id {
        condicao = condicao.add(device_logs::Column::DeviceId.eq(device_id));
    }
    if let Some(severity) = query.severity {
        condicao = condicao.add(device_logs::Column::Severity.lte(severity));
    }
    if let Some(facility) = query.facility {
        condicao = condicao.add(device_logs::Column::Facility.eq(facility));
    }
    if let Some(termo) = &query.q {
        condicao = condicao.add(busca_textual(termo));
    }
    if let Some(cursor) = query.cursor {
        // A desigualdade composta do keyset. Sem o desempate por `id`, uma
        // rajada de mensagens no mesmo microssegundo repetiria ou pularia
        // linha entre páginas.
        condicao = condicao.add(
            Condition::any()
                .add(device_logs::Column::ReceivedAt.lt(cursor.received_at))
                .add(
                    Condition::all()
                        .add(device_logs::Column::ReceivedAt.eq(cursor.received_at))
                        .add(device_logs::Column::Id.lt(cursor.id)),
                ),
        );
    }

    // Uma linha a mais do que o pedido: é o que distingue "acabou" de "tem
    // mais" sem pagar um `COUNT(*)` sobre a janela inteira.
    let sonda = query.limit + 1;
    let mut rows = device_logs::Entity::find()
        .filter(condicao)
        .order_by_desc(device_logs::Column::ReceivedAt)
        .order_by_desc(device_logs::Column::Id)
        .limit(sonda)
        .all(db)
        .await?;

    let tem_mais = rows.len() as u64 > query.limit;
    if tem_mais {
        rows.truncate(usize::try_from(query.limit).unwrap_or(usize::MAX));
    }
    let next_cursor = if tem_mais {
        rows.last().map(|linha| Cursor {
            received_at: linha.received_at.into(),
            id: linha.id,
        })
    } else {
        None
    };

    Ok(LogPage { rows, next_cursor })
}

/// `LIKE '%termo%'` com `%` e `_` do usuário escapados.
///
/// Sem o escape, procurar `pppoe_client` casaria com `pppoe-client` e
/// `pppoeXclient` — o `_` do SQL vale por qualquer caractere, e sublinhado é
/// corriqueiro em texto de log. O `ESCAPE` vale no SQLite e no PostgreSQL.
fn busca_textual(termo: &str) -> Expr {
    let escapado = termo
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    Expr::col(device_logs::Column::Message)
        .like(LikeExpr::new(format!("%{escapado}%")).escape('\\'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::syslog::db;
    use chrono::TimeZone;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    use serial_test::serial;

    fn agora() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 15, 12, 0, 0).unwrap()
    }

    fn consulta(cursor: Option<Cursor>, limit: Option<u64>) -> LogQuery {
        LogQuery::normalize(
            LogFilters {
                from: Some(agora() - Duration::days(1)),
                to: Some(agora()),
                cursor,
                limit,
                ..LogFilters::default()
            },
            agora(),
        )
    }

    async fn banco() -> DatabaseConnection {
        std::env::remove_var("SYSLOG_DB_URL");
        db::connect("sqlite::memory:")
            .await
            .expect("banco de logs")
            .connection()
            .clone()
    }

    async fn semeia(db: &DatabaseConnection, quantas: i64, base: DateTime<Utc>) {
        let linhas: Vec<device_logs::ActiveModel> = (0..quantas)
            .map(|indice| device_logs::ActiveModel {
                device_id: Set(Some(1)),
                source_ip: Set("192.168.88.1".into()),
                received_at: Set((base + Duration::seconds(indice)).into()),
                severity: Set(Some(6)),
                message: Set(format!("linha {indice}")),
                ..Default::default()
            })
            .collect();
        device_logs::Entity::insert_many(linhas)
            .exec(db)
            .await
            .expect("semeadura");
    }

    #[test]
    fn o_cursor_sobrevive_a_ida_e_volta() {
        let cursor = Cursor {
            received_at: agora(),
            id: 4711,
        };
        let decodificado = Cursor::decode(&cursor.encode()).expect("decodifica");
        assert_eq!(decodificado, cursor);
    }

    #[test]
    fn cursor_ilegivel_vira_erro_de_validacao() {
        for lixo in ["não é base64!!", "", "YWJj", &URL_SAFE_NO_PAD.encode("x:y")] {
            assert!(Cursor::decode(lixo).is_err(), "aceitou {lixo:?}");
        }
    }

    #[test]
    fn a_janela_ganha_padrao_de_24h_quando_o_cliente_nao_manda() {
        let query = LogQuery::normalize(LogFilters::default(), agora());
        assert_eq!(query.to, agora());
        assert_eq!(query.from, agora() - Duration::hours(24));
    }

    #[test]
    fn a_janela_tem_teto_porque_o_like_nao_usa_indice() {
        let query = LogQuery::normalize(
            LogFilters {
                from: Some(agora() - Duration::days(365)),
                to: Some(agora()),
                ..LogFilters::default()
            },
            agora(),
        );
        assert_eq!(
            query.from,
            agora() - Duration::days(MAX_WINDOW_DAYS),
            "um pedido de 365 dias tem de virar 7, não varredura da tabela inteira"
        );
    }

    #[test]
    fn intervalo_invertido_e_corrigido_em_vez_de_devolver_vazio() {
        let query = LogQuery::normalize(
            LogFilters {
                from: Some(agora()),
                to: Some(agora() - Duration::hours(2)),
                ..LogFilters::default()
            },
            agora(),
        );
        assert!(query.from < query.to);
    }

    #[test]
    fn o_limite_tem_teto_e_piso() {
        assert_eq!(consulta(None, Some(100_000)).limit, MAX_LIMIT);
        assert_eq!(consulta(None, Some(0)).limit, 1);
        assert_eq!(consulta(None, None).limit, DEFAULT_LIMIT);
    }

    #[test]
    fn o_texto_em_branco_nao_vira_filtro() {
        let query = LogQuery::normalize(
            LogFilters {
                q: Some("   ".into()),
                ..LogFilters::default()
            },
            agora(),
        );
        assert_eq!(query.q, None, "espaço em branco viraria LIKE '%%'");
    }

    #[tokio::test]
    #[serial]
    async fn a_pagina_vem_do_mais_novo_para_o_mais_antigo() {
        let db = banco().await;
        semeia(&db, 5, agora() - Duration::hours(1)).await;

        let pagina = search(&db, &consulta(None, Some(10))).await.expect("busca");

        assert_eq!(pagina.rows.len(), 5);
        assert_eq!(
            pagina.rows[0].message, "linha 4",
            "o mais novo vem primeiro"
        );
        assert!(pagina.next_cursor.is_none(), "acabou, não há próxima");
    }

    #[tokio::test]
    #[serial]
    async fn o_cursor_percorre_tudo_sem_repetir_nem_pular() {
        let db = banco().await;
        semeia(&db, 25, agora() - Duration::hours(2)).await;

        let mut vistas = Vec::new();
        let mut cursor = None;
        for _ in 0..10 {
            let pagina = search(&db, &consulta(cursor, Some(7)))
                .await
                .expect("busca");
            vistas.extend(pagina.rows.iter().map(|linha| linha.id));
            match pagina.next_cursor {
                Some(proximo) => cursor = Some(proximo),
                None => break,
            }
        }

        assert_eq!(vistas.len(), 25, "faltou ou sobrou linha");
        let unicas: std::collections::HashSet<i64> = vistas.iter().copied().collect();
        assert_eq!(unicas.len(), 25, "a paginação repetiu linha");
    }

    #[tokio::test]
    #[serial]
    async fn linhas_do_mesmo_instante_nao_confundem_o_cursor() {
        // Rajada: 10 mensagens no mesmo `received_at`. Sem o desempate por
        // `id`, a segunda página repetiria ou pularia o bloco inteiro.
        let db = banco().await;
        let instante = agora() - Duration::hours(1);
        let linhas: Vec<device_logs::ActiveModel> = (0..10)
            .map(|indice| device_logs::ActiveModel {
                source_ip: Set("192.168.88.1".into()),
                received_at: Set(instante.into()),
                message: Set(format!("rajada {indice}")),
                ..Default::default()
            })
            .collect();
        device_logs::Entity::insert_many(linhas)
            .exec(&db)
            .await
            .expect("semeadura");

        let primeira = search(&db, &consulta(None, Some(4))).await.expect("busca");
        let segunda = search(&db, &consulta(primeira.next_cursor, Some(4)))
            .await
            .expect("busca");

        let ids_primeira: Vec<i64> = primeira.rows.iter().map(|l| l.id).collect();
        let ids_segunda: Vec<i64> = segunda.rows.iter().map(|l| l.id).collect();
        assert_eq!(ids_primeira.len(), 4);
        assert_eq!(ids_segunda.len(), 4);
        assert!(
            ids_primeira.iter().all(|id| !ids_segunda.contains(id)),
            "o mesmo instante confundiu o cursor: {ids_primeira:?} e {ids_segunda:?}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn a_janela_de_tempo_recorta_o_resultado() {
        let db = banco().await;
        semeia(&db, 3, agora() - Duration::hours(1)).await; // dentro
        semeia(&db, 4, agora() - Duration::days(30)).await; // fora

        let pagina = search(&db, &consulta(None, Some(50))).await.expect("busca");

        assert_eq!(pagina.rows.len(), 3, "log fora da janela vazou");
    }

    #[tokio::test]
    #[serial]
    async fn a_severidade_filtra_do_nivel_para_baixo() {
        let db = banco().await;
        for severidade in [2_i16, 3, 4, 6, 7] {
            device_logs::ActiveModel {
                source_ip: Set("192.168.88.1".into()),
                received_at: Set((agora() - Duration::hours(1)).into()),
                severity: Set(Some(severidade)),
                message: Set(format!("sev {severidade}")),
                ..Default::default()
            }
            .insert(&db)
            .await
            .expect("insere");
        }

        let mut query = consulta(None, Some(50));
        query.severity = Some(3);
        let pagina = search(&db, &query).await.expect("busca");

        // "Erro e acima" traz erro (3) e crítico (2), não aviso (4).
        assert_eq!(pagina.rows.len(), 2);
        assert!(pagina.rows.iter().all(|linha| linha.severity <= Some(3)));
    }

    #[tokio::test]
    #[serial]
    async fn o_sublinhado_do_usuario_nao_vira_curinga() {
        let db = banco().await;
        for mensagem in ["pppoe_client caiu", "pppoeXclient caiu"] {
            device_logs::ActiveModel {
                source_ip: Set("192.168.88.1".into()),
                received_at: Set((agora() - Duration::hours(1)).into()),
                message: Set(mensagem.into()),
                ..Default::default()
            }
            .insert(&db)
            .await
            .expect("insere");
        }

        let mut query = consulta(None, Some(50));
        query.q = Some("pppoe_client".into());
        let pagina = search(&db, &query).await.expect("busca");

        assert_eq!(
            pagina.rows.len(),
            1,
            "o `_` do SQL casou com qualquer coisa"
        );
        assert_eq!(pagina.rows[0].message, "pppoe_client caiu");
    }
}
