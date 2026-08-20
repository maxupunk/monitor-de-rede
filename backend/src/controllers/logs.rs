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
    dtos::logs::{
        BindSourceInput, LogEntry, LogStreamQuery, LogsQuery, ProvisionHintsResponse,
        ProvisionLoggingInput, ProvisionLoggingResponse,
    },
    services::{
        devices::{access, systems},
        network_tools::mactelnet,
        server_addresses,
        shared::errors::{AppError, AppResult},
        syslog::{
            hints, provision,
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
        servico.ingestor.resolver().nat(),
    )
    .await?;
    Ok(format::json(resposta)?)
}

/// `POST /api/logs/sources/{key}/bind` — vincula (ou desvincula) uma origem.
///
/// A chave é um IP ou um hostname prefixado (`host:MikroTik-CCR`), que é o que
/// a tela devolve quando a origem chega mascarada por NAT.
async fn bind_source(
    State(ctx): State<AppContext>,
    Path(chave): Path<String>,
    body: String,
) -> AppResult<Response> {
    let entrada: BindSourceInput = crate::dtos::optional_body(&body);
    let por_hostname = chave.starts_with(resolver::HOSTNAME_BIND_PREFIX);

    if !por_hostname {
        let endereco = chave
            .parse::<std::net::IpAddr>()
            .map_err(|_| AppError::Validation(format!("`{chave}` não é um endereço IP válido.")))?;

        // Vincular o gateway do NAT atribuiria **todos** os equipamentos atrás
        // dele ao mesmo dispositivo. É o erro que a tela mais convida a
        // cometer — a origem aparece como desconhecida e o seletor está logo
        // ali — e o estrago (aba contaminada, alerta no alvo errado) só
        // aparece depois. Recusar e dizer o caminho certo é mais barato.
        if let Ok(servico) = service(&ctx) {
            if servico.ingestor.resolver().nat().is_masked(endereco) {
                return Err(AppError::business_rule(format!(
                    "`{chave}` é o gateway do Docker, não o endereço de um equipamento: todos os \
                     roteadores chegam por ele. Vincular aqui atribuiria o parque inteiro a um \
                     dispositivo só. Vincule pelo nome que o equipamento envia no syslog, ou use \
                     `network_mode: host` no compose para que o endereço real chegue."
                )));
            }
        }
    }

    if let Some(device_id) = entrada.device_id {
        if crate::models::devices::Entity::find_by_id(device_id)
            .one(&ctx.db)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound("Dispositivo não encontrado.".into()));
        }
    }

    resolver::bind(&ctx.db, &chave, entrada.device_id).await?;

    let mut resolvido = None;
    if let Ok(servico) = service(&ctx) {
        // O resolvedor guarda a decisão por 30 s; sem invalidar, o operador
        // vincularia o IP e continuaria vendo "fonte desconhecida" por meio
        // minuto.
        servico.ingestor.resolver().invalidate().await;

        // E o registro de origens guarda o que valia **quando a última linha
        // chegou** — ele só aprende com mensagem nova. Num roteador que fala de
        // hora em hora, a tela continuaria dizendo "Descartando" e o seletor
        // voltaria a vazio: a resposta seria honesta e leria como se o vínculo
        // não tivesse pegado. Reclassificar aqui é o que faz a escolha valer no
        // mesmo instante.
        //
        // Resolvido de novo, e não assumido: no desvínculo o certo é o que a
        // heurística disser agora — rede cadastrada, ambíguo ou desconhecido —
        // e não "desconhecido" por decreto.
        if let Some(fonte) = servico.sources.snapshot(&chave) {
            if let Ok(origem) = fonte.source_ip.parse::<std::net::IpAddr>() {
                let resolucao = servico
                    .ingestor
                    .resolver()
                    .resolve(&ctx.db, origem, fonte.hostname.as_deref())
                    .await?;
                servico.sources.reclassify(&chave, &resolucao);
                resolvido = resolucao.device_id();
            }
        }
    }

    Ok(format::json(serde_json::json!({
        "bindKey": chave,
        // O que o vínculo produziu de fato. No desvínculo pode não ser nulo: a
        // origem volta a ser resolvida pelo cadastro, e a tela precisa mostrar
        // isso em vez de um campo vazio que mente.
        "deviceId": resolvido.or(entrada.device_id),
    }))?)
}

/// Dedução do sistema a partir **só** do cadastro — sem tocar a rede.
///
/// A ativação já tem o que precisa em mãos: quando a tela mandou um sistema, a
/// escolha é dela; quando não mandou, sondar SNMP e SSH de novo aqui repetiria o
/// que o `provision-hints` acabou de fazer, e ainda atrasaria o começo da
/// sessão que o operador está esperando.
fn do_cadastro(
    dispositivo: &crate::models::devices::Model,
    declarado: Option<&str>,
) -> systems::Detection {
    systems::detect(&systems::Evidence {
        declared: declarado,
        vendor: dispositivo.vendor.as_deref(),
        model: dispositivo.model.as_deref(),
        ..systems::Evidence::default()
    })
}

/// `POST /api/logs/devices/{id}/provision` — entra no equipamento e configura
/// o envio de syslog.
///
/// A credencial vem no corpo, é usada uma vez e não é gravada em lugar nenhum
/// — ver a nota do módulo [`crate::services::syslog::provision`].
async fn provision_device(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    Json(entrada): Json<ProvisionLoggingInput>,
) -> AppResult<Response> {
    let dispositivo = crate::models::devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Dispositivo não encontrado.".into()))?;

    if entrada.username.trim().is_empty() {
        return Err(AppError::validation("Informe o usuário de acesso."));
    }
    if entrada.password.is_empty() {
        return Err(AppError::validation("Informe a senha de acesso."));
    }

    let protocolo = provision::Protocol::parse(&entrada.protocol)?;

    // O IP é obrigatório para SSH e Telnet, e **não** para o MAC-Telnet: chegar
    // a um equipamento sem IP utilizável é justamente para o que ele serve.
    let host = match dispositivo
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
    {
        Some(texto) => Some(texto.parse::<std::net::IpAddr>().map_err(|_| {
            AppError::business_rule(
                "O endereço cadastrado neste dispositivo não é um IP válido. Corrija o cadastro \
                 antes de ativar o log.",
            )
        })?),
        None if protocolo.by_mac() => None,
        None => {
            return Err(AppError::business_rule(
                "Este dispositivo não tem endereço IP cadastrado — não há para onde conectar. \
                 Use MAC-Telnet se o equipamento for MikroTik e estiver na mesma rede local.",
            ))
        }
    };
    // O que a tela mandou é uma escolha do catálogo, e um id fora dele é erro de
    // validação — não um valor que segue adiante para virar "não há receita"
    // três camadas abaixo. Ausente, vale a mesma dedução do cadastro.
    let sistema = match entrada
        .operating_system
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
    {
        Some(escolhido) => systems::parse(escolhido)
            .map_err(AppError::validation)?
            .map_or_else(|| do_cadastro(&dispositivo, None).system, |sistema| sistema),
        None => do_cadastro(&dispositivo, dispositivo.operating_system.as_deref()).system,
    };
    let operating_system = sistema.id.to_owned();

    // O MAC-Telnet endereça por MAC, e `devices` não guarda MAC: ele vem das
    // interfaces coletadas por SNMP ou do ARP do discovery. A tela pode mandar
    // um digitado à mão, que vence os dois.
    let mac = match entrada.mac_address.as_deref().map(str::trim) {
        Some(texto) if !texto.is_empty() => Some(mactelnet::MacAddress::parse(texto)?),
        _ => hints::mac_conhecido(&ctx.db, &dispositivo)
            .await?
            .as_deref()
            .map(mactelnet::MacAddress::parse)
            .transpose()?,
    };
    if protocolo.by_mac() && mac.is_none() {
        return Err(AppError::business_rule(
            "O MAC-Telnet endereça o equipamento pelo MAC, e não há um conhecido para este \
             dispositivo. Rode uma coleta SNMP ou uma descoberta na rede, ou informe o MAC na \
             tela.",
        ));
    }

    // `localhost` é o motivo de este saneamento existir: é o host da barra de
    // endereços de quem abre a interface na própria máquina, é aceito por todo
    // comando de configuração, e faz o roteador mandar o syslog para si mesmo —
    // sem erro, sem aviso e sem nada chegando. Ver `hints::sanitiza_endereco`.
    let server_address = hints::sanitiza_endereco(entrada.server_address.as_deref())
        .or_else(|| {
            host.and_then(hints::local_address_toward)
                .map(|ip| ip.to_string())
        })
        .or_else(|| hints::sanitiza_endereco(Some(&endereco_do_servidor())))
        .ok_or_else(|| {
            AppError::business_rule(
                "Não foi possível determinar o endereço deste servidor que o equipamento deve \
                 usar. Preencha o campo \"Endereço deste servidor\" com o IP pelo qual o roteador \
                 alcança o NetMonitor — `localhost` não serve, porque aponta o roteador para ele \
                 mesmo.",
            )
        })?;

    let server_port = snippets::published_port();
    let servico = service(&ctx).ok();
    let pedido = provision::ProvisionRequest {
        // Só o MAC-Telnet chega aqui sem IP, e ele não usa este campo.
        host: host.unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)),
        mac,
        port: entrada.port.unwrap_or_else(|| protocolo.default_port()),
        protocol: protocolo,
        username: entrada.username.trim().to_owned(),
        password: entrada.password.clone(),
        operating_system: operating_system.clone(),
        server_address: server_address.clone(),
        server_port,
    };

    let resultado = provision::run(&pedido, servico.as_ref().map(|s| &s.sources), id).await?;

    Ok(format::json(ProvisionLoggingResponse {
        operating_system,
        server_address,
        server_port,
        commands: resultado.commands,
        transcript: resultado.transcript,
        confirmed: resultado.confirmed,
    })?)
}

/// `GET /api/logs/devices/{id}/provision-hints` — o que a tela consegue
/// preencher sozinha antes de pedir qualquer coisa ao operador.
///
/// Sonda portas e consulta o SNMP, então custa alguns segundos no pior caso.
/// Vale: os três campos que ele preenche — endereço do servidor, meio de acesso
/// e fabricante — eram chute, e chute errado falha em silêncio dentro do
/// roteador.
async fn provision_hints(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let dispositivo = crate::models::devices::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Dispositivo não encontrado.".into()))?;

    let detector = service(&ctx).map_or_else(
        |_| crate::services::syslog::NatDetector::detect(),
        |servico| servico.ingestor.resolver().nat().clone(),
    );
    let dicas = hints::collect(&ctx.db, &dispositivo, &detector).await?;

    // A lista de endereços do servidor é a fonte preferencial: ela carrega o
    // que o operador corrigiu à mão, e é ela que a tela oferece no seletor. A
    // detecção por rota só decide **qual** delas serve a este equipamento.
    let lista = server_addresses::list(&ctx.db, &detector).await?;
    // A forma de acesso é resolvida uma vez e usada duas: para escolher o
    // endereço e para explicar a escolha na tela. Resolver de novo lá dentro
    // custaria as mesmas três consultas para chegar à mesma conclusão.
    let contexto = access::AccessContext::load(&ctx.db).await?;
    let acesso = contexto.resolve(&dispositivo);
    let sugestao =
        server_addresses::suggest_with(&ctx.db, &dispositivo, &lista, &contexto, &acesso).await?;
    let sugerido = sugestao.as_ref().and_then(|(id, _)| {
        lista
            .iter()
            .find(|item| &item.id == id)
            .and_then(|item| item.value.clone())
    });

    // Ordem dos palpites, do mais confiável para o menos: a entrada sugerida da
    // lista, a rota até o equipamento, e por fim a `FRONTEND_ORIGIN` — esta
    // última com o mesmo crivo das outras, que é o que impede `localhost` de
    // voltar pela porta dos fundos.
    let (server_address, server_address_source) = match (sugerido, dicas.server_address.clone()) {
        (Some(endereco), _) => (Some(endereco), "endereços deste servidor"),
        (None, Some(endereco)) => (Some(endereco), dicas.server_address_source),
        (None, None) => (
            hints::sanitiza_endereco(Some(&endereco_do_servidor())),
            "origem configurada",
        ),
    };

    Ok(format::json(ProvisionHintsResponse {
        server_address,
        server_address_source: server_address_source.to_owned(),
        suggested_address_id: sugestao.as_ref().map(|(id, _)| id.clone()),
        suggested_address_reason: sugestao.map(|(_, motivo)| motivo),
        access_mode: acesso.mode.id().to_owned(),
        access_mode_declared: acesso.declared,
        access_mode_reason: acesso.reason,
        server_port: snippets::published_port(),
        operating_system: dicas.operating_system,
        operating_system_source: dicas.operating_system_source.to_owned(),
        operating_system_reason: dicas.operating_system_reason,
        ssh_open: dicas.ssh_open,
        telnet_open: dicas.telnet_open,
        mac_address: dicas.mac_address,
        layer2_reachable: dicas.layer2_reachable,
    })?)
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
        .add("/devices/{id}/provision", post(provision_device))
        .add("/devices/{id}/provision-hints", get(provision_hints))
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
