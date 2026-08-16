//! Consulta, live tail e diagnóstico do servidor de syslog.
//!
//! Controller extrai, valida, delega e serializa: a consulta vive em
//! `services::syslog::repository` e a hidratação do nome do dispositivo, em
//! `views::logs`.

use std::convert::Infallible;

use axum::{
    extract::{Path, Query},
    http::{header, HeaderValue},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
};
use chrono::{DateTime, Utc};
use loco_rs::prelude::*;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

use crate::{
    dtos::logs::{BindSourceInput, LogEntry, LogStreamQuery, LogsQuery},
    services::{
        shared::errors::{AppError, AppResult},
        syslog::{
            repository::{self, Cursor, LogFilters, LogQuery},
            resolver, snippets, LogsDb, SyslogService,
        },
    },
    views::logs::{serialize_page, serialize_sources},
};

/// O serviço de ingestão, ou um erro nomeado quando ele está desligado.
fn service(ctx: &AppContext) -> AppResult<SyslogService> {
    SyslogService::from_context(ctx).ok_or_else(|| {
        AppError::BusinessRule(
            "O servidor de syslog não está ativo neste processo (SYSLOG_ENABLED=false).".into(),
        )
    })
}

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

/// `GET /api/logs/stream` — live tail filtrado, por SSE.
///
/// Barramento **próprio**, nunca o `EventBus` de domínio: a 12 msg/s o log
/// rolaria o anel de 1024 do dashboard em ~85 s e faria o painel inteiro
/// ressincronizar sem parar (ver `services::syslog::bus`).
async fn stream(
    State(ctx): State<AppContext>,
    Query(query): Query<LogStreamQuery>,
) -> AppResult<Response> {
    let servico = service(&ctx)?;
    let mut updates = servico.bus.subscribe();
    let (sender, receiver) = mpsc::channel::<Option<LogEntry>>(64);

    tokio::spawn(async move {
        // Primeiro quadro com o `retry`: o `EventSource` do navegador tem
        // backoff próprio, e sem esta linha uma queda de rede deixaria o tail
        // mudo por muito mais do que os 3 s combinados.
        if sender.send(None).await.is_err() {
            return;
        }
        loop {
            match updates.recv().await {
                Ok(entry) => {
                    if !passa_no_filtro(&entry, &query) {
                        continue;
                    }
                    if sender.send(Some(entry)).await.is_err() {
                        return;
                    }
                }
                // Atraso do assinante é normal aqui: o tail mostra o que está
                // chegando, e o que passou está na paginação. Segue lendo em
                // vez de derrubar a conexão.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });

    let stream = ReceiverStream::new(receiver).map(|entry| {
        let primeiro = entry.is_none();
        let corpo = entry.map_or_else(
            || r#"{"type":"stream:connected"}"#.to_owned(),
            |entry| serde_json::to_string(&entry).unwrap_or_else(|_| "{}".into()),
        );
        let frame = Event::default().data(corpo);
        Ok::<Event, Infallible>(if primeiro {
            frame.retry(std::time::Duration::from_millis(3_000))
        } else {
            frame
        })
    });

    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(std::time::Duration::from_secs(25))
                .text("keep-alive"),
        )
        .into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}

/// O filtro do tail espelha o da consulta: mesma semântica de severidade
/// (número menor é mais grave), para o tempo real não mostrar o que a tabela
/// filtrada esconderia.
fn passa_no_filtro(entry: &LogEntry, query: &LogStreamQuery) -> bool {
    if let Some(device_id) = query.device_id {
        if entry.device_id != Some(device_id) {
            return false;
        }
    }
    if let Some(severity) = query.severity {
        // Linha sem severidade não é escondida por um filtro de severidade:
        // ela sumiria da tela sem o operador entender por quê.
        if entry.severity.is_some_and(|atual| atual > severity) {
            return false;
        }
    }
    true
}

/// `GET /api/logs/sources` — o que está chegando, resolvido ou não.
async fn sources(State(ctx): State<AppContext>) -> AppResult<Response> {
    let servico = service(&ctx)?;
    let resposta = serialize_sources(
        &ctx.db,
        servico.sources.list(),
        servico.sources.unknown_count(),
        servico.ingestor.metrics().snapshot(),
    )
    .await?;
    Ok(format::json(resposta)?)
}

/// `POST /api/logs/sources/{ip}/bind` — vincula (ou desvincula) um IP.
async fn bind_source(
    State(ctx): State<AppContext>,
    Path(ip): Path<String>,
    body: String,
) -> AppResult<Response> {
    let entrada: BindSourceInput = crate::dtos::optional_body(&body);

    ip.parse::<std::net::IpAddr>()
        .map_err(|_| AppError::Validation(format!("`{ip}` não é um endereço IP válido.")))?;

    if let Some(device_id) = entrada.device_id {
        if crate::models::devices::Entity::find_by_id(device_id)
            .one(&ctx.db)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound("Dispositivo não encontrado.".into()));
        }
    }

    resolver::bind(&ctx.db, &ip, entrada.device_id).await?;
    // O resolvedor guarda a decisão por 30 s; sem invalidar, o operador
    // vincularia o IP e continuaria vendo "fonte desconhecida" por meio minuto.
    if let Ok(servico) = service(&ctx) {
        servico.ingestor.resolver().invalidate().await;
    }
    Ok(format::json(
        serde_json::json!({ "sourceIp": ip, "deviceId": entrada.device_id }),
    )?)
}

/// `GET /api/logs/setup-snippet` — comandos prontos por fabricante.
async fn setup_snippet(Query(query): Query<SetupQuery>) -> AppResult<Response> {
    let endereco = query
        .address
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map_or_else(endereco_do_servidor, str::to_owned);
    Ok(format::json(snippets::build(&endereco))?)
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupQuery {
    /// Endereço a estampar nos comandos.
    ///
    /// A tela manda o host da barra de endereços, que é o melhor palpite
    /// possível: é o endereço pelo qual o operador de fato alcança o servidor.
    /// De dentro do container o processo só enxerga o IP da bridge, que não
    /// serve para roteador nenhum.
    address: Option<String>,
}

/// Palpite de último recurso quando a tela não informou o endereço.
fn endereco_do_servidor() -> String {
    let Ok(origem) = std::env::var("FRONTEND_ORIGIN") else {
        return String::new();
    };
    // Exige a forma de URL: uma `FRONTEND_ORIGIN` que não é URL é
    // configuração errada, e adivinhar um host a partir dela produziria um
    // snippet que falha em silêncio no roteador.
    let Some((_, sem_esquema)) = origem.split_once("//") else {
        return String::new();
    };
    let host = sem_esquema.split(['/', ':']).next().unwrap_or_default();
    // `localhost` não serve: o roteador precisa alcançar este servidor pela
    // rede, e um snippet com `remote=localhost` falha em silêncio no aparelho.
    if host.is_empty() || host == "localhost" {
        return String::new();
    }
    host.to_owned()
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/logs")
        .add("/", get(index))
        .add("/stream", get(stream))
        .add("/sources", get(sources))
        .add("/sources/{ip}/bind", post(bind_source))
        .add("/setup-snippet", get(setup_snippet))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn entrada(device_id: Option<i64>, severity: Option<i16>) -> LogEntry {
        LogEntry {
            id: 1,
            device_id,
            device_name: None,
            source_ip: "10.0.0.1".into(),
            received_at: "2026-08-15T12:00:00Z".into(),
            device_time: None,
            facility: None,
            severity,
            severity_label: None,
            hostname: None,
            app_name: None,
            pid: None,
            topics: Vec::new(),
            message: "linha".into(),
        }
    }

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

    #[test]
    fn sem_filtro_tudo_passa_no_tail() {
        let query = LogStreamQuery::default();
        assert!(passa_no_filtro(&entrada(None, None), &query));
        assert!(passa_no_filtro(&entrada(Some(7), Some(6)), &query));
    }

    #[test]
    fn o_tail_respeita_o_dispositivo() {
        let query = LogStreamQuery {
            device_id: Some(7),
            severity: None,
        };
        assert!(passa_no_filtro(&entrada(Some(7), None), &query));
        assert!(!passa_no_filtro(&entrada(Some(8), None), &query));
        assert!(!passa_no_filtro(&entrada(None, None), &query));
    }

    #[test]
    fn o_tail_filtra_severidade_do_nivel_para_baixo() {
        let query = LogStreamQuery {
            device_id: None,
            severity: Some(3),
        };
        assert!(passa_no_filtro(&entrada(None, Some(2)), &query), "crítico");
        assert!(passa_no_filtro(&entrada(None, Some(3)), &query), "erro");
        assert!(!passa_no_filtro(&entrada(None, Some(6)), &query), "info");
        // Sem severidade não é escondido: sumiria sem explicação.
        assert!(passa_no_filtro(&entrada(None, None), &query));
    }

    #[test]
    #[serial]
    fn o_endereco_de_ultimo_recurso_recusa_localhost() {
        // `remote=localhost` no roteador falha em silêncio no aparelho.
        for origem in [
            "http://localhost:3333",
            "https://localhost",
            "http://",
            "lixo",
        ] {
            std::env::set_var("FRONTEND_ORIGIN", origem);
            assert!(
                endereco_do_servidor().is_empty(),
                "aceitou {origem:?} como endereço de servidor"
            );
        }
        std::env::set_var("FRONTEND_ORIGIN", "http://192.168.1.10:3333");
        assert_eq!(endereco_do_servidor(), "192.168.1.10");
        std::env::remove_var("FRONTEND_ORIGIN");
    }
}
