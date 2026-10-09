//! Retenção da trilha de auditoria e das execuções de plugin.
//!
//! As duas tabelas cresciam sem teto: a purga horária cobria só as séries
//! técnicas. Aqui se prova o recorte de cada uma — o que é antigo sai, o que
//! é recente fica, e execução em andamento nunca envelhece.

use backend::{
    app::App,
    models::_entities::{audit_logs, devices, plugin_runs},
    services::maintenance::data_pruner,
};
use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::json;
use serial_test::serial;

async fn auditoria(db: &sea_orm::DatabaseConnection, dias_atras: i64) -> audit_logs::Model {
    audit_logs::ActiveModel {
        action: Set("auth.login".into()),
        created_at: Set((Utc::now() - Duration::days(dias_atras)).into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("registro de auditoria")
}

async fn execucao(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    dias_atras: i64,
    terminada: bool,
) -> plugin_runs::Model {
    let quando = Utc::now() - Duration::days(dias_atras);
    plugin_runs::ActiveModel {
        device_id: Set(device_id),
        action: Set("backup".into()),
        origin: Set("user".into()),
        status: Set(if terminada { "success" } else { "running" }.into()),
        transcript: Set(json!([])),
        finished_at: Set(terminada.then(|| quando.into())),
        created_at: Set(quando.into()),
        updated_at: Set(quando.into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("execução de plugin")
}

#[tokio::test]
#[serial]
async fn a_purga_respeita_a_retencao_da_auditoria_e_das_execucoes_de_plugin() {
    let boot = boot_test::<App>().await.expect("subir app de teste");
    let db = &boot.app_context.db;
    let device = devices::ActiveModel {
        name: Set("roteador".into()),
        r#type: Set("router".into()),
        is_monitored: Set(true),
        snmp_enabled: Set(false),
        status: Set("online".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("dispositivo");

    let auditoria_antiga = auditoria(db, data_pruner::DEFAULT_RETENTION_AUDIT_DAYS + 1).await;
    let auditoria_recente = auditoria(db, 10).await;
    let dias_plugin = data_pruner::DEFAULT_RETENTION_PLUGIN_RUNS_DAYS + 1;
    let execucao_antiga = execucao(db, device.id, dias_plugin, true).await;
    let em_andamento = execucao(db, device.id, dias_plugin, false).await;
    let execucao_recente = execucao(db, device.id, 1, true).await;

    let stats = data_pruner::prune_all(db).await.expect("purga");
    assert_eq!(stats.audit_deleted, 1);
    assert_eq!(stats.plugin_runs_deleted, 1);

    let existe_auditoria = |id| async move {
        audit_logs::Entity::find_by_id(id)
            .one(db)
            .await
            .expect("consulta")
            .is_some()
    };
    assert!(!existe_auditoria(auditoria_antiga.id).await);
    assert!(existe_auditoria(auditoria_recente.id).await);

    let existe_execucao = |id| async move {
        plugin_runs::Entity::find_by_id(id)
            .one(db)
            .await
            .expect("consulta")
            .is_some()
    };
    assert!(!existe_execucao(execucao_antiga.id).await);
    assert!(
        existe_execucao(em_andamento.id).await,
        "execução sem fim registrado nunca é purgada"
    );
    assert!(existe_execucao(execucao_recente.id).await);
}
