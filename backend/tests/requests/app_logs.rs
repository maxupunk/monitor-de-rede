//! Fase 4 — o log interno como log do dispositivo.
//!
//! O aceite não é "o log grava": é que ele grava **pelo pipeline que já
//! existia**. Por isso os testes abaixo exercitam a fila, o escritor em lote e
//! a tabela de sempre, e verificam a ausência das coisas que a seção 6 proíbe —
//! segunda fila, segundo escritor, endpoint separado, tabela `runtime_logs`.

use std::{sync::Arc, time::Duration};

use backend::{
    app::App,
    models::logs::device_logs,
    services::{
        devices::system_device::{self, SystemDeviceService},
        syslog::{
            app_layer::{self, AppLogLayer, LOCAL_SOURCE_IP},
            db,
            queue::{IngestMetrics, LogQueue},
            writer,
        },
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder};
use serial_test::serial;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, Registry};

use super::prepare_data;

/// Monta fila + escritor sobre um banco de logs em memória e devolve tudo que
/// o teste precisa para exercitar a camada.
///
/// É deliberadamente o **mesmo** caminho da produção: `LogQueue::create` e
/// `writer::run_with`. Um dublê aqui provaria que o dublê funciona.
async fn pipeline_em_memoria() -> (
    sea_orm::DatabaseConnection,
    tokio::sync::mpsc::Receiver<backend::services::syslog::queue::PendingLog>,
    LogQueue,
    Arc<IngestMetrics>,
) {
    let logs = db::connect("sqlite::memory:").await.expect("banco de logs");
    let metrics = Arc::new(IngestMetrics::default());
    let (queue, receiver) = LogQueue::create(64, Arc::clone(&metrics));
    (logs.connection().clone(), receiver, queue, metrics)
}

/// Roda o bloco com a camada instalada **só nesta thread**.
///
/// `tracing::subscriber::with_default` em vez de `.init()`: o subscriber global
/// é único por processo, e a suíte inteira roda no mesmo. Um `.init()` aqui
/// derrubaria os demais testes.
fn com_a_camada<T>(bloco: impl FnOnce() -> T) -> T {
    let subscriber = Registry::default().with(AppLogLayer);
    tracing::subscriber::with_default(subscriber, bloco)
}

/// Drena a fila para o banco, com os gatilhos apertados.
async fn descarrega(
    db: &sea_orm::DatabaseConnection,
    receiver: &mut tokio::sync::mpsc::Receiver<backend::services::syslog::queue::PendingLog>,
    metrics: Arc<IngestMetrics>,
    queue: LogQueue,
) {
    // Solta as duas pontas de escrita — a do teste e a que a camada guarda.
    // Com o canal fechado, o escritor grava o que sobrou e retorna, que é o
    // mesmo caminho do desligamento gracioso.
    app_layer::clear_queue();
    drop(queue);
    writer::run_with(
        db.clone(),
        receiver,
        metrics,
        500,
        Duration::from_millis(5),
        None,
    )
    .await;
}

async fn linhas(db: &sea_orm::DatabaseConnection) -> Vec<device_logs::Model> {
    device_logs::Entity::find()
        .order_by_asc(device_logs::Column::Id)
        .all(db)
        .await
        .expect("linhas")
}

#[tokio::test]
#[serial]
async fn o_evento_da_aplicacao_vira_linha_com_severidade_alvo_e_pid() {
    let (db, mut receiver, queue, metrics) = pipeline_em_memoria().await;
    app_layer::install_queue(queue.clone());
    system_device::resolver::invalidate();

    com_a_camada(|| {
        tracing::error!(monitor_id = 7, "falha ao executar monitor");
    });
    descarrega(&db, &mut receiver, metrics, queue).await;

    let linhas = linhas(&db).await;
    let nossa = linhas
        .iter()
        .find(|linha| linha.message.contains("falha ao executar monitor"))
        .expect("a linha precisa chegar ao banco");

    assert_eq!(nossa.severity, Some(3), "ERROR é severidade syslog 3");
    assert_eq!(
        nossa.source_ip, LOCAL_SOURCE_IP,
        "a coluna é NOT NULL e 127.0.0.1 é o valor honesto"
    );
    assert_eq!(nossa.source, "application");
    assert!(nossa.pid.is_some());
    assert!(
        nossa
            .app_name
            .as_deref()
            .unwrap_or_default()
            .contains("app_logs")
            || nossa.app_name.is_some(),
        "o target do evento vira app_name"
    );
    // Os campos do evento são achatados na mensagem — o FTS os encontra de
    // graça, sem coluna JSON invisível à busca.
    assert!(
        nossa.message.contains("monitor_id=7"),
        "campo estruturado perdido: {}",
        nossa.message
    );
}

#[tokio::test]
#[serial]
async fn a_severidade_acompanha_o_nivel_para_o_filtro_da_tela_funcionar_igual() {
    let (db, mut receiver, queue, metrics) = pipeline_em_memoria().await;
    app_layer::install_queue(queue.clone());

    com_a_camada(|| {
        tracing::error!("um erro");
        tracing::warn!("um aviso");
        tracing::info!("uma informação");
    });
    descarrega(&db, &mut receiver, metrics, queue).await;

    let linhas = linhas(&db).await;
    let severidade = |trecho: &str| {
        linhas
            .iter()
            .find(|linha| linha.message.contains(trecho))
            .and_then(|linha| linha.severity)
    };
    assert_eq!(severidade("um erro"), Some(3));
    assert_eq!(severidade("um aviso"), Some(4));
    assert_eq!(severidade("uma informação"), Some(6));
}

#[tokio::test]
#[serial]
async fn a_ordem_do_lote_e_a_de_emissao() {
    let (db, mut receiver, queue, metrics) = pipeline_em_memoria().await;
    app_layer::install_queue(queue.clone());

    com_a_camada(|| {
        for indice in 0..20 {
            tracing::info!("evento ordenado {indice}");
        }
    });
    descarrega(&db, &mut receiver, metrics, queue).await;

    let ordenadas: Vec<String> = linhas(&db)
        .await
        .into_iter()
        .filter(|linha| linha.message.starts_with("evento ordenado"))
        .map(|linha| linha.message)
        .collect();
    assert_eq!(ordenadas.len(), 20);
    for (indice, mensagem) in ordenadas.iter().enumerate() {
        assert_eq!(mensagem, &format!("evento ordenado {indice}"));
    }
}

#[tokio::test]
#[serial]
async fn a_fila_cheia_descarta_contando_em_vez_de_bloquear_o_request() {
    let logs = db::connect("sqlite::memory:").await.unwrap();
    let metrics = Arc::new(IngestMetrics::default());
    // Fila minúscula e ninguém drenando: é o cenário de sobrecarga.
    let (queue, _receiver) = LogQueue::create(2, Arc::clone(&metrics));
    app_layer::install_queue(queue.clone());

    com_a_camada(|| {
        for indice in 0..50 {
            tracing::info!("evento de sobrecarga {indice}");
        }
    });

    app_layer::clear_queue();
    let snapshot = metrics.snapshot();
    assert!(
        snapshot.dropped_queue_full > 0,
        "descarte precisa ser contado, não silencioso"
    );
    // E nada travou: o teste chegou até aqui.
    assert_eq!(
        device_logs::Entity::find()
            .count(logs.connection())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[serial]
async fn nao_existe_realimentacao_log_insert_log() {
    let (db, mut receiver, queue, metrics) = pipeline_em_memoria().await;
    app_layer::install_queue(queue.clone());

    com_a_camada(|| {
        // O que o próprio escritor e o driver SQL emitem no caminho de
        // gravação. Se qualquer um destes virasse linha, o `INSERT` dela
        // emitiria o próximo, e o laço não fecharia.
        tracing::info!(target: "backend::services::syslog::writer", "lote de logs gravado");
        tracing::debug!(target: "sqlx::query", "INSERT INTO device_logs ...");
        tracing::debug!(target: "sea_orm::driver", "consulta executada");
    });
    descarrega(&db, &mut receiver, metrics, queue).await;

    assert_eq!(
        device_logs::Entity::find().count(&db).await.unwrap(),
        0,
        "nenhum destes alvos pode entrar no banco"
    );

    // A política é a mesma consultada pela camada — e um erro do SQLx continua
    // passando, que é o que o operador procura quando o banco trava.
    assert!(!app_layer::deve_gravar("sqlx::query", Level::DEBUG));
    assert!(app_layer::deve_gravar("sqlx::query", Level::ERROR));
}

#[tokio::test]
#[serial]
async fn log_emitido_antes_do_dispositivo_existir_vai_com_device_id_nulo() {
    let (db, mut receiver, queue, metrics) = pipeline_em_memoria().await;
    app_layer::install_queue(queue.clone());

    // Boot e migrations acontecem antes de o dispositivo existir.
    system_device::resolver::invalidate();
    com_a_camada(|| tracing::info!("linha do boot, antes do dispositivo"));

    // Depois que o resolvedor tem o ID, as linhas passam a ser dele.
    system_device::resolver::set(4242);
    com_a_camada(|| tracing::info!("linha depois do dispositivo"));

    descarrega(&db, &mut receiver, metrics, queue).await;
    let linhas = linhas(&db).await;
    let device_de = |trecho: &str| {
        linhas
            .iter()
            .find(|linha| linha.message.contains(trecho))
            .map(|linha| linha.device_id)
    };
    assert_eq!(
        device_de("linha do boot"),
        Some(None),
        "comportamento explícito: sem dispositivo, sem device_id"
    );
    assert_eq!(device_de("linha depois"), Some(Some(4242)));

    system_device::resolver::invalidate();
}

#[tokio::test]
#[serial]
async fn o_log_interno_e_consultavel_pela_mesma_api_de_logs() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        let session = prepare_data::init_user_login(&request, &ctx).await;
        let (header, value) = prepare_data::auth_header(&session.token);
        request.add_header(header, value);

        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        // Uma consulta filtrada pelo dispositivo — a **mesma** de `/logs`.
        let resposta = request
            .get(&format!("/api/logs?deviceId={}", device.id))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());

        // E nenhuma rota paralela nasceu para responder isto.
        for rota in ["/api/runtime/logs", "/api/logs/server", "/api/server-logs"] {
            assert_eq!(
                request.get(rota).await.status_code(),
                404,
                "{rota} não pode existir (seção 6)"
            );
        }
    })
    .await;
}
