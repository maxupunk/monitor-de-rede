//! `GET /api/logs` — filtros, envelope de cursor e hidratação do nome.
//!
//! O que só aparece aqui é o acoplamento entre os dois bancos: a página vem do
//! banco de logs e o nome do dispositivo, da base principal. Um teste unitário
//! do repositório não pega uma quebra nessa junção porque ele nem enxerga
//! `devices`.
//!
//! O banco de logs de teste é **em memória** (ver `syslog::db::install`): cada
//! contexto nasce com o seu, porque o `Hooks::truncate` do Loco não alcança um
//! segundo banco e uma linha gravada aqui sobreviveria para o teste seguinte.

use backend::{app::App, models::logs::device_logs, services::syslog::LogsDb};
use chrono::{Duration, Utc};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection};
use serial_test::serial;

use super::prepare_data;

/// Grava uma linha direto no banco de logs, pulando o listener.
async fn grava(
    logs: &DatabaseConnection,
    device_id: Option<i64>,
    severidade: i16,
    minutos_atras: i64,
    mensagem: &str,
) {
    device_logs::ActiveModel {
        device_id: Set(device_id),
        source_ip: Set("10.0.0.1".into()),
        received_at: Set((Utc::now() - Duration::minutes(minutos_atras)).into()),
        facility: Set(Some(16)),
        severity: Set(Some(severidade)),
        hostname: Set(Some("rt-core".into())),
        topics: Set(Some("system,info".into())),
        message: Set(mensagem.into()),
        ..Default::default()
    }
    .insert(logs)
    .await
    .expect("gravar log");
}

fn logs_db(ctx: &AppContext) -> DatabaseConnection {
    LogsDb::from_context(ctx)
        .expect("banco de logs ausente — o after_context deixou de instalá-lo?")
        .connection()
        .clone()
}

/// Cria um dispositivo pela API e devolve o id.
async fn dispositivo(request: &loco_rs::TestServer) -> i64 {
    let site = request
        .post("/api/sites")
        .json(&serde_json::json!({ "name": "Matriz" }))
        .await;
    assert_eq!(site.status_code(), 201, "{}", site.text());
    let site: serde_json::Value = serde_json::from_str(&site.text()).unwrap();

    let device = request
        .post("/api/devices")
        .json(&serde_json::json!({
            "name": "rt-core", "type": "router", "ipAddress": "10.0.0.1",
            "siteId": site["id"].as_i64().unwrap()
        }))
        .await;
    assert_eq!(device.status_code(), 201, "{}", device.text());
    let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
    device["id"].as_i64().unwrap()
}

#[tokio::test]
#[serial]
async fn a_pagina_traz_o_nome_do_dispositivo_do_outro_banco() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let device_id = dispositivo(&request).await;
        let logs = logs_db(&ctx);
        grava(&logs, Some(device_id), 3, 10, "login failure for admin").await;

        let resposta = request.get("/api/logs").await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        let linha = &corpo["data"][0];
        assert_eq!(linha["message"], "login failure for admin");
        assert_eq!(linha["deviceId"], device_id);
        assert_eq!(
            linha["deviceName"], "rt-core",
            "a hidratação do nome quebrou — não há JOIN entre os bancos"
        );
        assert_eq!(linha["severityLabel"], "erro");
        assert_eq!(linha["topics"][0], "system");
        // camelCase em tudo: o frontend lê estes nomes.
        assert!(linha.get("device_id").is_none());
        assert!(corpo["meta"]["hasMore"].is_boolean());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_cursor_percorre_as_paginas_pela_api() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let logs = logs_db(&ctx);
        for indice in 0..7 {
            grava(&logs, None, 6, indice, &format!("linha {indice}")).await;
        }

        let primeira = request.get("/api/logs?limit=3").await;
        let primeira: serde_json::Value = serde_json::from_str(&primeira.text()).unwrap();
        assert_eq!(primeira["data"].as_array().unwrap().len(), 3);
        assert_eq!(primeira["meta"]["hasMore"], true);
        let cursor = primeira["meta"]["nextCursor"].as_str().unwrap().to_owned();

        let segunda = request
            .get(&format!("/api/logs?limit=3&cursor={cursor}"))
            .await;
        let segunda: serde_json::Value = serde_json::from_str(&segunda.text()).unwrap();
        assert_eq!(segunda["data"].as_array().unwrap().len(), 3);

        // Nenhum id se repete entre as páginas.
        let ids_um: Vec<i64> = primeira["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|linha| linha["id"].as_i64().unwrap())
            .collect();
        let ids_dois: Vec<i64> = segunda["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|linha| linha["id"].as_i64().unwrap())
            .collect();
        assert!(
            ids_um.iter().all(|id| !ids_dois.contains(id)),
            "{ids_um:?} {ids_dois:?}"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn os_filtros_recortam_o_resultado() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let logs = logs_db(&ctx);
        grava(&logs, None, 3, 5, "erro de autenticação").await;
        grava(&logs, None, 6, 5, "usuário conectado").await;
        grava(&logs, None, 3, 60 * 24 * 3, "erro antigo").await;

        // "Erro e acima" não traz informação (6).
        let por_severidade = request.get("/api/logs?severity=3").await;
        let corpo: serde_json::Value = serde_json::from_str(&por_severidade.text()).unwrap();
        assert_eq!(corpo["data"].as_array().unwrap().len(), 1);
        assert_eq!(corpo["data"][0]["message"], "erro de autenticação");

        // A janela padrão de 24 h deixa o erro de três dias atrás de fora.
        let padrao = request.get("/api/logs").await;
        let corpo: serde_json::Value = serde_json::from_str(&padrao.text()).unwrap();
        assert_eq!(corpo["data"].as_array().unwrap().len(), 2);

        // Busca textual.
        let por_texto = request.get("/api/logs?q=conectado").await;
        let corpo: serde_json::Value = serde_json::from_str(&por_texto.text()).unwrap();
        assert_eq!(corpo["data"].as_array().unwrap().len(), 1);
        assert_eq!(corpo["data"][0]["message"], "usuário conectado");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_filtro_de_dispositivo_nunca_traz_logs_de_outro_equipamento() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let logs = logs_db(&ctx);
        grava(&logs, Some(9_001), 6, 2, "log do dispositivo um").await;
        grava(&logs, Some(9_002), 6, 1, "log do dispositivo dois").await;

        let resposta = request.get("/api/logs?deviceId=9002").await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();
        let linhas = corpo["data"].as_array().unwrap();

        assert_eq!(
            linhas.len(),
            1,
            "um log de outro dispositivo vazou: {linhas:?}"
        );
        assert_eq!(linhas[0]["deviceId"], 9_002);
        assert_eq!(linhas[0]["message"], "log do dispositivo dois");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_janela_devolvida_mostra_o_teto_aplicado() {
    // Quem pede um ano recebe sete dias. O `meta` diz qual janela valeu, para o
    // usuário não concluir que o log sumiu.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let ano_passado = (Utc::now() - Duration::days(365)).to_rfc3339();
        let resposta = request
            .get(&format!("/api/logs?from={}", urlencoding(&ano_passado)))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        let from = chrono::DateTime::parse_from_rfc3339(corpo["meta"]["from"].as_str().unwrap())
            .expect("from em RFC 3339");
        let dias = (Utc::now() - from.with_timezone(&Utc)).num_days();
        assert!(dias <= 7, "a janela devolvida foi de {dias} dias");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn entrada_invalida_responde_422_com_mensagem_em_portugues() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let cursor_ruim = request.get("/api/logs?cursor=isto-nao-saiu-daqui").await;
        assert_eq!(cursor_ruim.status_code(), 422, "{}", cursor_ruim.text());
        assert!(
            cursor_ruim.text().contains("Cursor"),
            "{}",
            cursor_ruim.text()
        );

        let data_ruim = request.get("/api/logs?from=ontem").await;
        assert_eq!(data_ruim.status_code(), 422, "{}", data_ruim.text());
        assert!(data_ruim.text().contains("from"), "{}", data_ruim.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_rota_exige_sessao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |request, _ctx| async move {
        let resposta = request.get("/api/logs").await;
        assert_eq!(resposta.status_code(), 401, "{}", resposta.text());
    })
    .await;
}

/// `+` e `:` do RFC 3339 precisam ir escapados na query string.
fn urlencoding(valor: &str) -> String {
    valor.replace('+', "%2B").replace(':', "%3A")
}

// --- Fase 4: fontes, vínculo manual e snippets -------------------------------

#[tokio::test]
#[serial]
async fn as_fontes_vistas_incluem_contadores_e_desconhecidas() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let resposta = request.get("/api/logs/sources").await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        assert!(corpo["data"].is_array());
        assert!(corpo["unknownCount"].is_number());
        // Os contadores existem desde o boot, mesmo sem tráfego — é o que a
        // tela usa para dizer "nada chegou ainda" em vez de ficar em branco.
        assert!(corpo["metrics"]["received"].is_number());
        assert!(corpo["metrics"]["droppedUnknownSource"].is_number());
        // O diagnóstico de NAT vem sempre, mesmo sem origem mascarada: é ele
        // que permite a tela explicar um parque inteiro "desconhecido" sem o
        // operador adivinhar.
        assert!(corpo["nat"]["maskedCount"].is_number());
        assert!(corpo["nat"]["containerized"].is_boolean());
        assert!(corpo["nat"]["gateways"].is_array());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_vinculo_recusa_o_gateway_do_docker_com_a_explicacao() {
    // É o erro que a tela mais convida a cometer: a origem mascarada aparece
    // como desconhecida, o seletor está ali, e vincular atribuiria o parque
    // inteiro a um dispositivo. O estrago só apareceria depois, na aba de logs
    // do aparelho errado e no alerta disparado no alvo errado.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let device_id = dispositivo(&request).await;

        let recusa = request
            .post("/api/logs/sources/172.17.0.1/bind")
            .json(&serde_json::json!({ "deviceId": device_id }))
            .await;
        assert_eq!(recusa.status_code(), 400, "{}", recusa.text());
        assert!(
            recusa.text().contains("gateway do Docker"),
            "a recusa precisa dizer o motivo e o caminho certo: {}",
            recusa.text()
        );

        // E nada foi gravado — recusar pela metade seria pior que não recusar.
        let mapa = backend::services::syslog::resolver::bindings(&ctx.db)
            .await
            .expect("bindings");
        assert!(!mapa.contains_key("172.17.0.1"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_vinculo_por_hostname_e_aceito_e_persiste_com_o_prefixo() {
    // O caminho que sobra atrás do NAT: o nome que o equipamento envia é a
    // única coisa que ainda o distingue dos outros.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let device_id = dispositivo(&request).await;

        let vinculo = request
            .post("/api/logs/sources/host%3AMikroTik-Borda/bind")
            .json(&serde_json::json!({ "deviceId": device_id }))
            .await;
        assert_eq!(vinculo.status_code(), 200, "{}", vinculo.text());

        let mapa = backend::services::syslog::resolver::bindings(&ctx.db)
            .await
            .expect("bindings");
        assert_eq!(mapa.get("host:MikroTik-Borda"), Some(&device_id));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_vinculo_manual_persiste_e_pode_ser_desfeito() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let device_id = dispositivo(&request).await;

        let vinculo = request
            .post("/api/logs/sources/203.0.113.7/bind")
            .json(&serde_json::json!({ "deviceId": device_id }))
            .await;
        assert_eq!(vinculo.status_code(), 200, "{}", vinculo.text());

        let mapa = backend::services::syslog::resolver::bindings(&ctx.db)
            .await
            .expect("bindings");
        assert_eq!(mapa.get("203.0.113.7"), Some(&device_id));

        // `deviceId` nulo desfaz — mesmo endpoint, porque a tela oferece
        // "nenhum" no mesmo seletor.
        let desfaz = request
            .post("/api/logs/sources/203.0.113.7/bind")
            .json(&serde_json::json!({ "deviceId": serde_json::Value::Null }))
            .await;
        assert_eq!(desfaz.status_code(), 200, "{}", desfaz.text());
        let mapa = backend::services::syslog::resolver::bindings(&ctx.db)
            .await
            .expect("bindings");
        assert!(!mapa.contains_key("203.0.113.7"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_vinculo_recusa_ip_invalido_e_dispositivo_inexistente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let ip_ruim = request
            .post("/api/logs/sources/nao-e-um-ip/bind")
            .json(&serde_json::json!({ "deviceId": serde_json::Value::Null }))
            .await;
        assert_eq!(ip_ruim.status_code(), 422, "{}", ip_ruim.text());

        let inexistente = request
            .post("/api/logs/sources/10.0.0.1/bind")
            .json(&serde_json::json!({ "deviceId": 99_999 }))
            .await;
        assert_eq!(inexistente.status_code(), 404, "{}", inexistente.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_snippet_traz_o_endereco_informado_e_a_porta_publicada() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let resposta = request
            .get("/api/logs/setup-snippet?address=192.168.1.10")
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        assert_eq!(corpo["serverAddress"], "192.168.1.10");
        // 514 é a porta publicada pelo compose; 5514 é interna e não serve ao
        // roteador.
        assert_eq!(corpo["port"], 514);
        let comandos = corpo["snippets"][0]["commands"].as_str().unwrap();
        assert!(comandos.contains("192.168.1.10"), "{comandos}");
        assert!(comandos.contains("bsd-syslog=yes"), "{comandos}");
        assert!(!corpo.to_string().contains("5514"), "vazou a porta interna");
    })
    .await;
}

// --- Ativação automática do log no equipamento -------------------------------

#[tokio::test]
#[serial]
async fn a_ativacao_recusa_dispositivo_sem_ip_antes_de_tentar_conectar() {
    // Sem endereço não há para onde conectar, e o erro precisa dizer isso em
    // vez de esperar o timeout de uma conexão que nunca foi tentada.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let site = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Matriz" }))
            .await;
        let site: serde_json::Value = serde_json::from_str(&site.text()).unwrap();
        let device = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "sem-ip", "type": "router",
                "siteId": site["id"].as_i64().unwrap()
            }))
            .await;
        let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
        let device_id = device["id"].as_i64().unwrap();

        let resposta = request
            .post(&format!("/api/logs/devices/{device_id}/provision"))
            .json(&serde_json::json!({
                "protocol": "ssh", "username": "admin", "password": "x",
                "serverAddress": "192.168.1.10"
            }))
            .await;
        assert_eq!(resposta.status_code(), 400, "{}", resposta.text());
        assert!(
            resposta.text().contains("endereço IP"),
            "{}",
            resposta.text()
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_ativacao_valida_protocolo_e_credencial_antes_de_abrir_socket() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let device_id = dispositivo(&request).await;
        let rota = format!("/api/logs/devices/{device_id}/provision");

        let protocolo = request
            .post(&rota)
            .json(&serde_json::json!({
                "protocol": "rlogin", "username": "admin", "password": "x",
                "serverAddress": "192.168.1.10"
            }))
            .await;
        assert_eq!(protocolo.status_code(), 422, "{}", protocolo.text());

        let sem_senha = request
            .post(&rota)
            .json(&serde_json::json!({
                "protocol": "ssh", "username": "admin", "password": "",
                "serverAddress": "192.168.1.10"
            }))
            .await;
        assert_eq!(sem_senha.status_code(), 422, "{}", sem_senha.text());

        let sem_usuario = request
            .post(&rota)
            .json(&serde_json::json!({
                "protocol": "ssh", "username": "  ", "password": "x",
                "serverAddress": "192.168.1.10"
            }))
            .await;
        assert_eq!(sem_usuario.status_code(), 422, "{}", sem_usuario.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_ativacao_recusa_localhost_como_endereco_do_servidor() {
    // O bug de campo: quem abre a interface em `http://localhost:3333` mandava
    // `localhost` para dentro do roteador, que passava a enviar o syslog para
    // si mesmo — sem erro em lugar nenhum e sem nada chegando.
    //
    // O alvo é `127.0.0.1:9` (fechada), então a recusa **precisa** vir antes de
    // qualquer tentativa de conexão para o teste distinguir os dois erros.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        std::env::remove_var("FRONTEND_ORIGIN");

        let site = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Matriz" }))
            .await;
        let site: serde_json::Value = serde_json::from_str(&site.text()).unwrap();
        let device = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "local", "type": "router", "ipAddress": "127.0.0.1",
                "siteId": site["id"].as_i64().unwrap()
            }))
            .await;
        let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
        let device_id = device["id"].as_i64().unwrap();

        // `127.0.0.1` como alvo faz o palpite automático também cair no
        // loopback, e o filtro precisa recusá-lo igualmente — senão o
        // saneamento do campo seria contornado pela descoberta.
        let resposta = request
            .post(&format!("/api/logs/devices/{device_id}/provision"))
            .json(&serde_json::json!({
                "protocol": "telnet", "port": 9, "username": "admin", "password": "x",
                "serverAddress": "localhost"
            }))
            .await;
        assert_eq!(resposta.status_code(), 400, "{}", resposta.text());
        assert!(
            resposta.text().contains("Endereço deste servidor"),
            "a recusa precisa dizer qual campo preencher: {}",
            resposta.text()
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_mac_telnet_exige_mac_e_nao_exige_ip() {
    // O MAC-Telnet existe justamente para o equipamento sem IP utilizável, então
    // a falta de IP não pode barrá-lo. A falta de MAC, sim.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let site = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Matriz" }))
            .await;
        let site: serde_json::Value = serde_json::from_str(&site.text()).unwrap();
        let device = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "sem-ip", "type": "router",
                "siteId": site["id"].as_i64().unwrap()
            }))
            .await;
        let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
        let device_id = device["id"].as_i64().unwrap();
        let rota = format!("/api/logs/devices/{device_id}/provision");

        let sem_mac = request
            .post(&rota)
            .json(&serde_json::json!({
                "protocol": "mactelnet", "username": "admin", "password": "x",
                "serverAddress": "192.168.1.10"
            }))
            .await;
        assert_eq!(sem_mac.status_code(), 400, "{}", sem_mac.text());
        assert!(sem_mac.text().contains("MAC"), "{}", sem_mac.text());

        // MAC malformado é erro de entrada, não de rede.
        let mac_ruim = request
            .post(&rota)
            .json(&serde_json::json!({
                "protocol": "mactelnet", "username": "admin", "password": "x",
                "serverAddress": "192.168.1.10", "macAddress": "não é mac"
            }))
            .await;
        assert_eq!(mac_ruim.status_code(), 422, "{}", mac_ruim.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn os_palpites_da_tela_nunca_sugerem_um_endereco_local() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);
        // `FRONTEND_ORIGIN` apontando para localhost é o padrão do `.env` de
        // desenvolvimento — e é por ele que o palpite ruim voltaria.
        std::env::set_var("FRONTEND_ORIGIN", "http://localhost:5173");

        let device_id = dispositivo(&request).await;
        let resposta = request
            .get(&format!("/api/logs/devices/{device_id}/provision-hints"))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();

        assert!(corpo["serverPort"].is_number());
        assert!(corpo["operatingSystem"].is_string());
        assert!(corpo["sshOpen"].is_boolean());
        assert!(corpo["telnetOpen"].is_boolean());
        assert!(corpo["layer2Reachable"].is_boolean());

        if let Some(endereco) = corpo["serverAddress"].as_str() {
            assert!(
                !endereco.contains("localhost") && !endereco.starts_with("127."),
                "sugeriu {endereco}, que aponta o roteador para ele mesmo"
            );
        }
        std::env::remove_var("FRONTEND_ORIGIN");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_ativacao_recusa_dispositivo_inexistente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let resposta = request
            .post("/api/logs/devices/99999/provision")
            .json(&serde_json::json!({
                "protocol": "ssh", "username": "admin", "password": "x",
                "serverAddress": "192.168.1.10"
            }))
            .await;
        assert_eq!(resposta.status_code(), 404, "{}", resposta.text());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_ativacao_falha_com_mensagem_de_operador_quando_a_porta_esta_fechada() {
    // O alvo é `127.0.0.1` numa porta alta e fechada — nada sai da máquina,
    // conforme as diretrizes de teste. O que importa é a **classe** do erro:
    // porta fechada é problema do operador (400), não falha interna (500).
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let site = request
            .post("/api/sites")
            .json(&serde_json::json!({ "name": "Matriz" }))
            .await;
        let site: serde_json::Value = serde_json::from_str(&site.text()).unwrap();
        let device = request
            .post("/api/devices")
            .json(&serde_json::json!({
                "name": "local", "type": "router", "ipAddress": "127.0.0.1",
                "siteId": site["id"].as_i64().unwrap()
            }))
            .await;
        let device: serde_json::Value = serde_json::from_str(&device.text()).unwrap();
        let device_id = device["id"].as_i64().unwrap();

        let resposta = request
            .post(&format!("/api/logs/devices/{device_id}/provision"))
            .json(&serde_json::json!({
                "protocol": "telnet", "port": 9, "username": "admin", "password": "x",
                "serverAddress": "192.168.1.10"
            }))
            .await;
        assert_eq!(
            resposta.status_code(),
            400,
            "porta fechada é erro de operador: {}",
            resposta.text()
        );
        assert!(
            resposta.text().contains("acessar o equipamento"),
            "{}",
            resposta.text()
        );
    })
    .await;
}
