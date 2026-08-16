//! Consulta dos logs recebidos pelo servidor de syslog.
//!
//! Controller extrai, valida, delega e serializa: a consulta vive em
//! `services::syslog::repository` e a hidratação do nome do dispositivo, em
//! `views::logs`.

use axum::extract::Query;
use chrono::{DateTime, Utc};
use loco_rs::prelude::*;

use crate::{
    dtos::logs::LogsQuery,
    services::{
        shared::errors::{AppError, AppResult},
        syslog::{
            repository::{self, Cursor, LogFilters, LogQuery},
            LogsDb,
        },
    },
    views::logs::serialize_page,
};

/// `GET /api/logs` — página filtrada, em envelope de cursor.
async fn index(
    State(ctx): State<AppContext>,
    Query(query): Query<LogsQuery>,
) -> AppResult<Response> {
    let logs = LogsDb::from_context(&ctx)?;

    let filtros = LogQuery::normalize(
        LogFilters {
            device_id: query.device_id,
            severity: query.severity,
            facility: query.facility,
            from: instante(query.from.as_deref(), "from")?,
            to: instante(query.to.as_deref(), "to")?,
            q: query.q,
            cursor: query.cursor.as_deref().map(Cursor::decode).transpose()?,
            limit: query.limit,
        },
        Utc::now(),
    );

    let pagina = repository::search(logs.connection(), &filtros).await?;
    let resposta = serialize_page(&ctx.db, pagina, &filtros).await?;
    Ok(format::json(resposta)?)
}

/// Lê um instante em RFC 3339.
///
/// Data ilegível vira 422 com o nome do campo, e não uma janela silenciosamente
/// diferente da pedida: o usuário que digitou errado precisa saber, senão vai
/// concluir que o log sumiu.
fn instante(valor: Option<&str>, campo: &str) -> AppResult<Option<DateTime<Utc>>> {
    let Some(texto) = valor.map(str::trim).filter(|texto| !texto.is_empty()) else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(texto)
        .map(|instante| Some(instante.with_timezone(&Utc)))
        .map_err(|_| {
            AppError::Validation(format!(
                "Data inválida em `{campo}`: use o formato ISO 8601."
            ))
        })
}

pub fn routes() -> Routes {
    Routes::new().prefix("/logs").add("/", get(index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_ausente_ou_em_branco_nao_e_erro() {
        assert!(instante(None, "from").expect("ausente").is_none());
        assert!(instante(Some("  "), "from").expect("branco").is_none());
    }

    #[test]
    fn data_ilegivel_avisa_em_vez_de_calar() {
        let erro = instante(Some("ontem"), "from").expect_err("tinha de recusar");
        assert!(matches!(erro, AppError::Validation(_)));
        assert!(
            erro.to_string().contains("from"),
            "a mensagem tem de dizer qual campo"
        );
    }

    #[test]
    fn a_data_valida_chega_em_utc() {
        let instante = instante(Some("2026-08-15T09:00:00-03:00"), "from")
            .expect("válida")
            .expect("presente");
        assert_eq!(instante.to_rfc3339(), "2026-08-15T12:00:00+00:00");
    }
}
