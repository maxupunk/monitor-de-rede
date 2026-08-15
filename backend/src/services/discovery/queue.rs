//! Fila persistente de discovery em `discovery_runs`.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::{
    models::{discovery_runs, networks},
    services::{
        discovery::service::{run_discovery, ScanSessionService},
        shared::errors::{AppError, AppResult},
    },
};

pub const MIN_SCAN_INTERVAL_SECONDS: i64 = 300;

/// Tempo após o qual uma run `running` é considerada abandonada.
///
/// A varredura ao vivo roda dentro do processo que a iniciou: se ele cai no
/// meio, a linha fica `running` para sempre e passa a bloquear todo novo pedido
/// daquela rede — o botão "Escanear" responderia "já existe uma varredura em
/// andamento" indefinidamente. Uma faixa de /22 (o teto de `MAX_SCAN_HOSTS`) não
/// passa de alguns minutos.
pub const RUNNING_RUN_TIMEOUT_MINUTES: i64 = 15;
const MAX_RUNNING_RUN_TIMEOUT_MINUTES: i64 = 6 * 60;

/// `true` quando a run está `running` há tempo demais para ainda estar viva.
///
/// Runs `pending` **nunca** são abandonadas: elas apenas aguardam o scheduler.
#[must_use]
pub fn is_abandoned(run: &discovery_runs::Model) -> bool {
    if run.status != "running" {
        return false;
    }
    let started_at = run.started_at.with_timezone(&Utc);
    (Utc::now() - started_at).num_minutes() > running_timeout_minutes(run)
}

fn running_timeout_minutes(run: &discovery_runs::Model) -> i64 {
    let usable_hosts = run
        .configuration
        .as_ref()
        .and_then(|value| value.get("usableHosts"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1);
    let batches = usable_hosts.div_ceil(u64::from(
        crate::services::discovery::cidr_range::MAX_SCAN_HOSTS,
    ));
    (RUNNING_RUN_TIMEOUT_MINUTES + batches.saturating_sub(1) as i64 * 5)
        .min(MAX_RUNNING_RUN_TIMEOUT_MINUTES)
}

/// Configuração gravada na run — a UI lê `truncated` daqui para avisar que a
/// faixa não será varrida inteira (matriz de paridade #10).
fn run_configuration(network: &networks::Model) -> serde_json::Value {
    serde_json::json!({
        "cidr": network.cidr,
        "usableHosts": network.usable_hosts(),
        "truncated": network.scan_truncated(),
    })
}

/// Enfileira uma varredura da rede informada.
///
/// Devolve a run existente quando já há uma pendente **ou em curso**: dois
/// cliques no botão "Escanear" não podem virar duas varreduras concorrentes da
/// mesma faixa, que competiriam pelos mesmos pings e duplicariam resultados.
///
/// # Errors
///
/// [`AppError::Validation`] quando o CIDR cadastrado não é varredurável — é a
/// mesma mensagem que o 422 de `POST /api/networks/:id/scan` entrega à tela.
pub async fn enqueue_network_scan(
    db: &sea_orm::DatabaseConnection,
    network: &networks::Model,
) -> AppResult<(discovery_runs::Model, bool)> {
    if !network.scannable() {
        return Err(AppError::validation(format!(
            "A rede \"{}\" não tem uma faixa CIDR varredurável (valor atual: \"{}\").",
            network.name, network.cidr
        )));
    }

    // `pending` **e** `running`: a mais recente é quem decide.
    let current = discovery_runs::Entity::find()
        .filter(crate::models::_entities::discovery_runs::Column::NetworkId.eq(network.id))
        .filter(
            crate::models::_entities::discovery_runs::Column::Status.is_in(["pending", "running"]),
        )
        .order_by_desc(crate::models::_entities::discovery_runs::Column::Id)
        .one(db)
        .await?;

    if let Some(current) = current {
        if is_abandoned(&current) {
            // Fecha a órfã antes de enfileirar a nova, senão ela bloqueia a rede
            // para sempre.
            let timeout_minutes = running_timeout_minutes(&current);
            discovery_runs::ActiveModel {
                id: Set(current.id),
                status: Set("failed".into()),
                finished_at: Set(Some(Utc::now().into())),
                error: Set(Some(format!(
                    "Varredura abandonada (sem conclusão após {timeout_minutes} min)."
                ))),
                ..Default::default()
            }
            .update(db)
            .await?;
        } else {
            let stored_cidr = current
                .configuration
                .as_ref()
                .and_then(|config| config.get("cidr"))
                .and_then(serde_json::Value::as_str);
            // A faixa pode ter sido corrigida depois que a run foi enfileirada.
            // Sem esta atualização o scheduler varreria o CIDR antigo e a edição
            // do operador seria silenciosamente ignorada. Só vale para `pending`:
            // uma run já em curso não muda de faixa no meio do caminho.
            if current.status == "pending" && stored_cidr != Some(network.cidr.as_str()) {
                let run = discovery_runs::ActiveModel {
                    id: Set(current.id),
                    configuration: Set(Some(run_configuration(network))),
                    ..Default::default()
                }
                .update(db)
                .await?;
                return Ok((run, true));
            }
            return Ok((current, true));
        }
    }

    let run = discovery_runs::ActiveModel {
        network_id: Set(network.id),
        probe_id: Set(network.probe_id),
        status: Set("pending".into()),
        started_at: Set(Utc::now().into()),
        configuration: Set(Some(run_configuration(network))),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok((run, false))
}

pub async fn schedule_due_networks(db: &sea_orm::DatabaseConnection) -> AppResult<u64> {
    let now = Utc::now();
    let networks = networks::Entity::find().all(db).await?;
    let mut count = 0;
    for network in networks.into_iter().filter(|network| {
        network.active
            && network.scan_enabled
            && network
                .next_scan_at
                .is_none_or(|next| next.with_timezone(&Utc) <= now)
    }) {
        enqueue_network_scan(db, &network).await?;
        let interval = i64::from(network.scan_interval).max(MIN_SCAN_INTERVAL_SECONDS);
        networks::ActiveModel {
            id: Set(network.id),
            last_scan_at: Set(Some(now.into())),
            next_scan_at: Set(Some((now + chrono::Duration::seconds(interval)).into())),
            ..Default::default()
        }
        .update(db)
        .await?;
        count += 1;
    }
    Ok(count)
}

pub async fn process_pending_runs(ctx: &AppContext) -> AppResult<u64> {
    let Some(run) = discovery_runs::Entity::find()
        .filter(crate::models::_entities::discovery_runs::Column::Status.eq("pending"))
        .filter(crate::models::_entities::discovery_runs::Column::ProbeId.is_null())
        .order_by_asc(crate::models::_entities::discovery_runs::Column::Id)
        .one(&ctx.db)
        .await?
    else {
        return Ok(0);
    };
    let cidr = run
        .configuration
        .as_ref()
        .and_then(|config| config.get("cidr"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let run = discovery_runs::ActiveModel {
        id: Set(run.id),
        status: Set("running".into()),
        started_at: Set(Utc::now().into()),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;
    let session = ScanSessionService::from_context(ctx)?;
    let cancel = session.start(run.id, run.network_id).await;
    match run_discovery(ctx, &cidr, run.id, cancel).await {
        Ok(_) => {
            discovery_runs::ActiveModel {
                id: Set(run.id),
                status: Set("completed".into()),
                finished_at: Set(Some(Utc::now().into())),
                ..Default::default()
            }
            .update(&ctx.db)
            .await?;
            session.finish(None).await;
            Ok(1)
        }
        Err(error) => {
            discovery_runs::ActiveModel {
                id: Set(run.id),
                status: Set("failed".into()),
                finished_at: Set(Some(Utc::now().into())),
                error: Set(Some(error.to_string())),
                ..Default::default()
            }
            .update(&ctx.db)
            .await?;
            session.finish(Some(error.to_string())).await;
            Ok(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(status: &str, started_ago_minutes: i64) -> discovery_runs::Model {
        discovery_runs::Model {
            id: 1,
            network_id: 1,
            probe_id: None,
            status: status.into(),
            started_at: (Utc::now() - chrono::Duration::minutes(started_ago_minutes)).into(),
            finished_at: None,
            configuration: None,
            error: None,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    #[test]
    fn run_pendente_nunca_e_abandonada() {
        // Matriz de paridade #13: `pending` só aguarda o scheduler — declarar
        // abandono aqui faria a fila descartar trabalho que nunca começou.
        assert!(!is_abandoned(&run("pending", 0)));
        assert!(!is_abandoned(&run("pending", 60 * 24)));
    }

    #[test]
    fn run_em_curso_vira_abandonada_depois_do_limite() {
        assert!(!is_abandoned(&run("running", 0)));
        assert!(!is_abandoned(&run("running", RUNNING_RUN_TIMEOUT_MINUTES)));
        assert!(is_abandoned(&run(
            "running",
            RUNNING_RUN_TIMEOUT_MINUTES + 1
        )));
    }

    #[test]
    fn run_concluida_ou_falha_nunca_e_abandonada() {
        // Sem esta guarda o watchdog reescreveria o desfecho de runs já fechadas.
        assert!(!is_abandoned(&run("completed", 60)));
        assert!(!is_abandoned(&run("failed", 60)));
    }

    #[test]
    fn a_configuracao_da_run_confirma_ausencia_de_truncamento() {
        // Matriz de paridade #10: a UI lê `truncated` da configuração da run.
        let mut network = networks::Model {
            id: 1,
            site_id: None,
            probe_id: None,
            name: "Matriz".into(),
            cidr: "10.0.0.0/24".into(),
            gateway: None,
            vlan: None,
            dns_servers: None,
            scan_enabled: true,
            scan_interval: 3_600,
            last_scan_at: None,
            next_scan_at: None,
            active: true,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        };
        let config = run_configuration(&network);
        assert_eq!(config["cidr"], "10.0.0.0/24");
        assert_eq!(config["usableHosts"], 254);
        assert_eq!(config["truncated"], false);

        // /21 tem 2046 hosts utilizáveis e será dividido em dois lotes.
        network.cidr = "10.0.0.0/21".into();
        let config = run_configuration(&network);
        assert_eq!(config["usableHosts"], 2_046);
        assert_eq!(config["truncated"], false);
    }
}
