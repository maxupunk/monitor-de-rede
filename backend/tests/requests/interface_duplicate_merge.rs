//! Fusão das interfaces homônimas do mesmo aparelho (migração `m20260909_000001`).
//!
//! O caso real: uma interface PPPoE trocou de `ifIndex` enquanto a
//! sincronização ainda casava só por índice, e a linha antiga ficou órfã com o
//! mesmo nome. O painel passou a mostrar a porta duas vezes — uma delas
//! eternamente zerada — e a tela de descoberta a acusar "interface removida"
//! numa gravação que o operador não tinha como satisfazer.
//!
//! O que se afirma aqui é o que a migração precisa garantir para não custar
//! histórico a ninguém: sobra **uma** linha, é a que o equipamento ainda
//! reporta, e tudo que apontava para a órfã passou a apontar para ela.

use backend::{
    app::App,
    models::{
        _entities::{device_interfaces, device_links, metrics},
        devices,
    },
};
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use migration::{Migrator, MigratorTrait, SchemaManager};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serial_test::serial;

const FUSAO: &str = "m20260909_000001_merge_duplicate_device_interfaces";

/// Roda só a migração da fusão. O boot de teste já aplicou o conjunto inteiro
/// num banco vazio — sem duplicata para fundir —, então é aqui, com o cenário
/// montado, que ela é de fato exercitada.
///
/// Buscá-la pela lista do `Migrator` (em vez de instanciar o tipo) também
/// afirma que ela está registrada: esquecer a linha no `lib.rs` é o jeito fácil
/// de escrever uma migração que nunca roda.
async fn funde(db: &sea_orm::DatabaseConnection) {
    let migracoes = Migrator::migrations();
    let fusao = migracoes
        .iter()
        .find(|item| item.name() == FUSAO)
        .expect("migração de fusão registrada no Migrator");
    fusao
        .up(&SchemaManager::new(db))
        .await
        .expect("migração de fusão");
}

#[tokio::test]
#[serial]
async fn a_interface_orfa_e_fundida_na_viva_sem_perder_referencia() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let agora = Utc::now();

        let aparelho = devices::ActiveModel {
            name: Set("Borda".into()),
            r#type: Set("router".into()),
            ip_address: Set(Some("10.0.0.1".into())),
            is_monitored: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // A órfã: índice antigo, vista pela última vez há duas semanas.
        let orfa = device_interfaces::ActiveModel {
            device_id: Set(aparelho.id),
            snmp_index: Set(Some(34)),
            name: Set("pppoe-wan".into()),
            last_seen_at: Set(Some((agora - Duration::days(14)).into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // A viva: índice novo depois da reconexão PPPoE.
        let viva = device_interfaces::ActiveModel {
            device_id: Set(aparelho.id),
            snmp_index: Set(Some(55)),
            name: Set("pppoe-wan".into()),
            last_seen_at: Set(Some(agora.into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // Uma interface de nome diferente no mesmo aparelho não pode ser tocada.
        let intocada = device_interfaces::ActiveModel {
            device_id: Set(aparelho.id),
            snmp_index: Set(Some(9)),
            name: Set("sfp2".into()),
            last_seen_at: Set(Some(agora.into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let metrica = metrics::ActiveModel {
            device_id: Set(aparelho.id),
            interface_id: Set(Some(orfa.id)),
            name: Set("ifHCInOctets".into()),
            value: Set(1_024.0),
            unit: Set("bytes".into()),
            recorded_at: Set(agora.into()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let enlace = device_links::ActiveModel {
            source_device_id: Set(aparelho.id),
            target_device_id: Set(aparelho.id),
            source_interface_id: Set(Some(orfa.id)),
            target_interface_id: Set(Some(orfa.id)),
            link_type: Set("lldp".into()),
            discovery_method: Set("snmp".into()),
            confidence: Set(100),
            confirmed: Set(true),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        // O dispositivo aponta a órfã como entrada de link — foi o que
        // aconteceu em produção.
        let mut ativo: devices::ActiveModel = aparelho.clone().into();
        ativo.link_interface_id = Set(Some(orfa.id));
        ativo.update(&ctx.db).await.unwrap();

        funde(&ctx.db).await;

        let restantes = device_interfaces::Entity::find()
            .filter(device_interfaces::Column::DeviceId.eq(aparelho.id))
            .filter(device_interfaces::Column::Name.eq("pppoe-wan"))
            .all(&ctx.db)
            .await
            .unwrap();
        assert_eq!(restantes.len(), 1, "deveria sobrar uma única pppoe-wan");
        assert_eq!(
            restantes[0].id, viva.id,
            "a sobrevivente é a que o equipamento ainda reporta"
        );
        assert_eq!(restantes[0].snmp_index, Some(55));

        assert!(
            device_interfaces::Entity::find_by_id(intocada.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_some(),
            "interface de outro nome não pode ser afetada"
        );

        let metrica = metrics::Entity::find_by_id(metrica.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .expect("a métrica não pode ser apagada, só repontada");
        assert_eq!(metrica.interface_id, Some(viva.id));

        let enlace = device_links::Entity::find_by_id(enlace.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .expect("enlace preservado");
        assert_eq!(enlace.source_interface_id, Some(viva.id));
        assert_eq!(enlace.target_interface_id, Some(viva.id));

        let aparelho = devices::Entity::find_by_id(aparelho.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .expect("dispositivo preservado");
        assert_eq!(aparelho.link_interface_id, Some(viva.id));
    })
    .await;
}

/// Rodar duas vezes não pode quebrar nem mexer no que já está certo: o
/// `auto_migrate` do boot reexecuta o conjunto a cada subida.
#[tokio::test]
#[serial]
async fn a_fusao_e_idempotente() {
    request_with_config::<App, _, _>(RequestConfig::default(), |_request, ctx| async move {
        let aparelho = devices::ActiveModel {
            name: Set("Borda".into()),
            r#type: Set("router".into()),
            ip_address: Set(Some("10.0.0.1".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let unica = device_interfaces::ActiveModel {
            device_id: Set(aparelho.id),
            snmp_index: Set(Some(55)),
            name: Set("pppoe-wan".into()),
            last_seen_at: Set(Some(Utc::now().into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        funde(&ctx.db).await;
        funde(&ctx.db).await;

        assert!(
            device_interfaces::Entity::find_by_id(unica.id)
                .one(&ctx.db)
                .await
                .unwrap()
                .is_some(),
            "sem duplicata não há o que fundir"
        );
    })
    .await;
}
