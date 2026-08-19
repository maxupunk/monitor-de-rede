//! Monitores **gerenciados**: criados e mantidos pelo sistema, não pelo usuário.
//!
//! Hoje há um só — a coleta de saúde do Servidor NetMonitor, do tipo
//! [`SYSTEM_HEALTH`]. O módulo existe como conceito genérico porque a regra
//! ("o usuário ajusta intervalo e executa agora, mas não troca tipo, alvo nem
//! apaga") não tem nada de específico do servidor: o dia em que outro monitor
//! nascer do sistema, ele entra aqui.
//!
//! # As três armadilhas que este módulo evita
//!
//! 1. **`probe_id` preenchido.** O `execute_one` despacha para o probe remoto
//!    quando o campo existe — e a coleta mediria a saúde do probe, não a do
//!    servidor. O provisionamento força `NULL` e a guarda recusa a mudança.
//! 2. **`retry_count` herdado.** O padrão do produto é `3`, e
//!    `run_local_confirming_failure` repetiria a checagem até quatro vezes num
//!    `down`. Reler `/proc` quatro vezes não confirma coisa alguma; nasce `0`.
//! 3. **Boots concorrentes.** O índice único por `(device_id, type)` decide o
//!    vencedor, e o perdedor relê a linha do outro.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};

use crate::{
    models::monitors,
    services::shared::errors::{AppError, AppResult},
};

/// Tipo do monitor de coleta de saúde local.
pub const SYSTEM_HEALTH: &str = "system_health";

/// Intervalo inicial. Ajustável pelo usuário — 15 s é o mesmo passo da coleta
/// SNMP, então os gráficos de CPU do servidor e de um roteador ficam
/// comparáveis lado a lado.
pub const DEFAULT_INTERVAL_SECONDS: i32 = 15;

/// Verdadeiro para os tipos que o sistema gerencia.
#[must_use]
pub fn is_managed(kind: &str) -> bool {
    kind.eq_ignore_ascii_case(SYSTEM_HEALTH)
}

/// Garante o monitor de saúde do dispositivo informado.
///
/// Idempotente e seguro sob concorrência, pela mesma mecânica do dispositivo
/// do sistema. Corrige também linhas herdadas de um provisionamento anterior
/// que estivessem com `probe_id` ou `retry_count` errados — as duas condições
/// que quebrariam a coleta em silêncio.
pub async fn ensure_system_health_monitor(
    db: &DatabaseConnection,
    device_id: i64,
) -> Result<monitors::Model, sea_orm::DbErr> {
    if let Some(row) = find(db, device_id).await? {
        return repair(db, row).await;
    }

    let txn = db.begin().await?;
    let inserted = monitors::ActiveModel {
        device_id: Set(Some(device_id)),
        // Nunca um probe: ver a armadilha 1 na nota do módulo.
        probe_id: Set(None),
        r#type: Set(SYSTEM_HEALTH.to_string()),
        name: Set("Saúde do sistema".to_string()),
        configuration: Set(serde_json::json!({})),
        interval_seconds: Set(DEFAULT_INTERVAL_SECONDS),
        timeout_seconds: Set(10),
        // Nunca reter: ver a armadilha 2.
        retry_count: Set(0),
        enabled: Set(true),
        status: Set("unknown".to_string()),
        ..Default::default()
    }
    .insert(&txn)
    .await;

    match inserted {
        Ok(row) => {
            txn.commit().await?;
            Ok(row)
        }
        Err(error) => {
            txn.rollback().await?;
            find(db, device_id).await?.ok_or_else(|| {
                sea_orm::DbErr::Custom(format!(
                    "não foi possível garantir o monitor de saúde do sistema: {error}"
                ))
            })
        }
    }
}

async fn find(
    db: &DatabaseConnection,
    device_id: i64,
) -> Result<Option<monitors::Model>, sea_orm::DbErr> {
    monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device_id))
        .filter(monitors::Column::Type.eq(SYSTEM_HEALTH))
        .one(db)
        .await
}

/// Reconduz uma linha existente às invariantes do monitor gerenciado.
async fn repair(
    db: &DatabaseConnection,
    row: monitors::Model,
) -> Result<monitors::Model, sea_orm::DbErr> {
    if row.probe_id.is_none() && row.retry_count == 0 {
        return Ok(row);
    }
    tracing::warn!(
        monitor_id = row.id,
        probe_id = ?row.probe_id,
        retry_count = row.retry_count,
        "monitor gerenciado fora das invariantes; corrigindo"
    );
    let mut active: monitors::ActiveModel = row.into();
    active.probe_id = Set(None);
    active.retry_count = Set(0);
    active.update(db).await
}

/// O que uma edição pretende mudar num monitor.
///
/// `None` é "não informado" — a mesma semântica do `PUT` parcial que o
/// cadastro de monitores já usa.
#[derive(Default)]
pub struct ProposedMonitor<'a> {
    pub monitor_type: Option<&'a str>,
    pub device_id: Option<i64>,
    pub probe_id: Option<i64>,
    pub configuration: Option<&'a serde_json::Value>,
    pub enabled: Option<bool>,
}

/// Barra as mudanças que descaracterizariam um monitor gerenciado.
///
/// O que continua permitido é o que o roadmap pede: ajustar intervalo e
/// executar agora. Tipo, alvo, probe, desativação e exclusão, não.
pub fn ensure_editable(current: &monitors::Model, proposed: &ProposedMonitor<'_>) -> AppResult<()> {
    if !is_managed(&current.r#type) {
        return Ok(());
    }
    fn recusa(campo: &str) -> AppResult<()> {
        Err(AppError::BusinessRule(format!(
            "{campo} da coleta de saúde do sistema não pode ser alterado: o monitor é mantido pelo próprio NetMonitor"
        )))
    }
    if proposed
        .monitor_type
        .is_some_and(|kind| !kind.eq_ignore_ascii_case(&current.r#type))
    {
        return recusa("O tipo");
    }
    if proposed
        .device_id
        .is_some_and(|id| Some(id) != current.device_id)
    {
        return recusa("O dispositivo");
    }
    if proposed.probe_id.is_some() {
        return recusa("O probe");
    }
    if proposed
        .configuration
        .is_some_and(|config| config.as_object().is_none_or(|objeto| !objeto.is_empty()))
    {
        return recusa("O alvo");
    }
    if proposed.enabled == Some(false) {
        return Err(AppError::BusinessRule(
            "A coleta de saúde do sistema não pode ser desativada: sem ela o servidor deixa de ter métricas".to_string(),
        ));
    }
    Ok(())
}

/// Barra a exclusão de um monitor gerenciado.
pub fn ensure_deletable(current: &monitors::Model) -> AppResult<()> {
    if is_managed(&current.r#type) {
        return Err(AppError::BusinessRule(
            "A coleta de saúde do sistema é mantida pelo próprio NetMonitor e não pode ser excluída"
                .to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(kind: &str) -> monitors::Model {
        let agora = chrono::Utc::now();
        monitors::Model {
            id: 1,
            device_id: Some(7),
            probe_id: None,
            r#type: kind.to_string(),
            name: "Saúde do sistema".into(),
            configuration: serde_json::json!({}),
            interval_seconds: DEFAULT_INTERVAL_SECONDS,
            timeout_seconds: 10,
            retry_count: 0,
            enabled: true,
            next_run_at: None,
            last_run_at: None,
            status: "unknown".into(),
            created_at: agora.into(),
            updated_at: agora.into(),
        }
    }

    #[test]
    fn o_intervalo_continua_ajustavel_porque_nao_descaracteriza_nada() {
        // Um `PUT` que só mexe no intervalo não informa nenhum dos campos
        // protegidos, e por isso passa.
        assert!(ensure_editable(&monitor(SYSTEM_HEALTH), &ProposedMonitor::default()).is_ok());
    }

    #[test]
    fn trocar_tipo_alvo_probe_ou_desativar_e_recusado() {
        let gerenciado = monitor(SYSTEM_HEALTH);
        let casos: Vec<ProposedMonitor<'_>> = vec![
            ProposedMonitor {
                monitor_type: Some("ping"),
                ..Default::default()
            },
            ProposedMonitor {
                device_id: Some(99),
                ..Default::default()
            },
            ProposedMonitor {
                probe_id: Some(1),
                ..Default::default()
            },
            ProposedMonitor {
                enabled: Some(false),
                ..Default::default()
            },
        ];
        for caso in casos {
            assert!(
                ensure_editable(&gerenciado, &caso).is_err(),
                "mudança deveria ser recusada"
            );
        }
        assert!(ensure_deletable(&gerenciado).is_err());
    }

    #[test]
    fn configuracao_vazia_nao_e_troca_de_alvo() {
        let vazia = serde_json::json!({});
        assert!(ensure_editable(
            &monitor(SYSTEM_HEALTH),
            &ProposedMonitor {
                configuration: Some(&vazia),
                ..Default::default()
            }
        )
        .is_ok());

        let com_alvo = serde_json::json!({"host": "8.8.8.8"});
        assert!(ensure_editable(
            &monitor(SYSTEM_HEALTH),
            &ProposedMonitor {
                configuration: Some(&com_alvo),
                ..Default::default()
            }
        )
        .is_err());
    }

    #[test]
    fn monitor_comum_nao_e_afetado_por_nenhuma_das_guardas() {
        let comum = monitor("ping");
        assert!(ensure_deletable(&comum).is_ok());
        assert!(ensure_editable(
            &comum,
            &ProposedMonitor {
                monitor_type: Some("tcp"),
                probe_id: Some(3),
                enabled: Some(false),
                ..Default::default()
            }
        )
        .is_ok());
    }
}
