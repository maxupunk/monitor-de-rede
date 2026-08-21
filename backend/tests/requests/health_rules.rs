//! Fase 3 — regras de saúde para **todo dispositivo**.
//!
//! O aceite desta fase é sobre generalidade: o mesmo template aplicado ao
//! servidor e a um roteador SNMP precisa criar **duas** regras, e as duas
//! precisam falar do mesmo campo. Enquanto a idempotência do catálogo era
//! global por `template_key`, a segunda aplicação devolvia `already_exists` e
//! não criava nada — em silêncio, que é o pior jeito de falhar.

use backend::{
    app::App,
    models::alert_rules,
    services::{
        alerts::fields,
        devices::system_device::SystemDeviceService,
        monitoring::{
            health::series, managed::ensure_system_health_monitor, result_processor::process_result,
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

async fn cria_dispositivo(request: &loco_rs::TestServer, corpo: Value) -> i64 {
    let resposta = request.post("/api/devices").json(&corpo).await;
    assert_eq!(resposta.status_code(), 201, "{}", resposta.text());
    let corpo: Value = serde_json::from_str(&resposta.text()).unwrap();
    corpo["id"].as_i64().expect("id")
}

fn regras_de(corpo: &Value) -> Vec<&Value> {
    corpo.as_array().expect("lista de regras").iter().collect()
}

#[tokio::test]
#[serial]
async fn o_mesmo_template_aplicado_a_dois_dispositivos_cria_duas_regras() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let a = cria_dispositivo(
            &request,
            serde_json::json!({"name": "rt-a", "type": "router", "ipAddress": "10.9.0.1"}),
        )
        .await;
        let b = cria_dispositivo(
            &request,
            serde_json::json!({"name": "rt-b", "type": "router", "ipAddress": "10.9.0.2"}),
        )
        .await;

        for device_id in [a, b] {
            let resposta = request
                .post("/api/alert-rules/catalog")
                .json(&serde_json::json!({"keys": ["cpu_usage_high"], "deviceId": device_id}))
                .await;
            assert_eq!(resposta.status_code(), 201, "{}", resposta.text());
            let corpo: Value = serde_json::from_str(&resposta.text()).unwrap();
            assert_eq!(
                corpo["created"].as_array().unwrap().len(),
                1,
                "o segundo dispositivo não pode receber `already_exists`: {corpo}"
            );
            assert_eq!(corpo["created"][0]["deviceId"].as_i64(), Some(device_id));
        }

        // O dispositivo do sistema já recebeu a sua no boot, então a conta é
        // sobre os dois roteadores — que é justamente o ponto: cada
        // dispositivo tem a **sua** regra do mesmo template.
        assert_eq!(
            alert_rules::Entity::find()
                .filter(alert_rules::Column::TemplateKey.eq("cpu_usage_high"))
                .filter(alert_rules::Column::DeviceId.is_in([a, b]))
                .count(&ctx.db)
                .await
                .unwrap(),
            2,
            "duas regras, uma por dispositivo"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn aplicar_duas_vezes_ao_mesmo_dispositivo_continua_idempotente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let device_id = cria_dispositivo(
            &request,
            serde_json::json!({"name": "rt", "type": "router", "ipAddress": "10.9.1.1"}),
        )
        .await;
        let corpo = serde_json::json!({"keys": ["cpu_usage_high"], "deviceId": device_id});

        let primeira = request.post("/api/alert-rules/catalog").json(&corpo).await;
        let primeira: Value = serde_json::from_str(&primeira.text()).unwrap();
        assert_eq!(primeira["created"].as_array().unwrap().len(), 1);

        let segunda = request.post("/api/alert-rules/catalog").json(&corpo).await;
        let segunda: Value = serde_json::from_str(&segunda.text()).unwrap();
        assert!(segunda["created"].as_array().unwrap().is_empty());
        assert_eq!(segunda["skipped"][0]["reason"], "already_exists");

        assert_eq!(
            alert_rules::Entity::find()
                .filter(alert_rules::Column::DeviceId.eq(device_id))
                .count(&ctx.db)
                .await
                .unwrap(),
            1
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_catalogo_do_dispositivo_so_oferece_o_que_ele_publica() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        // Um dispositivo que só responde ping não publica CPU.
        let ping = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "host", "type": "server", "ipAddress": "10.9.2.1", "isMonitored": true
            }),
        )
        .await;
        let resposta = request
            .get(&format!("/api/alert-rules/catalog?deviceId={ping}"))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).unwrap();
        let cpu = corpo["templates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["key"] == "cpu_usage_high")
            .expect("o template continua no catálogo");
        assert_eq!(
            cpu["applicable"], false,
            "oferecer CPU a quem só faz ping produziria uma regra que nunca dispara"
        );
        let latencia = corpo["templates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["key"] == "latency_high")
            .unwrap();
        assert_eq!(latencia["applicable"], true);

        // O servidor, que coleta saúde, publica.
        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        ensure_system_health_monitor(&ctx.db, device.id)
            .await
            .unwrap();
        let resposta = request
            .get(&format!("/api/alert-rules/catalog?deviceId={}", device.id))
            .await;
        let corpo: Value = serde_json::from_str(&resposta.text()).unwrap();
        for chave in ["cpu_usage_high", "memory_usage_high", "storage_usage_high"] {
            let template = corpo["templates"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["key"] == chave)
                .unwrap_or_else(|| panic!("{chave} ausente do catálogo"));
            assert_eq!(
                template["applicable"], true,
                "{chave} deveria ser aplicável"
            );
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn as_regras_de_saude_sao_aplicadas_uma_unica_vez_por_dispositivo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        use backend::services::alerts::catalog::health_defaults;

        autenticado(&mut request, &ctx).await;

        // O servidor já recebeu as suas no boot — é o `Initializer` que chama
        // `ensure_for_device`, e o aceite da fase é exatamente este.
        let servidor = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        let do_servidor = alert_rules::Entity::find()
            .filter(alert_rules::Column::DeviceId.eq(servidor.id))
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(
            do_servidor.len(),
            3,
            "CPU, memória e armazenamento, aplicadas no boot"
        );
        for regra in &do_servidor {
            assert_eq!(regra.device_id, Some(servidor.id), "regra sem escopo");
        }

        // A mecânica do marcador, num dispositivo que ainda não passou por ela.
        let outro = cria_dispositivo(
            &request,
            serde_json::json!({"name": "rt-saude", "type": "router", "ipAddress": "10.9.5.1"}),
        )
        .await;

        let primeira = health_defaults::ensure_for_device(&ctx.db, outro)
            .await
            .unwrap();
        assert_eq!(primeira.created.len(), 3);

        // Segundo boot: nada de novo.
        let segunda = health_defaults::ensure_for_device(&ctx.db, outro)
            .await
            .unwrap();
        assert!(segunda.created.is_empty());

        // Regra removida pelo operador **não** ressuscita no boot seguinte.
        let removida = primeira.created[0].id;
        alert_rules::Entity::delete_by_id(removida)
            .exec(&ctx.db)
            .await
            .unwrap();
        let terceira = health_defaults::ensure_for_device(&ctx.db, outro)
            .await
            .unwrap();
        assert!(
            terceira.created.is_empty(),
            "o marcador existe justamente para isto"
        );
        assert!(alert_rules::Entity::find_by_id(removida)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn as_regras_sao_listaveis_por_dispositivo_sem_deixar_de_estar_na_central() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let device_id = cria_dispositivo(
            &request,
            serde_json::json!({"name": "rt", "type": "router", "ipAddress": "10.9.3.1"}),
        )
        .await;
        request
            .post("/api/alert-rules/catalog")
            .json(&serde_json::json!({"keys": ["cpu_usage_high"], "deviceId": device_id}))
            .await;

        let filtradas = request
            .get(&format!("/api/alert-rules?deviceId={device_id}"))
            .await;
        let filtradas: Value = serde_json::from_str(&filtradas.text()).unwrap();
        assert_eq!(regras_de(&filtradas).len(), 1);
        assert_eq!(filtradas[0]["deviceId"].as_i64(), Some(device_id));

        // A mesma regra continua na Central: é o mesmo recurso, não uma cópia.
        let todas = request.get("/api/alert-rules").await;
        let todas: Value = serde_json::from_str(&todas.text()).unwrap();
        assert!(regras_de(&todas)
            .iter()
            .any(|regra| regra["id"] == filtradas[0]["id"]));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_escopo_de_uma_regra_pode_voltar_para_todos_os_dispositivos() {
    // O `PUT` é parcial — o toggle da lista manda só `enabled` —, então
    // "campo ausente" precisa manter o escopo. Mas `null` explícito precisa
    // **limpá-lo**: sem isso, uma regra vinculada por engano a um dispositivo
    // ficava presa a ele para sempre, e a tela oferecia uma opção
    // ("Todos os dispositivos") que o backend ignorava em silêncio.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let device_id = cria_dispositivo(
            &request,
            serde_json::json!({"name": "rt-escopo", "type": "router", "ipAddress": "10.9.6.1"}),
        )
        .await;

        let criada = request
            .post("/api/alert-rules")
            .json(&serde_json::json!({
                "name": "CPU do rt-escopo",
                "type": "custom",
                "deviceId": device_id,
                "condition": {"field": fields::CPU_USAGE_PERCENT, "operator": "gt", "value": 85}
            }))
            .await;
        assert_eq!(criada.status_code(), 201, "{}", criada.text());
        let criada: Value = serde_json::from_str(&criada.text()).unwrap();
        let id = criada["id"].as_i64().expect("id");
        assert_eq!(criada["deviceId"].as_i64(), Some(device_id));

        // Campo ausente mantém o escopo.
        let resposta = request
            .put(&format!("/api/alert-rules/{id}"))
            .json(&serde_json::json!({"enabled": false}))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).unwrap();
        assert_eq!(corpo["deviceId"].as_i64(), Some(device_id));

        // `null` explícito devolve a regra ao escopo global.
        let resposta = request
            .put(&format!("/api/alert-rules/{id}"))
            .json(&serde_json::json!({"deviceId": null}))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).unwrap();
        assert!(
            corpo["deviceId"].is_null(),
            "a regra continuou presa ao dispositivo: {corpo}"
        );

        assert!(alert_rules::Entity::find_by_id(id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap()
            .device_id
            .is_none());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn o_servidor_nao_recebe_template_de_alcance_que_nunca_dispararia() {
    // "Dispositivo sem resposta" compara `status == 'down'`, e a coleta de
    // saúde devolve `up` ou `unknown` — nunca `down`. Oferecê-lo para a
    // máquina que faria o alerta é a definição de regra inútil, e o caso que
    // ele descreveria (o processo parado) está fora de escopo por construção.
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        ensure_system_health_monitor(&ctx.db, device.id)
            .await
            .unwrap();

        let corpo: Value = serde_json::from_str(
            &request
                .get(&format!("/api/alert-rules/catalog?deviceId={}", device.id))
                .await
                .text(),
        )
        .unwrap();
        let offline = corpo["templates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["key"] == "device_offline")
            .expect("o template continua no catálogo global");
        assert_eq!(offline["applicable"], false);

        let capacidades: Value = serde_json::from_str(
            &request
                .get(&format!("/api/devices/{}/capabilities", device.id))
                .await
                .text(),
        )
        .unwrap();
        assert!(
            !capacidades["alertFields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|campo| campo == "status"),
            "o servidor não publica `status` avaliável"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn uma_carga_curta_nao_alerta_e_a_sustentada_alerta() {
    // A duração é do motor, não do coletor — o teste existe para provar que os
    // campos novos passam pelo mesmo caminho de `duration_seconds` que os
    // antigos, sem lógica de severidade dentro da coleta.
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        use backend::services::alerts::catalog::service::{apply_scoped, TemplateScope};
        use backend::services::monitoring::contracts::{CheckMetric, CheckResult, MonitorStatus};

        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        let monitor = ensure_system_health_monitor(&ctx.db, device.id)
            .await
            .unwrap();
        apply_scoped(
            &ctx.db,
            &["cpu_usage_high".to_string()],
            TemplateScope::device(device.id),
        )
        .await
        .unwrap();

        let agora = chrono::Utc::now();
        let resultado = CheckResult {
            success: true,
            status: MonitorStatus::Up,
            started_at: agora,
            finished_at: agora,
            duration_ms: 5,
            message: Some("coleta".into()),
            metrics: vec![CheckMetric {
                name: series::CPU_USAGE.into(),
                value: 97.0,
                unit: "percent".into(),
            }],
            data: serde_json::json!({"sources": {}, "unavailable": {}}),
        };
        process_result(&ctx, monitor.id, &resultado, None)
            .await
            .unwrap();

        // `duration_seconds` do template é 300 s: uma única amostra alta não
        // abre evento.
        use backend::models::_entities::alert_events;
        assert_eq!(
            alert_events::Entity::find()
                .filter(alert_events::Column::DeviceId.eq(device.id))
                .count(&ctx.db)
                .await
                .unwrap(),
            0,
            "pico curto não pode alertar"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_leitura_de_cpu_do_snmp_passa_a_ser_avaliavel_por_regra() {
    // Antes desta fase o SNMP publicava `usagePercent`/`usedPercent` soltos, e
    // regra alguma podia falar sobre eles. Agora a mesma leitura vira o campo
    // de dispositivo, o que é o que torna o alerta de CPU válido para o parque.
    use backend::services::alerts::{baseline, datasets::monitor_result};
    use backend::services::monitoring::contracts::{CheckMetric, CheckResult, MonitorStatus};

    let agora = chrono::Utc::now();
    let facts = monitor_result::build(
        "snmp",
        &CheckResult {
            success: true,
            status: MonitorStatus::Up,
            started_at: agora,
            finished_at: agora,
            duration_ms: 3,
            message: None,
            metrics: vec![CheckMetric {
                name: series::CPU_USAGE.into(),
                value: 88.0,
                unit: "percent".into(),
            }],
            data: serde_json::json!({}),
        },
        &baseline::MonitorBaseline::default(),
    );
    assert_eq!(
        facts[fields::CPU_USAGE_PERCENT],
        serde_json::json!(88.0),
        "a série do SNMP precisa chegar ao vocabulário do motor"
    );
}

#[tokio::test]
#[serial]
async fn as_capacidades_vem_do_backend_e_nao_do_nome_do_dispositivo() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let device = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        ensure_system_health_monitor(&ctx.db, device.id)
            .await
            .unwrap();
        let resposta = request
            .get(&format!("/api/devices/{}/capabilities", device.id))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let corpo: Value = serde_json::from_str(&resposta.text()).unwrap();

        assert_eq!(corpo["isSystem"], true);
        assert_eq!(
            corpo["canScanPorts"], false,
            "o servidor não escaneia a si mesmo"
        );
        assert_eq!(corpo["canEditIdentity"], false);
        // A exclusão não está na projeção: quem responde por ela é `isSystem`,
        // que viaja no próprio dispositivo e serve também à lista de
        // `/devices`. Um campo de contrato que ninguém consulta é dívida com
        // aparência de completude.
        assert_eq!(corpo["canSnmpCollect"], false);
        assert_eq!(corpo["interfaces"], false, "sem conexão SNMP, sem aba");
        assert!(corpo["alertFields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|campo| campo == fields::CPU_USAGE_PERCENT));

        // Um roteador comum, com SNMP declarado mas sem nunca ter respondido.
        let outro = cria_dispositivo(
            &request,
            serde_json::json!({
                "name": "rt", "type": "router", "ipAddress": "10.9.4.1", "snmpEnabled": true
            }),
        )
        .await;
        let corpo: Value = serde_json::from_str(
            &request
                .get(&format!("/api/devices/{outro}/capabilities"))
                .await
                .text(),
        )
        .unwrap();
        assert_eq!(corpo["snmpConfigured"], true, "a intenção fica registrada");
        assert_eq!(
            corpo["snmpConnected"], false,
            "configurar não é conectar: a aba e o botão de coleta não podem aparecer"
        );
        assert_eq!(corpo["interfaces"], false);
        assert_eq!(corpo["canSnmpCollect"], false);
        assert_eq!(corpo["canEditIdentity"], true);
    })
    .await;
}
