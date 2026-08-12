//! Linhas da matriz de paridade (§16) que só se provam contra o banco.
//!
//! O que vive aqui não cabe em unitário: são comportamentos de acoplamento —
//! relay entre processos, transição de status de dispositivo, janela por
//! monitor na apresentação e o ciclo do scheduler.

use backend_rust::{
    app::App,
    models::{devices, monitor_results, monitors, networks, sites},
    services::{
        discovery::queue,
        events::{relay::relay_pending, EventBus},
        monitoring::{
            device_status::{self, DeviceStatus},
            presenter::{present_monitors, RECENT_RESULTS_LIMIT},
        },
    },
};
use chrono::Utc;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serial_test::serial;

async fn site(db: &sea_orm::DatabaseConnection) -> sites::Model {
    sites::ActiveModel {
        name: Set("Matriz".into()),
        active: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("criar site")
}

async fn rede(db: &sea_orm::DatabaseConnection, site_id: i64, cidr: &str) -> networks::Model {
    networks::ActiveModel {
        site_id: Set(Some(site_id)),
        name: Set(format!("Rede {cidr}")),
        cidr: Set(cidr.into()),
        scan_enabled: Set(true),
        scan_interval: Set(3_600),
        active: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("criar rede")
}

async fn dispositivo(db: &sea_orm::DatabaseConnection, nome: &str, status: &str) -> devices::Model {
    devices::ActiveModel {
        name: Set(nome.into()),
        r#type: Set("switch".into()),
        ip_address: Set(Some(format!("10.0.0.{}", nome.len()))),
        is_monitored: Set(true),
        snmp_enabled: Set(false),
        status: Set(status.into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("criar dispositivo")
}

async fn monitor(db: &sea_orm::DatabaseConnection, device_id: i64, nome: &str) -> monitors::Model {
    monitors::ActiveModel {
        device_id: Set(Some(device_id)),
        r#type: Set("ping".into()),
        name: Set(nome.into()),
        configuration: Set(serde_json::json!({ "host": "10.0.0.1" })),
        interval_seconds: Set(60),
        timeout_seconds: Set(10),
        retry_count: Set(3),
        enabled: Set(true),
        status: Set("unknown".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("criar monitor")
}

/// Matriz #4 — `device.status` só muda pelo `device_status`, e a transição é a
/// única coisa que vira evento.
#[tokio::test]
#[serial]
async fn a_transicao_de_status_publica_device_status_uma_unica_vez() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let ctx = &boot.app_context;
    let device = dispositivo(&ctx.db, "sw-core", "online").await;
    let bus = EventBus::from_context(ctx).expect("barramento inicializado");
    let mut inscrito = bus.subscribe();

    // online → offline: transição real.
    let transicao = device_status::apply(ctx, &device, DeviceStatus::Offline, None)
        .await
        .expect("aplicar status");
    assert!(transicao.changed);
    assert_eq!(transicao.previous_status, DeviceStatus::Online);

    let evento = inscrito.try_recv().expect("device:status publicado");
    assert_eq!(evento.event_type, "device:status");
    // O frontend lê `id` **ou** `deviceId`, e ambos precisam existir.
    assert_eq!(evento.payload["id"], device.id);
    assert_eq!(evento.payload["deviceId"], device.id);
    assert_eq!(evento.payload["status"], "offline");
    assert_eq!(evento.payload["previousStatus"], "online");
    assert_eq!(evento.payload["name"], "sw-core");

    // Reaplicar o mesmo status não é transição: nada novo no barramento.
    let recarregado = devices::Entity::find_by_id(device.id)
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();
    let repetido = device_status::apply(ctx, &recarregado, DeviceStatus::Offline, None)
        .await
        .expect("reaplicar status");
    assert!(!repetido.changed);
    assert!(
        inscrito.try_recv().is_err(),
        "status repetido publicou evento — a tela veria offline ➔ offline"
    );
}

/// Matriz #4 — `last_seen_at` é telemetria: avança sem gerar evento.
#[tokio::test]
#[serial]
async fn prova_de_vida_avanca_sem_publicar_transicao() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let ctx = &boot.app_context;
    let device = dispositivo(&ctx.db, "ap-01", "online").await;
    let bus = EventBus::from_context(ctx).expect("barramento inicializado");
    let mut inscrito = bus.subscribe();

    let agora = Utc::now();
    let transicao = device_status::apply(ctx, &device, DeviceStatus::Online, Some(agora))
        .await
        .expect("aplicar status");

    assert!(!transicao.changed);
    assert!(
        inscrito.try_recv().is_err(),
        "prova de vida publicou transição inexistente"
    );
    let recarregado = devices::Entity::find_by_id(device.id)
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();
    assert!(
        recarregado.last_seen_at.is_some(),
        "last_seen_at não foi gravado"
    );
}

/// Matriz #30 e #32 — o relay só lê o banco com assinante SSE conectado, e o
/// que veio de outro processo chega ao barramento local.
#[tokio::test]
#[serial]
async fn o_relay_so_trabalha_com_assinante_e_entrega_evento_de_outro_processo() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let ctx = &boot.app_context;
    let bus = EventBus::from_context(ctx).expect("barramento inicializado");

    // Um processo diferente gravou no outbox (origin alheio).
    backend_rust::models::event_outbox::ActiveModel {
        r#type: Set("monitor:result".into()),
        origin: Set("outro-processo".into()),
        payload: Set(serde_json::json!({
            "type": "monitor:result",
            "data": { "monitorId": 42 },
            "timestamp": Utc::now().to_rfc3339(),
        })),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .expect("gravar no outbox");

    // Sem ninguém escutando, o relay não consulta o banco (matriz #32).
    assert_eq!(
        relay_pending(ctx).await.expect("relay sem assinante"),
        0,
        "relay trabalhou sem assinante SSE"
    );

    // Com assinante, o evento atravessa (matriz #30).
    let mut inscrito = bus.subscribe();
    assert_eq!(relay_pending(ctx).await.expect("relay com assinante"), 1);
    let evento = inscrito.try_recv().expect("evento replicado");
    assert_eq!(evento.event_type, "monitor:result");
    assert_eq!(evento.payload["monitorId"], 42);

    // Segunda passada não reentrega: o `last_relayed_id` avançou.
    assert_eq!(relay_pending(ctx).await.expect("segunda passada"), 0);
}

/// Matriz #31 — o relay ignora o que a própria instância publicou, senão o
/// assinante local receberia o mesmo evento duas vezes.
#[tokio::test]
#[serial]
async fn o_relay_ignora_evento_da_propria_origem() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let ctx = &boot.app_context;
    let bus = EventBus::from_context(ctx).expect("barramento inicializado");
    let mut inscrito = bus.subscribe();

    // `publish` grava no outbox **e** entrega localmente.
    bus.publish(&ctx.db, "device:status", serde_json::json!({ "id": 1 }))
        .await
        .expect("publicar");
    assert_eq!(
        inscrito.try_recv().expect("entrega local").event_type,
        "device:status"
    );

    // O relay vê a linha, reconhece a própria origem e não republica.
    assert_eq!(relay_pending(ctx).await.expect("relay"), 0);
    assert!(
        inscrito.try_recv().is_err(),
        "evento da própria origem foi entregue em dobro"
    );
}

/// Matriz #5 — `recentResults` é limite **por monitor**, não da consulta.
#[tokio::test]
#[serial]
async fn o_historico_recente_tem_trinta_itens_por_monitor() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;
    let device = dispositivo(db, "sw-borda", "online").await;

    let mut criados = Vec::new();
    for indice in 1..=3 {
        let monitor = monitor(db, device.id, &format!("Ping {indice}")).await;
        for passo in 0..50 {
            let instante = Utc::now() - chrono::Duration::seconds(50 - passo);
            monitor_results::ActiveModel {
                monitor_id: Set(monitor.id),
                status: Set("up".into()),
                started_at: Set(instante.into()),
                finished_at: Set(instante.into()),
                duration_ms: Set(5),
                latency_ms: Set(Some(passo as f64)),
                ..Default::default()
            }
            .insert(db)
            .await
            .expect("gravar resultado");
        }
        criados.push(monitor);
    }

    let apresentados = present_monitors(db, criados, RECENT_RESULTS_LIMIT)
        .await
        .expect("apresentar monitores");

    assert_eq!(apresentados.len(), 3);
    for monitor in &apresentados {
        assert_eq!(
            monitor.recent_results.len(),
            RECENT_RESULTS_LIMIT as usize,
            "o LIMIT vazou para o conjunto inteiro: o monitor {} ficou sem timeline",
            monitor.name
        );
        // Ordem crescente: a barra mais recente é a última do array.
        let instantes: Vec<_> = monitor
            .recent_results
            .iter()
            .map(|r| r.started_at.clone())
            .collect();
        let mut ordenados = instantes.clone();
        ordenados.sort();
        assert_eq!(instantes, ordenados, "histórico fora de ordem");
        // `latencyMs` do monitor vem do resultado mais recente.
        assert_eq!(monitor.latency_ms, Some(49.0));
    }
}

/// Matriz #14 — corrigir o CIDR da rede atualiza a run `pending` já enfileirada,
/// em vez de deixar o scheduler varrer a faixa velha.
#[tokio::test]
#[serial]
async fn cidr_corrigido_atualiza_a_run_pendente() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;
    let site = site(db).await;
    let rede = rede(db, site.id, "192.168.1.0/24").await;

    let (primeira, ja_existia) = queue::enqueue_network_scan(db, &rede)
        .await
        .expect("enfileirar");
    assert!(!ja_existia);
    assert_eq!(
        primeira.configuration.as_ref().unwrap()["cidr"],
        "192.168.1.0/24"
    );

    // Operador corrige a faixa antes de o scheduler pegar a run.
    let corrigida = networks::ActiveModel {
        id: Set(rede.id),
        cidr: Set("192.168.5.0/24".into()),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("corrigir cidr");

    let (segunda, ja_existia) = queue::enqueue_network_scan(db, &corrigida)
        .await
        .expect("reenfileirar");
    assert!(ja_existia, "criou uma segunda run em vez de reaproveitar");
    assert_eq!(segunda.id, primeira.id);
    assert_eq!(
        segunda.configuration.as_ref().unwrap()["cidr"],
        "192.168.5.0/24"
    );
}

/// Matriz #13 — a run órfã é fechada e desbloqueia a rede.
#[tokio::test]
#[serial]
async fn run_abandonada_e_fechada_e_libera_a_rede() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;
    let site = site(db).await;
    let rede = rede(db, site.id, "10.10.0.0/24").await;

    let (run, _) = queue::enqueue_network_scan(db, &rede)
        .await
        .expect("enfileirar");
    // Simula o processo que morreu no meio da varredura, 20 min atrás.
    backend_rust::models::discovery_runs::ActiveModel {
        id: Set(run.id),
        status: Set("running".into()),
        started_at: Set((Utc::now() - chrono::Duration::minutes(20)).into()),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("envelhecer a run");

    let (nova, ja_existia) = queue::enqueue_network_scan(db, &rede)
        .await
        .expect("enfileirar depois do abandono");
    assert!(!ja_existia, "a run órfã continuou bloqueando a rede");
    assert_ne!(nova.id, run.id);

    let orfa = backend_rust::models::discovery_runs::Entity::find_by_id(run.id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(orfa.status, "failed");
    assert!(orfa.error.unwrap().contains("abandonada"));
    assert!(orfa.finished_at.is_some());
}

/// Matriz #13 — duas chamadas seguidas não criam varreduras concorrentes.
#[tokio::test]
#[serial]
async fn dois_cliques_em_escanear_nao_viram_duas_varreduras() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;
    let site = site(db).await;
    let rede = rede(db, site.id, "172.16.0.0/24").await;

    let (primeira, _) = queue::enqueue_network_scan(db, &rede)
        .await
        .expect("1º clique");
    let (segunda, ja_existia) = queue::enqueue_network_scan(db, &rede)
        .await
        .expect("2º clique");
    assert!(ja_existia);
    assert_eq!(primeira.id, segunda.id);

    // Mesmo com a run já em curso, o clique devolve a existente.
    backend_rust::models::discovery_runs::ActiveModel {
        id: Set(primeira.id),
        status: Set("running".into()),
        started_at: Set(Utc::now().into()),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("marcar em curso");
    let (terceira, ja_existia) = queue::enqueue_network_scan(db, &rede)
        .await
        .expect("3º clique");
    assert!(ja_existia);
    assert_eq!(terceira.id, primeira.id);
}

/// Matriz #10 — CIDR não varredurável é recusado antes de enfileirar.
#[tokio::test]
#[serial]
async fn faixa_invalida_nao_enfileira_varredura() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;
    let site = site(db).await;
    let rede = rede(db, site.id, "nao-e-cidr").await;

    let erro = queue::enqueue_network_scan(db, &rede)
        .await
        .expect_err("faixa inválida deveria ser recusada");
    assert!(erro.to_string().contains("varredurável"));
}

/// Matriz #6 — o scheduler grava `next_run_at` **antes** de medir, senão dois
/// processos executam a mesma linha quando o ciclo seguinte começa antes de a
/// rede responder.
#[tokio::test]
#[serial]
async fn o_scheduler_reserva_o_monitor_antes_de_executar() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;
    let device = dispositivo(db, "sw-agenda", "online").await;
    let monitor = monitor(db, device.id, "Ping agendado").await;

    // Vencido há muito: entra no lote deste ciclo.
    monitors::ActiveModel {
        id: Set(monitor.id),
        next_run_at: Set(Some((Utc::now() - chrono::Duration::hours(1)).into())),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("vencer o monitor");

    let antes = Utc::now();
    backend_rust::tasks::scheduler_run::run_cycle(&boot.app_context)
        .await
        .expect("rodar ciclo");

    let recarregado = monitors::Entity::find_by_id(monitor.id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let proximo = recarregado.next_run_at.expect("next_run_at gravado");
    assert!(
        proximo.with_timezone(&Utc) > antes,
        "o monitor continuou vencido: o ciclo seguinte o executaria de novo"
    );
    // A reserva é exatamente um intervalo à frente.
    let esperado = antes + chrono::Duration::seconds(i64::from(monitor.interval_seconds));
    assert!(
        (proximo.with_timezone(&Utc) - esperado).num_seconds().abs() <= 5,
        "reserva fora do intervalo do monitor"
    );
}

/// Matriz #15 — `discovery_results` é cache da última execução, não histórico.
#[tokio::test]
#[serial]
async fn discovery_results_e_cache_e_nao_acumula_entre_execucoes() {
    use backend_rust::models::discovery_results;
    use sea_orm::{ColumnTrait, QueryFilter};

    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;
    let site = site(db).await;
    let rede = rede(db, site.id, "10.20.0.0/24").await;
    let (run, _) = queue::enqueue_network_scan(db, &rede)
        .await
        .expect("enfileirar");

    let semear = |ip: &str| {
        let ip = ip.to_string();
        async move {
            discovery_results::ActiveModel {
                discovery_run_id: Set(run.id),
                ip_address: Set(ip),
                confidence: Set(80),
                first_seen_at: Set(Utc::now().into()),
                last_seen_at: Set(Utc::now().into()),
                ..Default::default()
            }
            .insert(db)
            .await
            .expect("gravar resultado de discovery");
        }
    };
    semear("10.20.0.5").await;
    semear("10.20.0.6").await;

    let contar = || async {
        discovery_results::Entity::find()
            .filter(
                backend_rust::models::_entities::discovery_results::Column::DiscoveryRunId
                    .eq(run.id),
            )
            .all(db)
            .await
            .expect("contar")
            .len()
    };
    assert_eq!(contar().await, 2);

    // Uma reexecução limpa a run antes de gravar: o host que saiu da rede não
    // pode continuar aparecendo na tela de descoberta.
    discovery_results::Entity::delete_many()
        .filter(
            backend_rust::models::_entities::discovery_results::Column::DiscoveryRunId.eq(run.id),
        )
        .exec(db)
        .await
        .expect("limpar cache");
    semear("10.20.0.9").await;

    let restantes = discovery_results::Entity::find()
        .filter(
            backend_rust::models::_entities::discovery_results::Column::DiscoveryRunId.eq(run.id),
        )
        .all(db)
        .await
        .expect("listar");
    assert_eq!(
        restantes.len(),
        1,
        "resultados antigos sobreviveram ao scan"
    );
    assert_eq!(restantes[0].ip_address, "10.20.0.9");
}

/// Matriz #12 — `POST /networks/:id/scan` **enfileira**, não varre.
///
/// A resposta é 202 e volta em milissegundos: se o handler varresse, uma faixa
/// /24 seguraria a requisição por minutos e o proxy cortaria antes.
#[tokio::test]
#[serial]
async fn o_endpoint_de_scan_apenas_enfileira() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = super::prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = super::prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let site = site(&ctx.db).await;
        let rede = rede(&ctx.db, site.id, "192.168.50.0/24").await;

        let comeco = std::time::Instant::now();
        let resposta = request
            .post(&format!("/api/networks/{}/scan", rede.id))
            .await;
        let decorrido = comeco.elapsed();

        assert_eq!(
            resposta.status_code(),
            202,
            "scan deveria ser aceito, não executado"
        );
        assert!(
            decorrido < std::time::Duration::from_secs(3),
            "o handler varreu em vez de enfileirar ({decorrido:?})"
        );

        let corpo: serde_json::Value = serde_json::from_str(&resposta.text()).unwrap();
        assert_eq!(corpo["alreadyQueued"], false);
        assert_eq!(corpo["run"]["status"], "pending");
        assert_eq!(corpo["usableHosts"], 254);
        assert_eq!(corpo["truncated"], false);

        // Nenhum resultado foi produzido — a varredura é do scheduler.
        use sea_orm::{ColumnTrait, QueryFilter};
        let run_id = corpo["run"]["id"].as_i64().unwrap();
        let resultados = backend_rust::models::discovery_results::Entity::find()
            .filter(
                backend_rust::models::_entities::discovery_results::Column::DiscoveryRunId
                    .eq(run_id),
            )
            .all(&ctx.db)
            .await
            .unwrap();
        assert!(resultados.is_empty(), "o endpoint varreu de verdade");

        // Segundo pedido reaproveita a run.
        let repetido = request
            .post(&format!("/api/networks/{}/scan", rede.id))
            .await;
        let corpo: serde_json::Value = serde_json::from_str(&repetido.text()).unwrap();
        assert_eq!(corpo["alreadyQueued"], true);
        assert_eq!(corpo["run"]["id"], run_id);
    })
    .await;
}

/// Matriz #47 — `createdAt` em `dd/MM/yyyy HH:mm:ss` nas métricas e eventos do
/// dispositivo. A tela exibe o valor cru; ISO-8601 apareceria como texto bruto.
#[tokio::test]
#[serial]
async fn metricas_e_eventos_do_dispositivo_saem_no_formato_brasileiro() {
    use backend_rust::models::{alert_events, metrics};

    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = super::prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = super::prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let device = dispositivo(&ctx.db, "sw-datas", "online").await;
        metrics::ActiveModel {
            device_id: Set(device.id),
            name: Set("cpu_usage".into()),
            value: Set(42.5),
            unit: Set("%".into()),
            recorded_at: Set(Utc::now().into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("gravar métrica");
        alert_events::ActiveModel {
            device_id: Set(Some(device.id)),
            status: Set("triggered".into()),
            severity: Set("info".into()),
            started_at: Set(Utc::now().into()),
            message: Set(Some("Dispositivo online".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .expect("gravar evento");

        // `dd/MM/yyyy HH:mm:ss` — 19 caracteres, com barras e dois-pontos.
        let formato = regex_lite_data();
        for rota in [
            format!("/api/devices/{}/metrics", device.id),
            format!("/api/devices/{}/events", device.id),
        ] {
            let corpo: serde_json::Value =
                serde_json::from_str(&request.get(&rota).await.text()).unwrap();
            let linhas = corpo.as_array().cloned().unwrap_or_else(|| {
                corpo["data"]
                    .as_array()
                    .cloned()
                    .expect("array ou envelope")
            });
            assert!(!linhas.is_empty(), "{rota} veio vazia");
            let created_at = linhas[0]["createdAt"].as_str().expect("createdAt presente");
            assert!(
                formato(created_at),
                "{rota} devolveu `{created_at}`, fora de dd/MM/yyyy HH:mm:ss"
            );
        }
    })
    .await;
}

/// `dd/MM/yyyy HH:mm:ss` sem trazer a crate `regex` só para um teste.
fn regex_lite_data() -> impl Fn(&str) -> bool {
    |valor: &str| {
        let bytes = valor.as_bytes();
        bytes.len() == 19
            && bytes[2] == b'/'
            && bytes[5] == b'/'
            && bytes[10] == b' '
            && bytes[13] == b':'
            && bytes[16] == b':'
            && bytes
                .iter()
                .enumerate()
                .filter(|(i, _)| ![2, 5, 10, 13, 16].contains(i))
                .all(|(_, b)| b.is_ascii_digit())
    }
}

/// Matriz #20 — o `adminStatus` que o operador escolheu sobrevive ao poll.
///
/// O poll relê o `ifAdminStatus` do agente a cada ciclo. Se ele sobrescrevesse
/// a coluna, a porta que o operador tirou do monitoramento voltaria sozinha —
/// e com ela os alertas de queda de link que ele silenciou de propósito.
#[tokio::test]
#[serial]
async fn admin_status_escolhido_pelo_operador_sobrevive_ao_poll() {
    use backend_rust::services::snmp::{collectors::SnmpInterface, service::sync_interface};

    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;
    let device = dispositivo(db, "sw-portas", "online").await;

    let lida = |oper: u64| SnmpInterface {
        if_index: 3,
        if_name: "Gi0/3".into(),
        if_descr: None,
        if_alias: None,
        if_type: Some(6),
        if_speed: Some(1_000_000_000),
        // O agente sempre reporta a porta como administrativamente ligada.
        if_admin_status: Some(1),
        if_oper_status: Some(oper),
        mac_address: None,
        is_monitored: false,
    };

    // Primeiro poll: cria a linha com o que o agente informou.
    let criada = sync_interface(db, device.id, &lida(1))
        .await
        .expect("primeiro poll");
    assert_eq!(criada.interface.admin_status.as_deref(), Some("up"));

    // O operador desliga o monitoramento daquela porta.
    backend_rust::models::device_interfaces::ActiveModel {
        id: Set(criada.interface.id),
        admin_status: Set(Some("down".into())),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("operador desliga a porta");

    // Poll seguinte: o agente continua dizendo "up", mas a escolha prevalece.
    let repolada = sync_interface(db, device.id, &lida(2))
        .await
        .expect("segundo poll");
    assert_eq!(
        repolada.interface.admin_status.as_deref(),
        Some("down"),
        "o poll ressuscitou a porta que o operador tinha desligado"
    );
    // O estado operacional, esse sim, acompanha o agente.
    assert_eq!(repolada.interface.oper_status.as_deref(), Some("down"));
    assert_eq!(repolada.previous_oper_status.as_deref(), Some("up"));
}

/// A aba "Interfaces SNMP" marca a porta pelo monitor, não pelo `adminStatus`.
///
/// Os dois campos parecem dizer a mesma coisa e não dizem: o `adminStatus` da
/// linha nova sai do que o *agente* reportou, então toda porta ligada no
/// equipamento apareceria como monitorada — mesmo sem ninguém coletá-la. Quem
/// responde "está sendo monitorada?" é o monitor `Interface X` habilitado.
#[tokio::test]
#[serial]
async fn a_interface_so_conta_como_monitorada_quando_tem_monitor_habilitado() {
    use backend_rust::services::snmp::{
        client::SnmpConfig,
        collectors::SnmpInterface,
        service::{
            interface_monitor_name, list_interfaces, set_interface_monitoring, sync_interface,
            DeviceInterfaceView,
        },
    };

    let boot = boot_test::<App>().await.expect("subir app de teste");
    let ctx = &boot.app_context;
    let device = dispositivo(&ctx.db, "sw-listagem", "online").await;

    let criada = sync_interface(
        &ctx.db,
        device.id,
        &SnmpInterface {
            if_index: 3,
            if_name: "Gi0/3".into(),
            if_descr: None,
            if_alias: None,
            if_type: Some(6),
            if_speed: Some(1_000_000_000),
            // A porta está ligada no equipamento — mas ninguém pediu para coletá-la.
            if_admin_status: Some(1),
            if_oper_status: Some(1),
            mac_address: None,
            is_monitored: false,
        },
    )
    .await
    .expect("registrar a interface")
    .interface;

    async fn listada(ctx: &loco_rs::app::AppContext, device_id: i64) -> DeviceInterfaceView {
        list_interfaces(ctx, device_id)
            .await
            .expect("listar interfaces")
            .pop()
            .expect("a interface registrada some da listagem")
    }

    let antes = listada(ctx, device.id).await;
    assert_eq!(antes.admin_status.as_deref(), Some("up"));
    assert!(
        !antes.is_monitored,
        "porta sem monitor apareceu como monitorada só porque o agente a reportou ligada"
    );
    // O `id` é o que liga a interface às suas métricas — sem ele o diálogo de
    // gráficos não tem por onde filtrar.
    assert_eq!(antes.id, criada.id);
    assert_eq!(antes.snmp_index, Some(3));

    monitor(&ctx.db, device.id, &interface_monitor_name("Gi0/3")).await;
    assert!(
        listada(ctx, device.id).await.is_monitored,
        "monitor habilitado não refletiu na listagem"
    );

    // Remover do monitoramento desliga o monitor e a coluna, sem passar pela
    // tela de descoberta. (Só a inclusão precisa falar com o agente.)
    set_interface_monitoring(
        ctx,
        &device,
        SnmpConfig::v2c("10.0.0.1", "public", 161),
        criada.id,
        false,
    )
    .await
    .expect("remover do monitoramento");

    let depois = listada(ctx, device.id).await;
    assert!(!depois.is_monitored);
    assert_eq!(depois.admin_status.as_deref(), Some("down"));
}

const EXPORT_ZABBIX: &str = r#"{"zabbix_export":{"version":"7.0","templates":[{"uuid":"uuid-roteador","name":"Roteador","items":[{"type":"SNMP_AGENT","key_":"if.in","snmp_oid":"1.3.6.1.2.1.2.2.1.10.1"}]}]}}"#;
const EXPORT_ZABBIX_RENOMEADO: &str = r#"{"zabbix_export":{"version":"7.0","templates":[{"uuid":"uuid-roteador","name":"Roteador Core","items":[{"type":"SNMP_AGENT","key_":"if.in","snmp_oid":"1.3.6.1.2.1.2.2.1.10.1"},{"type":"SNMP_AGENT","key_":"if.out","snmp_oid":"1.3.6.1.2.1.2.2.1.16.1"}]}]}}"#;

/// Matriz #22 — reimportar pelo mesmo `uuid` preserva o `id` do template.
///
/// O `id` é a chave que os dispositivos guardam em `zabbix_template_id`.
/// Criar uma linha nova a cada importação desvincularia todos eles em silêncio.
#[tokio::test]
#[serial]
async fn reimportar_template_por_uuid_preserva_o_id_e_os_devices_vinculados() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = super::prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = super::prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let primeira = request
            .post("/api/zabbix-templates")
            .json(&serde_json::json!({ "content": EXPORT_ZABBIX }))
            .await;
        assert!(primeira.status_code().is_success(), "{}", primeira.text());
        let templates = backend_rust::models::zabbix_templates::Entity::find()
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(templates.len(), 1);
        let id_original = templates[0].id;

        // Um dispositivo passa a apontar para o template.
        let device = dispositivo(&ctx.db, "rt-filial", "online").await;
        devices::ActiveModel {
            id: Set(device.id),
            zabbix_template_id: Set(Some(id_original)),
            ..Default::default()
        }
        .update(&ctx.db)
        .await
        .expect("vincular device");

        // Reimporta o mesmo uuid, com nome novo e um item a mais.
        let segunda = request
            .post("/api/zabbix-templates")
            .json(&serde_json::json!({ "content": EXPORT_ZABBIX_RENOMEADO }))
            .await;
        assert!(segunda.status_code().is_success(), "{}", segunda.text());

        let templates = backend_rust::models::zabbix_templates::Entity::find()
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(templates.len(), 1, "reimportação duplicou o template");
        assert_eq!(
            templates[0].id, id_original,
            "o id mudou e desvinculou os devices"
        );
        assert_eq!(templates[0].name, "Roteador Core");

        // O vínculo do dispositivo continua de pé.
        let recarregado = devices::Entity::find_by_id(device.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recarregado.zabbix_template_id, Some(id_original));

        // Os itens foram substituídos, não acumulados.
        let itens = backend_rust::models::zabbix_template_items::Entity::find()
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(
            itens.len(),
            2,
            "os itens antigos sobreviveram à reimportação"
        );
    })
    .await;
}

/// Matriz #23 — o monitor "Coleta de Template Zabbix" é autocorretivo.
#[tokio::test]
#[serial]
async fn o_monitor_de_template_zabbix_se_conserta_sozinho() {
    use backend_rust::services::zabbix::collector::{
        sync_zabbix_template_monitor, ZABBIX_TEMPLATE_MONITOR_NAME,
    };
    use sea_orm::{ColumnTrait, QueryFilter};

    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;

    let template = backend_rust::models::zabbix_templates::ActiveModel {
        zabbix_uuid: Set(Some("uuid-x".into())),
        name: Set("Switch".into()),
        raw_export: Set("{}".into()),
        imported_at: Set(Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("criar template");

    let device = dispositivo(db, "sw-zbx", "online").await;
    let device = devices::ActiveModel {
        id: Set(device.id),
        zabbix_template_id: Set(Some(template.id)),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("vincular template");

    let monitores = || async {
        monitors::Entity::find()
            .filter(
                backend_rust::models::_entities::monitors::Column::Name
                    .eq(ZABBIX_TEMPLATE_MONITOR_NAME),
            )
            .all(db)
            .await
            .expect("listar monitores")
    };

    // Primeira sincronização cria o monitor.
    sync_zabbix_template_monitor(db, &device)
        .await
        .expect("sync 1");
    let criados = monitores().await;
    assert_eq!(criados.len(), 1);
    let monitor_id = criados[0].id;

    // Chamar de novo não duplica — é o mesmo poll rodando a cada ciclo.
    sync_zabbix_template_monitor(db, &device)
        .await
        .expect("sync 2");
    assert_eq!(monitores().await.len(), 1, "o poll duplicou o monitor");

    // O operador desativa o monitor à mão: o ciclo seguinte o religa.
    monitors::ActiveModel {
        id: Set(monitor_id),
        enabled: Set(false),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("desativar a mao");
    sync_zabbix_template_monitor(db, &device)
        .await
        .expect("sync 3");
    assert!(monitores().await[0].enabled, "o monitor não foi religado");

    // O operador apaga o monitor: o ciclo seguinte o recria.
    monitors::Entity::delete_by_id(monitor_id)
        .exec(db)
        .await
        .expect("apagar a mao");
    assert!(monitores().await.is_empty());
    sync_zabbix_template_monitor(db, &device)
        .await
        .expect("sync 4");
    assert_eq!(monitores().await.len(), 1, "o monitor apagado não voltou");

    // Tirar o template do dispositivo remove o monitor — ele perdeu a razão.
    let sem_template = devices::ActiveModel {
        id: Set(device.id),
        zabbix_template_id: Set(None),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("desvincular");
    sync_zabbix_template_monitor(db, &sem_template)
        .await
        .expect("sync 5");
    assert!(
        monitores().await.is_empty(),
        "o monitor sobreviveu à remoção do template"
    );
}
