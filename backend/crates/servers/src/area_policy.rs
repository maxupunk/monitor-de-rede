//! Área Servidores (Docker e backup de bancos).

use async_trait::async_trait;
use netmonitor_core::areas::{Area, AreaPolicy, Consequence, Evidence};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::Value;

use netmonitor_core::shared::text::count_noun;

use netmonitor_core::areas::monitors_keep_running;
use netmonitor_core::{
    agents::identity::{is_agent, is_local},
    models::_entities::{database_connections, monitors, probes},
    shared::errors::AppResult,
};

/// Tipo do monitor de container.
const CONTAINER_MONITOR: &str = "container";

pub struct ServersPolicy;

/// O agente anunciou Docker disponível na última conexão (guardado em
/// `probes.configuration.host.docker`). É estado persistido, não "online agora".
fn announces_docker(probe: &probes::Model) -> bool {
    probe
        .configuration
        .as_ref()
        .and_then(|config| config.pointer("/host/docker/available"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[async_trait]
impl AreaPolicy for ServersPolicy {
    fn area(&self) -> Area {
        Area::Servers
    }

    fn api_prefixes(&self) -> &'static [&'static str] {
        &["/api/docker", "/api/databases"]
    }

    fn event_prefixes(&self) -> &'static [&'static str] {
        &["docker:", "database_jobs:", "database_connections:"]
    }

    async fn evidence(&self, db: &DatabaseConnection) -> AppResult<Evidence> {
        // O agente local nasce com toda instalação (ADR 014): não é evidência.
        let remotos = probes::Entity::find()
            .filter(probes::Column::Status.ne(netmonitor_core::models::probes::STATUS_REVOKED))
            .all(db)
            .await?
            .into_iter()
            .filter(|probe| is_agent(probe) && !is_local(probe) && announces_docker(probe))
            .count();
        if remotos > 0 {
            return Ok(Evidence::because(format!(
                "{} com Docker",
                count_noun(
                    u64::try_from(remotos).unwrap_or(u64::MAX),
                    "servidor remoto",
                    "servidores remotos"
                )
            )));
        }
        let containers = monitors::Entity::find()
            .filter(monitors::Column::Type.eq(CONTAINER_MONITOR))
            .count(db)
            .await?;
        if containers > 0 {
            return Ok(Evidence::because(count_noun(
                containers,
                "monitor de container",
                "monitores de container",
            )));
        }
        let bancos = database_connections::Entity::find().count(db).await?;
        if bancos > 0 {
            return Ok(Evidence::because(format!(
                "{} para backup",
                count_noun(
                    bancos,
                    "banco de dados cadastrado",
                    "bancos de dados cadastrados"
                )
            )));
        }
        Ok(Evidence::none())
    }

    async fn consequences(&self, db: &DatabaseConnection) -> AppResult<Vec<Consequence>> {
        let mut out = vec![
            Consequence::new(
                "Docker (containers, projetos, imagens, volumes, redes e histórico) sai do menu, \
                 do dashboard e da IA.",
            ),
            Consequence::new(
                "Os agentes deixam de enviar o estado ao vivo do Docker; o histórico de consumo \
                 continua gravado.",
            ),
        ];
        let containers = monitors::Entity::find()
            .filter(monitors::Column::Type.eq(CONTAINER_MONITOR))
            .count(db)
            .await?;
        out.extend(monitors_keep_running(
            containers,
            "monitor de container",
            "monitores de container",
        ));
        let backups = database_connections::Entity::find()
            .filter(database_connections::Column::BackupEnabled.eq(true))
            .count(db)
            .await?;
        if backups > 0 {
            out.push(Consequence::new(format!(
                "O backup automático de {} continua: proteção de dado não para pela área.",
                count_noun(backups, "banco", "bancos")
            )));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn agente(configuration: Value) -> probes::Model {
        probes::Model {
            id: 1,
            site_id: None,
            name: "srv".into(),
            token_hash: String::new(),
            status: "online".into(),
            version: None,
            last_seen_at: None,
            registered_at: None,
            revoked_at: None,
            configuration: Some(configuration),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
            device_id: None,
        }
    }

    #[test]
    fn docker_anunciado_e_estado_persistido() {
        assert!(announces_docker(&agente(
            json!({"role": "agent", "host": {"docker": {"available": true}}})
        )));
        assert!(!announces_docker(&agente(
            json!({"role": "agent", "host": {"docker": {"available": false}}})
        )));
        assert!(!announces_docker(&agente(json!({"role": "agent"}))));
    }
}
