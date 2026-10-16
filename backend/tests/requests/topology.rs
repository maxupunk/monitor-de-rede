//! Rotas de topologia: grafo com recorte por site, layout (inteiro e parcial),
//! CRUD de enlaces, hierarquia virtual e menor caminho.

use backend::{
    app::App,
    models::{_entities::event_outbox, device_interfaces, device_links, devices, sites},
    services::{
        devices::system_device::SystemDeviceService,
        topology::link_resolver::{persist_resolved_links_detailed, NetworkLink},
    },
};
use loco_rs::{app::AppContext, testing::prelude::*};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter, Set,
};
use serde_json::{json, Value};
use serial_test::serial;

use super::prepare_data;

async fn autenticado(request: &mut loco_rs::TestServer, ctx: &AppContext) {
    let session = prepare_data::init_user_login(request, ctx).await;
    let (header, value) = prepare_data::auth_header(&session.token);
    request.add_header(header, value);
}

async fn site(ctx: &AppContext, name: &str) -> sites::Model {
    sites::ActiveModel {
        name: Set(name.into()),
        active: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn dispositivo(
    ctx: &AppContext,
    name: &str,
    tipo: &str,
    site_id: Option<i64>,
    parent_id: Option<i64>,
) -> devices::Model {
    devices::ActiveModel {
        name: Set(name.into()),
        r#type: Set(tipo.into()),
        site_id: Set(site_id),
        parent_id: Set(parent_id),
        status: Set("online".into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn interface(ctx: &AppContext, device_id: i64, name: &str, oper: &str) -> i64 {
    device_interfaces::ActiveModel {
        device_id: Set(device_id),
        name: Set(name.into()),
        speed: Set(Some(1_000_000_000)),
        admin_status: Set(Some("up".into())),
        oper_status: Set(Some(oper.into())),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
    .id
}

async fn eventos(ctx: &AppContext, tipo: &str) -> u64 {
    event_outbox::Entity::find()
        .filter(event_outbox::Column::Type.eq(tipo))
        .count(&ctx.db)
        .await
        .unwrap()
}

fn corpo(texto: &str) -> Value {
    serde_json::from_str(texto).expect("json")
}

#[tokio::test]
#[serial]
async fn filtro_de_site_recorta_nos_e_enlaces_e_marca_o_sistema() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let matriz = site(&ctx, "Matriz").await;
        let filial = site(&ctx, "Filial").await;
        let roteador = dispositivo(&ctx, "rt-matriz", "router", Some(matriz.id), None).await;
        let switch = dispositivo(&ctx, "sw-matriz", "switch", Some(matriz.id), None).await;
        let outro = dispositivo(&ctx, "rt-filial", "router", Some(filial.id), None).await;
        let sistema = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();

        let porta_rt = interface(&ctx, roteador.id, "ge-0/0/1", "up").await;
        let porta_sw = interface(&ctx, switch.id, "Gi0/1", "down").await;
        for (a, b, ia, ib) in [
            (roteador.id, switch.id, Some(porta_rt), Some(porta_sw)),
            (roteador.id, outro.id, None, None),
            (sistema.id, roteador.id, None, None),
        ] {
            let resposta = request
                .post("/api/topology/links")
                .json(&json!({
                    "sourceDeviceId": a, "targetDeviceId": b,
                    "sourceInterfaceId": ia, "targetInterfaceId": ib,
                }))
                .await;
            assert_eq!(resposta.status_code(), 201, "{}", resposta.text());
        }

        let resposta = request
            .get(&format!("/api/topology?siteId={}", matriz.id))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        let grafo = corpo(&resposta.text());
        let mut ids: Vec<i64> = grafo["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["id"].as_i64().unwrap())
            .collect();
        ids.sort_unstable();
        // O NetMonitor entra porque está ligado ao roteador; a filial não.
        let mut esperado = vec![roteador.id, switch.id, sistema.id];
        esperado.sort_unstable();
        assert_eq!(ids, esperado);

        let no_sistema = grafo["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == sistema.id)
            .unwrap();
        assert_eq!(no_sistema["isSystem"], true);
        let no_roteador = grafo["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == roteador.id)
            .unwrap();
        assert_eq!(no_roteador["isSystem"], false);
        assert_eq!(no_roteador["siteName"], "Matriz");
        assert_eq!(no_roteador["interfaceCount"], 1);
        assert_eq!(no_roteador["monitorsDown"], 0);

        let arestas = grafo["edges"].as_array().unwrap();
        assert_eq!(arestas.len(), 2, "enlace para a filial vazou: {arestas:?}");
        let fisico = arestas
            .iter()
            .find(|edge| edge["targetDeviceId"] == switch.id)
            .unwrap();
        // Porta do switch caída: o enlace não pode aparecer "up".
        assert_eq!(fisico["status"], "down");
        assert_eq!(fisico["speedBps"], 1_000_000_000_i64);
        assert_eq!(fisico["confirmed"], true);
        assert_eq!(fisico["stale"], false);
        assert!(fisico["lastSeenAt"].is_string());

        // Sem filtro, todos aparecem.
        let tudo = corpo(&request.get("/api/topology").await.text());
        assert_eq!(tudo["nodes"].as_array().unwrap().len(), 4);
        assert_eq!(tudo["edges"].as_array().unwrap().len(), 3);

        assert_eq!(
            request.get("/api/topology?siteId=abc").await.status_code(),
            422
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn layout_put_substitui_patch_funde_e_ambos_publicam_evento() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;

        let vazio = corpo(&request.get("/api/topology/layout?siteId=3").await.text());
        assert_eq!(vazio["nodes"], json!([]));

        let put = request
            .put("/api/topology/layout?siteId=3")
            .json(&json!({ "nodes": [
                { "deviceId": 1, "x": 10.0, "y": 10.0 },
                { "deviceId": 2, "x": 20.0, "y": 20.0 },
            ]}))
            .await;
        assert_eq!(put.status_code(), 200, "{}", put.text());
        assert_eq!(eventos(&ctx, "topology:layout_updated").await, 1);

        let patch = request
            .patch("/api/topology/layout?siteId=3")
            .json(&json!({ "nodes": [
                { "deviceId": 2, "x": 99.0, "y": 98.0 },
                { "deviceId": 3, "x": 30.0, "y": 30.0 },
            ]}))
            .await;
        assert_eq!(patch.status_code(), 200, "{}", patch.text());
        assert_eq!(eventos(&ctx, "topology:layout_updated").await, 2);

        let layout = corpo(&request.get("/api/topology/layout?siteId=3").await.text());
        assert_eq!(
            layout["nodes"],
            json!([
                { "deviceId": 1, "x": 10.0, "y": 10.0 },
                { "deviceId": 2, "x": 99.0, "y": 98.0 },
                { "deviceId": 3, "x": 30.0, "y": 30.0 },
            ])
        );
        // Outro site não foi tocado.
        let global = corpo(&request.get("/api/topology/layout").await.text());
        assert_eq!(global["nodes"], json!([]));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn crud_de_enlace_e_hierarquia_virtual() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let a = dispositivo(&ctx, "a", "switch", None, None).await;
        let b = dispositivo(&ctx, "b", "switch", None, None).await;
        let porta_b = interface(&ctx, b.id, "eth0", "up").await;

        // Mesmo dispositivo e dispositivo inexistente.
        let mesmo = request
            .post("/api/topology/links")
            .json(&json!({ "sourceDeviceId": a.id, "targetDeviceId": a.id }))
            .await;
        assert_eq!(mesmo.status_code(), 422);
        let fantasma = request
            .post("/api/topology/links")
            .json(&json!({ "sourceDeviceId": a.id, "targetDeviceId": 999_999 }))
            .await;
        assert_eq!(fantasma.status_code(), 404);
        // Porta de B na ponta de A.
        let porta_errada = request
            .post("/api/topology/links")
            .json(&json!({ "sourceDeviceId": a.id, "targetDeviceId": b.id, "sourceInterfaceId": porta_b }))
            .await;
        assert_eq!(porta_errada.status_code(), 422);

        let criado = request
            .post("/api/topology/links")
            .json(&json!({ "sourceDeviceId": a.id, "targetDeviceId": b.id }))
            .await;
        assert_eq!(criado.status_code(), 201, "{}", criado.text());
        let id = corpo(&criado.text())["id"].as_i64().unwrap();

        let editado = request
            .put(&format!("/api/topology/links/{id}"))
            .json(&json!({ "targetInterfaceId": porta_b, "linkType": "fiber" }))
            .await;
        assert_eq!(editado.status_code(), 200, "{}", editado.text());
        let linha = device_links::Entity::find_by_id(id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(linha.target_interface_id, Some(porta_b));
        assert_eq!(linha.link_type, "fiber");

        assert_eq!(
            request
                .delete(&format!("/api/topology/links/{id}"))
                .await
                .status_code(),
            200
        );
        assert_eq!(
            request
                .delete(&format!("/api/topology/links/{id}"))
                .await
                .status_code(),
            404
        );

        // Hierarquia: pai com id alto não colide mais (era -(filho*1000+pai)).
        let filho = dispositivo(&ctx, "ap", "access_point", None, Some(b.id)).await;
        let grafo = corpo(&request.get("/api/topology").await.text());
        let virtual_edge = grafo["edges"]
            .as_array()
            .unwrap()
            .iter()
            .find(|edge| edge["linkType"] == "parent")
            .expect("aresta de hierarquia");
        assert_eq!(virtual_edge["id"], -filho.id);
        assert_eq!(
            request
                .put(&format!("/api/topology/links/{}", -filho.id))
                .json(&json!({}))
                .await
                .status_code(),
            422
        );
        assert_eq!(
            request
                .delete(&format!("/api/topology/links/{}", -filho.id))
                .await
                .status_code(),
            200
        );
        let filho = devices::Entity::find_by_id(filho.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(filho.parent_id, None);
        assert!(eventos(&ctx, "topology:updated").await >= 3);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn descoberta_lldp_nao_rebaixa_enlace_manual() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let a = dispositivo(&ctx, "a", "switch", None, None).await;
        let b = dispositivo(&ctx, "b", "switch", None, None).await;
        let porta_a = interface(&ctx, a.id, "ge1", "up").await;
        let criado = request
            .post("/api/topology/links")
            .json(&json!({ "sourceDeviceId": a.id, "targetDeviceId": b.id }))
            .await;
        assert_eq!(criado.status_code(), 201);

        // O LLDP de B viu o enlace no sentido contrário, com a porta de A.
        persist_resolved_links_detailed(
            &ctx.db,
            vec![NetworkLink {
                source_device_id: b.id,
                target_device_id: a.id,
                source_interface_id: None,
                target_interface_id: Some(porta_a),
                link_type: "lldp".into(),
                discovery_method: "lldp".into(),
                confidence: 95,
                confirmed: false,
            }],
        )
        .await
        .unwrap();

        let linhas = device_links::Entity::find().all(&ctx.db).await.unwrap();
        assert_eq!(linhas.len(), 1);
        let linha = &linhas[0];
        assert_eq!(linha.discovery_method, "user_defined");
        assert_eq!(linha.confidence, 100);
        assert!(linha.confirmed);
        // A porta que faltava foi completada na ponta certa.
        assert_eq!(linha.source_interface_id, Some(porta_a));
        assert_eq!(linha.target_interface_id, None);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn recalculo_liga_cada_host_a_um_unico_uplink() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let rede = request
            .post("/api/networks")
            .json(&json!({ "name": "LAN", "cidr": "127.0.0.0/30", "scanInterval": 60 }))
            .await;
        assert_eq!(rede.status_code(), 201, "{}", rede.text());
        let rede = corpo(&rede.text())["id"].as_i64().unwrap();
        let mut ids = Vec::new();
        for (nome, tipo) in [
            ("sw", "switch"),
            ("rt", "router"),
            ("h1", "server"),
            ("h2", "server"),
        ] {
            let mut device = dispositivo(&ctx, nome, tipo, None, None)
                .await
                .into_active_model();
            device.network_id = Set(Some(rede));
            ids.push(device.update(&ctx.db).await.unwrap().id);
        }

        for _ in 0..2 {
            let resposta = request.post("/api/topology/recalculate").await;
            assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
            assert_eq!(corpo(&resposta.text())["inferredCount"], 2);
        }
        let mut pares: Vec<(i64, i64)> = device_links::Entity::find()
            .all(&ctx.db)
            .await
            .unwrap()
            .into_iter()
            .map(|link| (link.source_device_id, link.target_device_id))
            .collect();
        pares.sort_unstable();
        // Roteador (ids[1]) → cada host; nada do switch, nada duplicado.
        assert_eq!(pares, vec![(ids[1], ids[2]), (ids[1], ids[3])]);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn caminho_do_servidor_ate_o_ap_passa_pelo_roteador() {
    request_with_config::<App, _, _>(RequestConfig::default(), |mut request, ctx| async move {
        autenticado(&mut request, &ctx).await;
        let sistema = SystemDeviceService::new(&ctx.db).ensure().await.unwrap();
        let roteador = dispositivo(&ctx, "rt", "router", None, None).await;
        let ap = dispositivo(&ctx, "ap", "access_point", None, Some(roteador.id)).await;
        let isolado = dispositivo(&ctx, "isolado", "server", None, None).await;
        let enlace = corpo(
            &request
                .post("/api/topology/links")
                .json(&json!({ "sourceDeviceId": sistema.id, "targetDeviceId": roteador.id }))
                .await
                .text(),
        )["id"]
            .as_i64()
            .unwrap();

        let resposta = request
            .get(&format!(
                "/api/topology/path?from={}&to={}",
                sistema.id, ap.id
            ))
            .await;
        assert_eq!(resposta.status_code(), 200, "{}", resposta.text());
        assert_eq!(
            corpo(&resposta.text()),
            json!({
                "from": sistema.id,
                "to": ap.id,
                "deviceIds": [sistema.id, roteador.id, ap.id],
                "edgeIds": [enlace, -ap.id],
                "hops": 2,
            })
        );

        let sem_caminho = request
            .get(&format!(
                "/api/topology/path?from={}&to={}",
                sistema.id, isolado.id
            ))
            .await;
        assert_eq!(sem_caminho.status_code(), 404);
        let inexistente = request
            .get(&format!("/api/topology/path?from={}&to=999999", sistema.id))
            .await;
        assert_eq!(inexistente.status_code(), 404);
        assert_eq!(
            request.get("/api/topology/path?from=1").await.status_code(),
            422
        );
    })
    .await;
}
