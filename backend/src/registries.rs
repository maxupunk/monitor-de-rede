//! Os registros que o ponto de montagem preenche (ADR 017): cada área
//! entrega as suas peças e o núcleo só consulta o registro. É o único lugar,
//! fora de `app.rs`, que conhece todas as áreas.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use loco_rs::app::AppContext;
use netmonitor_assistant::{
    central as assistant,
    mentions::{ContainerDirectory, MentionContainer},
};
use netmonitor_core::{
    agents::{
        command::{AgentCommandDispatchers, AgentCommandHandler, AgentCommandHandlers},
        host::HostDocker,
        targets::DeviceNetworks,
    },
    ai::tools::AiToolHandler,
    alerts::contracts::AlertEvaluationContext,
    areas::AreaPolicy,
    classifier::Classifier,
    devices::access::DeviceAccess,
    models::devices,
    monitoring::{CheckDeps, MonitorCheckers, MonitorGroups},
    notifications::NotificationMessage,
    routines::AreaRoutines,
    shared::errors::AppError,
};
use netmonitor_network::central as network;
use netmonitor_vpn::central::{Central, CentralServices};
use sea_orm::DatabaseConnection;
use serde_json::Value;

use netmonitor_nucleo::{
    controllers::devices::present_for_vpn,
    services::agents::install_address,
    services::ai_tools::{
        actions, alert_rules, analysis, ask, audit, charts, dashboard, diagnostics, grep, history,
        inventory, logs, platform, schedules,
    },
    services::alerts::{
        catalog as alert_catalog, inhibition, manager as alert_manager, recovery as alert_recovery,
        vpn_peer_watcher,
    },
    services::devices::identify,
    services::notifications::NotificationService,
    services::server_addresses,
    services::shared::errors::AppResult,
    services::webpush::{deliver, Audience},
};

use netmonitor_servers::docker::{
    engine, hosts,
    source::{DockerEngine, LocalEngine},
};

use netmonitor_network::plugins::{ai_tools as plugins, auto_accept, credentials};

/// As políticas das áreas opcionais.
#[must_use]
pub fn area_policies() -> Vec<Box<dyn AreaPolicy>> {
    vec![
        Box::new(netmonitor_network::NetworkPolicy),
        Box::new(netmonitor_servers::ServersPolicy),
        Box::new(netmonitor_vpn::VpnPolicy),
    ]
}

/// Os tipos de monitor das áreas (o `snmp` da Rede, o `container` de
/// Servidores). A Docker Engine só existe no agente
/// (ADR 014): a central passa `None`.
#[must_use]
pub fn monitor_checkers(docker: Option<Arc<dyn DockerEngine>>) -> MonitorCheckers {
    let mut checkers = netmonitor_network::monitor_checkers();
    checkers.extend(netmonitor_servers::monitor_checkers(docker));
    MonitorCheckers::new(checkers)
}

/// O que a área VPN pede ao resto da central: o motor de alertas, a
/// apresentação de dispositivo e a lista de endereços da central.
struct VpnCentral;

#[async_trait]
impl CentralServices for VpnCentral {
    async fn evaluate_peer_states(&self, ctx: &AppContext, vpn_server_id: i64) -> AppResult<usize> {
        vpn_peer_watcher::evaluate_server_peers(ctx, vpn_server_id).await
    }

    async fn present_device(
        &self,
        db: &DatabaseConnection,
        device: devices::Model,
    ) -> AppResult<Option<Value>> {
        Ok(serde_json::to_value(present_for_vpn(db, device).await?).ok())
    }

    async fn forget_address_override(
        &self,
        db: &DatabaseConnection,
        kind: &str,
        detected: &str,
    ) -> AppResult<bool> {
        server_addresses::forget_override_if(db, kind, detected).await
    }
}

/// Os serviços da central para a VPN, para o `shared_store`.
#[must_use]
pub fn vpn_central() -> Central {
    Central::new(Arc::new(VpnCentral))
}

/// O que a área Rede pede ao resto da central: o motor de alertas, os
/// endereços da central, a identificação de dispositivo, a porta publicada
/// da API e a IA.
struct NetworkCentral;

#[async_trait]
impl network::CentralServices for NetworkCentral {
    async fn evaluate_alert(
        &self,
        ctx: &AppContext,
        context: &AlertEvaluationContext,
    ) -> AppResult<()> {
        alert_manager::evaluate(ctx, context).await
    }

    async fn observe_device(
        &self,
        ctx: &AppContext,
        device: &devices::Model,
        ssh_port: Option<u16>,
    ) -> AppResult<devices::Model> {
        identify::observe(ctx, device, ssh_port).await
    }

    async fn close_alert_scope(
        &self,
        ctx: &AppContext,
        scope_key: &str,
        reason: &str,
    ) -> AppResult<usize> {
        alert_recovery::close_scope(ctx, scope_key, reason).await
    }

    async fn ensure_wifi_watch_rules(&self, db: &DatabaseConnection) -> AppResult<()> {
        alert_catalog::once::ensure_wifi_watch(db).await.map(|_| ())
    }

    async fn address_toward(
        &self,
        ctx: &AppContext,
        device: &devices::Model,
    ) -> AppResult<Option<String>> {
        let nat = server_addresses::nat_detector(ctx);
        let list = server_addresses::list(&ctx.db, &nat).await?;
        let Some((id, _)) = server_addresses::suggest_for(&ctx.db, device, &list).await? else {
            return Ok(None);
        };
        Ok(list
            .into_iter()
            .find(|address| address.id == id)
            .and_then(|address| address.value))
    }

    fn published_api_port(&self, ctx: &AppContext) -> u16 {
        install_address::ports(ctx).1
    }

    async fn ask_ai(
        &self,
        db: &DatabaseConnection,
        system: &str,
        user: &str,
        max_tokens: u32,
        timeout: Duration,
    ) -> Result<Option<(String, Option<String>)>, String> {
        use futures::StreamExt;
        use netmonitor_assistant::{
            drivers::{
                self,
                traits::{AiChatOptions, AiMessage},
            },
            settings,
        };

        let settings = settings::load(db)
            .await
            .map_err(|error| error.to_string())?;
        if !settings.enabled {
            return Ok(None);
        }
        let driver = drivers::connect(&settings)
            .await
            .map_err(|error| error.to_string())?;
        let messages = vec![
            AiMessage {
                role: "system".into(),
                content: Some(system.into()),
                tool_calls: None,
                tool_call_id: None,
            },
            AiMessage {
                role: "user".into(),
                content: Some(user.into()),
                tool_calls: None,
                tool_call_id: None,
            },
        ];
        let work = async {
            let mut stream = driver
                .chat_stream(
                    &messages,
                    &[],
                    AiChatOptions {
                        max_tokens: Some(max_tokens),
                    },
                )
                .await
                .map_err(|error| error.to_string())?;
            let (mut answer, mut model) = (String::new(), driver.model().map(str::to_owned));
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|error| error.to_string())?;
                if let Some(delta) = chunk.text_delta {
                    answer.push_str(&delta);
                }
                if chunk.model.is_some() {
                    model = chunk.model;
                }
            }
            Ok(Some((answer, model)))
        };
        tokio::time::timeout(timeout, work)
            .await
            .map_err(|_| "a IA não respondeu a tempo".to_owned())?
    }
}

/// Os comandos das áreas no host do agente (ADR 017, `AgentCommandHandler`):
/// o Docker (compose incluído), de Servidores; descoberta, acesso a
/// equipamento e vazão, da Rede. A política local (`AGENT_ALLOW`) continua
/// no executor, antes deles.
#[must_use]
pub fn agent_command_handlers(deps: &CheckDeps, networks: DeviceNetworks) -> AgentCommandHandlers {
    let mut handlers: Vec<Arc<dyn AgentCommandHandler>> = vec![Arc::new(
        netmonitor_servers::agent_host::DockerCommands::default(),
    )];
    handlers.extend(netmonitor_network::agent_commands::handlers(
        deps.clone(),
        networks,
        None,
    ));
    AgentCommandHandlers::new(handlers)
}

/// Os tipos de monitor no agente: os mesmos da central, com a Docker Engine
/// deste host (ADR 014) no monitor de container.
#[must_use]
pub fn agent_monitor_checkers() -> MonitorCheckers {
    monitor_checkers(Some(Arc::new(LocalEngine)))
}

/// O Docker do host do agente (Servidores): o `Hello`, o amostrador, o
/// estado ao vivo e a exportação de volume.
#[must_use]
pub fn agent_host_docker() -> Arc<dyn HostDocker> {
    Arc::new(netmonitor_servers::agent_host::LocalHostDocker)
}

/// As rotinas que as áreas contribuem ao ciclo do scheduler e à purga
/// (ADR 017, registro de rotinas): a coleta SNMP sem monitor, a varredura, o
/// vigia do Wi-Fi e os testes de enlace agendados, da Rede; a telemetria dos
/// túneis, da VPN; a retenção do histórico de enlace.
#[must_use]
pub fn area_routines() -> AreaRoutines {
    let mut routines = netmonitor_network::routines::routines();
    routines.extend(netmonitor_vpn::routines::routines());
    AreaRoutines::new(routines, netmonitor_network::routines::retention())
}

/// Os monitores que uma área executa em grupo (os SNMP locais de um mesmo
/// dispositivo, da Rede).
#[must_use]
pub fn monitor_groups() -> MonitorGroups {
    MonitorGroups::new(netmonitor_network::routines::monitor_groups())
}

/// O lado central dos comandos das áreas (ADR 017): o despacho das
/// varreduras pendentes aos agentes conectados.
#[must_use]
pub fn agent_command_dispatchers() -> AgentCommandDispatchers {
    AgentCommandDispatchers::new(vec![Arc::new(
        netmonitor_network::discovery::remote::DiscoveryDispatch,
    )])
}

/// Os serviços da central para a Rede, para o `shared_store`.
#[must_use]
pub fn network_central() -> network::Central {
    network::Central::new(Arc::new(NetworkCentral))
}

/// O acesso ao equipamento que a Rede oferece ao núcleo: a identidade SNMP,
/// o leitor de Wi-Fi, a credencial salva e a shell da ativação do log.
#[must_use]
pub fn device_access() -> Arc<dyn DeviceAccess> {
    Arc::new(netmonitor_network::device_access::NetworkDeviceAccess)
}

/// O classificador do processo (ADR 017, `Classifier`): o Decisor do
/// assistente. Sem ele, o núcleo usa a heurística.
#[must_use]
pub fn classifier() -> Arc<dyn Classifier> {
    Arc::new(netmonitor_assistant::DeciderClassifier)
}

/// O catálogo de ferramentas da IA, na ordem em que a IA as vê (a ordem é
/// contrato: o prefixo em cache no provedor e o `contracts/ai_chat.json`
/// dependem dela). Cada área contribui as suas; o assistente, a pesquisa na
/// web.
#[must_use]
pub fn ai_tool_catalog() -> Vec<Box<dyn AiToolHandler>> {
    use netmonitor_servers::ai_tools as docker;

    vec![
        Box::new(inventory::SystemSummary),
        Box::new(inventory::ListDevices),
        Box::new(inventory::DeviceDetail),
        Box::new(inventory::DeviceInterfaces),
        Box::new(inventory::ListMonitors),
        Box::new(inventory::Alerts),
        Box::new(history::MonitorHistory),
        Box::new(history::DeviceMetrics),
        Box::new(analysis::RootCause),
        Box::new(analysis::BaselineComparison),
        Box::new(analysis::HourlyPattern),
        Box::new(analysis::IncidentTimeline),
        Box::new(charts::MonitorLatencyChart),
        Box::new(charts::InterfaceTrafficChart),
        Box::new(charts::DeviceMetricChart),
        Box::new(logs::LogsOverview),
        Box::new(grep::Grep::new(Arc::new(docker::DockerLogs))),
        Box::new(docker::DockerHosts),
        Box::new(docker::DockerContainers),
        Box::new(docker::DockerUsage),
        Box::new(docker::DockerContainerAction),
        Box::new(platform::PlatformStatus::new(vec![
            Box::new(netmonitor_vpn::ai_tools::VpnStatus),
            Box::new(netmonitor_network::discovery::ai_tools::DiscoveryStatus),
        ])),
        Box::new(netmonitor_network::topology::ai_tools::Topology),
        Box::new(audit::AuditLog),
        Box::new(inventory::SearchDocs),
        Box::new(netmonitor_assistant::tools::WebSearch),
        Box::new(diagnostics::Ping),
        Box::new(diagnostics::Traceroute),
        Box::new(diagnostics::ScanPorts),
        Box::new(diagnostics::DnsLookup),
        Box::new(diagnostics::Playbook),
        Box::new(actions::AcknowledgeAlert),
        Box::new(actions::SilenceAlert),
        Box::new(actions::CreateMaintenanceWindow),
        Box::new(actions::CreateMonitor),
        Box::new(alert_rules::AlertRulesGuide),
        Box::new(alert_rules::ListAlertRules),
        Box::new(alert_rules::ExplainAlert),
        Box::new(alert_rules::CreateAlertRule),
        Box::new(alert_rules::DeleteAlertRule),
        Box::new(alert_rules::ToggleAlertRule),
        Box::new(plugins::AuthoringGuide),
        Box::new(plugins::ListDevicePlugins),
        Box::new(plugins::ListPluginApps),
        Box::new(plugins::GetPlugin),
        Box::new(plugins::RunPluginTests),
        Box::new(plugins::FingerprintDevice),
        Box::new(plugins::DeviceSshExec),
        Box::new(plugins::DeviceHttpRequest),
        Box::new(plugins::SavePluginDraft),
        Box::new(plugins::RunPluginAction),
        Box::new(plugins::ValidatePlugin),
        Box::new(schedules::ListScheduledTasks),
        Box::new(schedules::CreateScheduledTask),
        Box::new(schedules::UpdateScheduledTask),
        Box::new(schedules::RunScheduledTask),
        Box::new(schedules::DeleteScheduledTask),
        Box::new(dashboard::GetDashboardLayout),
        Box::new(dashboard::UpdateDashboardLayout),
        Box::new(ask::AskUser),
    ]
}

/// Instala o catálogo de ferramentas no núcleo. Idempotente: o primeiro
/// vale (o `after_context` chama; os testes de unidade também podem).
pub fn install_ai_tools() {
    netmonitor_core::ai::tools::install_catalog(ai_tool_catalog);
}

/// Os containers do host desta central, para as marcações do `@`.
struct HostContainers(Arc<dyn DockerEngine>);

#[async_trait]
impl ContainerDirectory for HostContainers {
    async fn containers(&self) -> Option<Vec<MentionContainer>> {
        let listed = engine::list_containers(self.0.as_ref()).await.ok()?;
        Some(
            listed
                .iter()
                .map(|container| MentionContainer {
                    id: container.id.clone(),
                    name: container.display_name(),
                    image: container.image.clone(),
                    state: container.state.clone(),
                })
                .collect(),
        )
    }
}

/// O que o assistente pede ao resto da central: Web Push, canais de
/// notificação, inibição dos alertas, plugins de dispositivo e Docker.
struct AssistantCentral;

#[async_trait]
impl assistant::CentralServices for AssistantCentral {
    async fn push_to_user(
        &self,
        db: &DatabaseConnection,
        user_id: i64,
        payload: &Value,
    ) -> AppResult<u64> {
        Ok(deliver(db, Audience::User(user_id), payload).await?.sent)
    }

    async fn notify(&self, ctx: &AppContext, message: &NotificationMessage) {
        NotificationService::with_default_channels()
            .notify(ctx, message)
            .await;
    }

    async fn explaining_ancestor(
        &self,
        db: &DatabaseConnection,
        device_id: i64,
    ) -> AppResult<Option<i64>> {
        inhibition::explaining_ancestor(db, device_id).await
    }

    fn missing_device_access(&self, error: &AppError) -> Option<(i64, Value)> {
        credentials::missing_access_of(error)
            .map(|(device_id, kind)| (device_id, serde_json::to_value(kind).unwrap_or_default()))
    }

    async fn device_auto_accepted(
        &self,
        db: &DatabaseConnection,
        conversation_key: &str,
        device_id: i64,
    ) -> bool {
        auto_accept::active(db, conversation_key, device_id)
            .await
            .ok()
            .flatten()
            .is_some()
    }

    async fn container_names(&self, ctx: &AppContext) -> Vec<String> {
        netmonitor_servers::ai_tools::list_all_container_names(ctx).await
    }

    async fn local_containers(&self, ctx: &AppContext) -> Option<Box<dyn ContainerDirectory>> {
        let host = hosts::default_host(ctx).await.ok().flatten()?;
        Some(Box::new(HostContainers(host.engine)))
    }
}

/// Os serviços da central para o assistente, para o `shared_store`.
#[must_use]
pub fn assistant_central() -> assistant::Central {
    assistant::Central::new(Arc::new(AssistantCentral))
}

#[cfg(test)]
mod ai_catalog_tests;

#[cfg(test)]
mod tests {
    use netmonitor_core::areas::Area;

    use super::*;

    #[test]
    fn todas_as_areas_opcionais_tem_politica() {
        let areas: Vec<Area> = area_policies().iter().map(|policy| policy.area()).collect();
        for area in Area::OPTIONAL {
            assert!(areas.contains(&area), "{area} sem política");
        }
    }

    #[test]
    fn o_monitor_de_container_esta_registrado() {
        assert!(monitor_checkers(None).find("container").is_some());
        assert!(monitor_checkers(None).find("snmp").is_some());
    }
}
