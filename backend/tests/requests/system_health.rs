//! Fase 2 — a saúde do servidor pelo pipeline normal de monitoramento.
//!
//! O aceite desta fase não é "o coletor funciona": é que **nada** de novo
//! precisou ser inventado para consultá-lo. Os testes abaixo, portanto, falam
//! sempre pelos endpoints genéricos — `/api/devices/{id}/monitors`,
//! `/api/devices/{id}/metrics`, `/api/monitors/{id}` — e nunca por uma rota do
//! servidor, porque essa rota não existe e não pode passar a existir.

use backend::{
    app::App,
    models::{_entities::metrics, monitors},
    services::{
        devices::system_device::SystemDeviceService,
        monitoring::{
            health::series,
            managed::{ensure_system_health_monitor, SYSTEM_HEALTH},
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
        },
    },
};
use loco_rs::testing::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::Value;
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &loco_rs::app::AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

/// Garante dispositivo + monitor e devolve os dois ids.
async fn provisionado(ctx: &loco_rs::app::AppContext) -> (i64, i64) {
    let device = SystemDeviceService::new(&ctx.db)
        .ensure()
        .await
        .expect("dispositivo do sistema");
    let monitor = ensure_system_health_monitor(&ctx.db, device.id)
        .await
        .expect("monitor de saúde");
    (device.id, monitor.id)
}

#[tokio::test]
#[serial]
async fn o_monitor_gerenciado_nasce_sem_probe_e_sem_retry() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (device_id, monitor_id) = provisionado(&ctx).await;
        let monitor = monitors::Entity::find_by_id(monitor_id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(monitor.device_id, Some(device_id));
        assert_eq!(monitor.r#type, SYSTEM_HEALTH);
        assert!(
            monitor.probe_id.is_none(),
            "com probe o execute_one mediria a saúde do probe, não a do servidor"
        );
        assert_eq!(
            monitor.retry_count, 0,
            "reler /proc quatro vezes não confirma um down"
        );
        assert!(monitor.enabled);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_provisionamento_e_idempotente_e_conserta_linha_fora_das_invariantes() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (device_id, monitor_id) = provisionado(&ctx).await;
        let segundo = ensure_system_health_monitor(&ctx.db, device_id)
            .await
            .unwrap();
        assert_eq!(segundo.id, monitor_id, "o segundo boot duplicou o monitor");
        assert_eq!(
            monitors::Entity::find()
                .filter(monitors::Column::Type.eq(SYSTEM_HEALTH))
                .count(&ctx.db)
                .await
                .unwrap(),
            1
        );

        // Uma linha herdada com retry e probe é reconduzida, não aceita.
        use sea_orm::{ActiveModelTrait, Set};
        let mut ativo: monitors::ActiveModel = segundo.into();
        ativo.retry_count = Set(3);
        ativo.update(&ctx.db).await.unwrap();

        let corrigido = ensure_system_health_monitor(&ctx.db, device_id)
            .await
            .unwrap();
        assert_eq!(corrigido.retry_count, 0);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_runner_conhece_o_tipo_e_nao_devolve_tipo_desconhecido() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let resultado = run_monitor(
            &ctx,
            SYSTEM_HEALTH,
            &serde_json::json!({}),
            RunOptions::default(),
        )
        .await
        .expect("o agendador precisa conhecer o tipo, senão erra a cada ciclo");

        assert!(resultado.message.is_some());
        assert!(resultado.data["sources"].is_object());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn um_ciclo_publica_monitor_results_e_metrics_pelo_caminho_generico() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let (device_id, monitor_id) = provisionado(&ctx).await;

        // Duas execuções: a primeira estabelece a linha de base dos contadores
        // acumulados (CPU e tráfego), a segunda já mede.
        for _ in 0..2 {
            let resultado = run_monitor(
                &ctx,
                SYSTEM_HEALTH,
                &serde_json::json!({}),
                RunOptions::default(),
            )
            .await
            .unwrap();
            process_result(&ctx, monitor_id, &resultado, None)
                .await
                .unwrap()
                .expect("a observação foi gravada");
        }

        // Só as séries de dispositivo entram em `metrics` — e o `monitor_id`
        // preserva qual checagem as produziu.
        let series_gravadas: Vec<metrics::Model> = metrics::Entity::find()
            .filter(metrics::Column::DeviceId.eq(device_id))
            .all(&ctx.db)
            .await
            .unwrap();
        for linha in &series_gravadas {
            assert_eq!(linha.monitor_id, Some(monitor_id));
            assert!(!linha.unit.is_empty(), "{} sem unidade", linha.name);
        }

        // Latência e perda **não** são copiadas para `metrics` (§3.1).
        assert!(
            !series_gravadas
                .iter()
                .any(|linha| linha.name == "latency" || linha.name == "packet_loss"),
            "copiar latência para metrics multiplica a tabela sem acrescentar informação"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_saude_e_consultavel_pelos_endpoints_comuns_de_dispositivo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let (device_id, monitor_id) = provisionado(&ctx).await;

        for _ in 0..2 {
            let resultado = run_monitor(
                &ctx,
                SYSTEM_HEALTH,
                &serde_json::json!({}),
                RunOptions::default(),
            )
            .await
            .unwrap();
            process_result(&ctx, monitor_id, &resultado, None)
                .await
                .unwrap();
        }

        // O monitor aparece na listagem comum do dispositivo.
        let resposta = request
            .get(&format!("/api/devices/{device_id}/monitors"))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let lista: Value = serde_json::from_str(&resposta.text()).unwrap();
        assert!(
            lista
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["type"] == SYSTEM_HEALTH),
            "a coleta de saúde precisa aparecer entre os monitores do dispositivo"
        );

        // E as séries, pelo endpoint de métricas que já existia.
        let resposta = request
            .get(&format!("/api/devices/{device_id}/metrics"))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());

        // Nenhum endpoint paralelo nasceu para responder isto.
        assert_eq!(
            request.get("/api/runtime/health").await.status_code(),
            404,
            "a seção 6 proíbe /api/runtime/*"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_monitor_gerenciado_recusa_troca_de_tipo_alvo_e_desativacao() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let (_device_id, monitor_id) = provisionado(&ctx).await;
        let url = format!("/api/monitors/{monitor_id}");

        for corpo in [
            serde_json::json!({"type": "ping"}),
            serde_json::json!({"probeId": 1}),
            serde_json::json!({"enabled": false}),
            serde_json::json!({"configuration": {"host": "8.8.8.8"}}),
        ] {
            let resposta = request.put(&url).json(&corpo).await;
            assert_eq!(
                resposta.status_code(),
                400,
                "deveria ser recusado por regra de negócio: {corpo}"
            );
        }

        // Ajustar o intervalo continua permitido.
        let resposta = request
            .put(&url)
            .json(&serde_json::json!({"intervalSeconds": 60}))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        assert_eq!(
            monitors::Entity::find_by_id(monitor_id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .interval_seconds,
            60
        );

        // Excluir, não.
        assert_eq!(request.delete(&url).await.status_code(), 400);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_tipo_gerenciado_nao_pode_ser_criado_pela_api() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let resposta = request
            .post("/api/monitors")
            .json(&serde_json::json!({
                "name": "meu system_health", "type": SYSTEM_HEALTH, "configuration": {}
            }))
            .await;
        assert_eq!(
            resposta.status_code(),
            422,
            "o tipo é interno: quem o provisiona é o sistema"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn renomear_o_servidor_nao_cria_um_monitor_de_ping_para_o_proprio_nome() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let (device_id, _) = provisionado(&ctx).await;

        let resposta = request
            .put(&format!("/api/devices/{device_id}"))
            .json(&serde_json::json!({"name": "Servidor da matriz"}))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());

        let pings = monitors::Entity::find()
            .filter(monitors::Column::DeviceId.eq(device_id))
            .filter(monitors::Column::Type.eq("ping"))
            .count(&ctx.db)
            .await
            .unwrap();
        assert_eq!(
            pings, 0,
            "um ping para o nome exibido só poderia falhar e deixaria o servidor offline para sempre"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn as_series_de_dispositivo_sao_uma_lista_fechada() {
    // A fronteira de §3.1 é o que segura o volume do banco. O teste é sobre a
    // lista, e não sobre o coletor: um checker novo não passa a escrever em
    // `metrics` sem que alguém edite `DEVICE_SERIES`.
    use backend::services::monitoring::result_processor::is_device_series;

    for nome in [
        series::CPU_USAGE,
        series::MEMORY_USAGE,
        series::MEMORY_USED_BYTES,
        series::MEMORY_TOTAL_BYTES,
        series::STORAGE_USAGE,
        series::LOAD_AVERAGE_1M,
        series::PROCESS_MEMORY_BYTES,
        series::UPTIME_SECONDS,
        series::IN_BPS,
        series::OUT_BPS,
    ] {
        assert!(is_device_series(nome), "{nome} deveria ser série do device");
    }
    for nome in ["latency", "packet_loss", "response_time", "connect_time"] {
        assert!(
            !is_device_series(nome),
            "{nome} pertence a monitor_results e não pode ser copiado"
        );
    }
}
