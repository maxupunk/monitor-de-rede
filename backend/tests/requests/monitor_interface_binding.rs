//! Vínculo entre monitor e interface por `interface_id` (migração `m20260909_000002`).
//!
//! O monitor de uma porta se chama `Interface {ifName}`, e essa string era o
//! único elo com a linha de `device_interfaces`. Duas homônimas — o que sobra de
//! uma PPPoE que trocou de `ifIndex` — apontavam para o mesmo monitor, e as duas
//! se declaravam monitoradas: o painel mostrava a porta duas vezes, uma zerada.
//!
//! O que se afirma aqui: o backfill acerta o alvo, monitor sem interface fica
//! sem vínculo, e `list_interfaces` passa a distinguir as homônimas.

use backend::{
    app::App,
    models::{_entities::device_interfaces, devices, monitors},
    services::snmp::service::list_interfaces,
};
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use migration::{Migrator, MigratorTrait, SchemaManager};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::json;
use serial_test::serial;

const VINCULO: &str = "m20260909_000002_monitors_interface_id";

/// Roda só a migração do vínculo. O boot de teste já aplicou o conjunto num
/// banco vazio — sem monitor para vincular —, então é aqui, com o cenário
/// montado, que ela é exercitada. Buscá-la pela lista do `Migrator` também
/// afirma que está registrada no `lib.rs`.
async fn vincula(db: &sea_orm::DatabaseConnection) {
    let migracoes = Migrator::migrations();
    let alvo = migracoes
        .iter()
        .find(|item| item.name() == VINCULO)
        .expect("migração de vínculo registrada no Migrator");
    alvo.up(&SchemaManager::new(db))
        .await
        .expect("migração de vínculo");
}

async fn aparelho(db: &sea_orm::DatabaseConnection) -> devices::Model {
    devices::ActiveModel {
        name: Set("Borda".into()),
        r#type: Set("router".into()),
        ip_address: Set(Some("10.0.0.1".into())),
        is_monitored: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

async fn interface(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    snmp_index: i32,
    name: &str,
    visto_ha_dias: i64,
) -> device_interfaces::Model {
    device_interfaces::ActiveModel {
        device_id: Set(device_id),
        snmp_index: Set(Some(snmp_index)),
        name: Set(name.into()),
        oper_status: Set(Some("up".into())),
        last_seen_at: Set(Some((Utc::now() - Duration::days(visto_ha_dias)).into())),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

async fn monitor(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    nome: &str,
    tipo: &str,
) -> monitors::Model {
    monitors::ActiveModel {
        device_id: Set(Some(device_id)),
        r#type: Set(tipo.into()),
        name: Set(nome.into()),
        configuration: Set(json!({"host": "10.0.0.1"})),
        interval_seconds: Set(60),
        timeout_seconds: Set(5),
        retry_count: Set(3),
        enabled: Set(true),
        status: Set("up".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

#[tokio::test]
#[serial]
async fn o_backfill_liga_cada_monitor_a_sua_interface() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx.db).await;
        let sfp2 = interface(&ctx.db, dev.id, 9, "sfp2", 0).await;
        let pppoe = interface(&ctx.db, dev.id, 55, "pppoe-wan", 0).await;

        let mon_sfp2 = monitor(&ctx.db, dev.id, "Interface sfp2", "snmp").await;
        let mon_pppoe = monitor(&ctx.db, dev.id, "Interface pppoe-wan", "snmp").await;
        let mon_cpu = monitor(&ctx.db, dev.id, "Monitor de Uso de CPU", "snmp").await;
        let mon_ping = monitor(&ctx.db, dev.id, "Ping Borda", "ping").await;

        vincula(&ctx.db).await;

        let vinculo = |id: i64| {
            let db = ctx.db.clone();
            async move {
                monitors::Entity::find_by_id(id)
                    .one(&db)
                    .await
                    .unwrap()
                    .expect("monitor preservado")
                    .interface_id
            }
        };

        assert_eq!(vinculo(mon_sfp2.id).await, Some(sfp2.id));
        assert_eq!(vinculo(mon_pppoe.id).await, Some(pppoe.id));
        assert_eq!(
            vinculo(mon_cpu.id).await,
            None,
            "monitor do dispositivo inteiro não pertence a interface nenhuma"
        );
        assert_eq!(vinculo(mon_ping.id).await, None, "idem para o ping");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn homonimas_deixam_de_se_declarar_monitoradas_juntas() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx.db).await;
        // O cenário de produção: a órfã de uma PPPoE que trocou de índice e a
        // viva, com o mesmo nome.
        let orfa = interface(&ctx.db, dev.id, 34, "pppoe-wan", 14).await;
        let viva = interface(&ctx.db, dev.id, 55, "pppoe-wan", 0).await;

        let mon = monitor(&ctx.db, dev.id, "Interface pppoe-wan", "snmp").await;
        // O vínculo aponta a viva — é dela que o monitor coleta.
        let mut ativo: monitors::ActiveModel = mon.into();
        ativo.interface_id = Set(Some(viva.id));
        ativo.update(&ctx.db).await.unwrap();

        let lista = list_interfaces(&ctx, dev.id).await.expect("listagem");
        let monitoradas: Vec<i64> = lista
            .iter()
            .filter(|item| item.is_monitored)
            .map(|item| item.id)
            .collect();

        assert_eq!(
            monitoradas,
            vec![viva.id],
            "só a interface vinculada; pelo nome, as duas apareciam monitoradas"
        );
        assert!(
            lista.iter().any(|item| item.id == orfa.id),
            "a órfã continua listada — quem a remove é a fusão, não esta mudança"
        );
    })
    .await;
}

/// Rodar duas vezes não pode mexer no que já está vinculado: o `auto_migrate`
/// reexecuta o conjunto a cada subida.
#[tokio::test]
#[serial]
async fn o_backfill_e_idempotente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let dev = aparelho(&ctx.db).await;
        let sfp2 = interface(&ctx.db, dev.id, 9, "sfp2", 0).await;
        let mon = monitor(&ctx.db, dev.id, "Interface sfp2", "snmp").await;

        vincula(&ctx.db).await;
        vincula(&ctx.db).await;

        assert_eq!(
            monitors::Entity::find_by_id(mon.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .interface_id,
            Some(sfp2.id)
        );
    })
    .await;
}
