//! Escopos fechados do servidor MCP e autorização em 3 camadas ao vivo.
//!
//! Toda listagem (`tools/list`) e chamada (`tools/call`) aplica:
//! 1. Filtro de segurança MCP (exclui `Interactive`, `DeviceAccess` e segredos);
//! 2. Papel ao vivo do dono do token (`user.active` + `AreaAccess` + áreas ligadas);
//! 3. Conjunto de escopos concedidos no token (e subação em `deploy_action`).

use serde::{Deserialize, Serialize};

use netmonitor_core::{
    ai::{
        args::ToolArgs,
        tools::{AiToolHandler, McpExposure, ToolKind, ToolPolicy},
    },
    areas::{Area, AreaSet},
    roles::{AreaAccess, Role},
    shared::errors::{AppError, AppResult},
};

/// Escopos fechados que um token pessoal MCP pode carregar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum McpScope {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "diagnostics")]
    Diagnostics,
    #[serde(rename = "deploys:read")]
    DeploysRead,
    #[serde(rename = "deploys:write")]
    DeploysWrite,
    #[serde(rename = "deploys:approve")]
    DeploysApprove,
    #[serde(rename = "docker:write")]
    DockerWrite,
    #[serde(rename = "operations:write")]
    OperationsWrite,
}

impl McpScope {
    /// Todos os escopos na ordem estável apresentada pela API e pela tela.
    pub const ALL: [Self; 7] = [
        Self::Read,
        Self::Diagnostics,
        Self::DeploysRead,
        Self::DeploysWrite,
        Self::DeploysApprove,
        Self::DockerWrite,
        Self::OperationsWrite,
    ];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Diagnostics => "diagnostics",
            Self::DeploysRead => "deploys:read",
            Self::DeploysWrite => "deploys:write",
            Self::DeploysApprove => "deploys:approve",
            Self::DockerWrite => "docker:write",
            Self::OperationsWrite => "operations:write",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "read" => Some(Self::Read),
            "diagnostics" => Some(Self::Diagnostics),
            "deploys:read" => Some(Self::DeploysRead),
            "deploys:write" => Some(Self::DeploysWrite),
            "deploys:approve" => Some(Self::DeploysApprove),
            "docker:write" => Some(Self::DockerWrite),
            "operations:write" => Some(Self::OperationsWrite),
            _ => None,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Read => "Leitura geral",
            Self::Diagnostics => "Diagnósticos ativos",
            Self::DeploysRead => "Deploys (leitura)",
            Self::DeploysWrite => "Deploys (execução)",
            Self::DeploysApprove => "Deploys (aprovação)",
            Self::DockerWrite => "Containers Docker",
            Self::OperationsWrite => "Operações no núcleo",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Read => {
                "Consultar dispositivos, monitores, alertas, métricas, logs, topologia e inventário."
            }
            Self::Diagnostics => {
                "Executar ping, traceroute, DNS, varredura de portas e playbooks de diagnóstico."
            }
            Self::DeploysRead => {
                "Consultar aplicações, prontidão, execuções, compilações e logs de deploy."
            }
            Self::DeploysWrite => {
                "Iniciar compilações, publicar revisões, restaurar versões e cancelar deploys ou builds."
            }
            Self::DeploysApprove => {
                "Aprovar ou recusar deploys pendentes que exigem aprovação manual."
            }
            Self::DockerWrite => "Iniciar, parar ou reiniciar containers nos hosts monitorados.",
            Self::OperationsWrite => {
                "Reconhecer/silenciar alertas, criar monitores, regras, janelas de manutenção, tarefas agendadas e layout."
            }
        }
    }

    /// Escopo exclusivo de administradores.
    #[must_use]
    pub const fn requires_admin(self) -> bool {
        matches!(
            self,
            Self::DeploysWrite | Self::DeploysApprove | Self::DockerWrite
        )
    }

    /// O papel/acesso atual do usuário permite conceder este escopo a um token?
    #[must_use]
    pub fn granted_by_access(self, access: &AreaAccess) -> bool {
        match self {
            Self::Read | Self::Diagnostics => true,
            Self::DeploysRead => access.role_in(Area::Servers).is_some(),
            Self::DeploysWrite | Self::DeploysApprove | Self::DockerWrite => {
                access.role_in(Area::Servers) == Some(Role::Admin)
            }
            Self::OperationsWrite => access.global().can_write(),
        }
    }
}

/// Camada 1: a ferramenta é elegível para exposição via MCP?
///
/// Nunca expõe `ToolKind::Interactive` (`ask_user`), `ToolKind::DeviceAccess`
/// (ADR 012) nem o que a própria ferramenta declara [`McpExposure::Hidden`].
#[must_use]
pub fn is_mcp_safe(handler: &dyn AiToolHandler) -> bool {
    !matches!(
        handler.kind(),
        ToolKind::Interactive | ToolKind::DeviceAccess
    ) && handler.mcp_exposure() != McpExposure::Hidden
}

/// Escopo MCP exigido para uma ferramenta (exceto a ação de deploy, que
/// depende da subação). Retorna `None` para ferramentas excluídas ou não
/// mapeadas (fail-closed).
#[must_use]
pub fn scope_for_tool(handler: &dyn AiToolHandler) -> Option<McpScope> {
    if !is_mcp_safe(handler) {
        return None;
    }
    match handler.mcp_exposure() {
        McpExposure::Hidden => None,
        McpExposure::DeploysRead => Some(McpScope::DeploysRead),
        McpExposure::DeploysAction => Some(McpScope::DeploysWrite),
        McpExposure::ByKind => match handler.kind() {
            ToolKind::Passive => Some(McpScope::Read),
            ToolKind::Active => Some(McpScope::Diagnostics),
            ToolKind::ContainerAction => Some(McpScope::DockerWrite),
            ToolKind::Action if handler.area() == Area::Core => Some(McpScope::OperationsWrite),
            ToolKind::Action | ToolKind::Interactive | ToolKind::DeviceAccess => None,
        },
    }
}

/// Camada 2: o usuário vivo tem permissão para esta ferramenta na área dela?
///
/// A mesma regra do chat ([`ToolPolicy::with_access`] + [`ToolPolicy::admits`]:
/// área ligada e visível, escrita só onde o papel escreve), mais o que o chat
/// deixa para o guard da API e aqui não há: escrita no Núcleo exige o papel
/// global de escrita, e `admin_only` exige administrador na área.
#[must_use]
pub fn user_may_use(
    handler: &dyn AiToolHandler,
    user_active: bool,
    access: &AreaAccess,
    enabled_areas: AreaSet,
) -> bool {
    if !user_active || !handler.enabled() || !is_mcp_safe(handler) {
        return false;
    }
    let (kind, area) = (handler.kind(), handler.area());
    let policy = ToolPolicy {
        areas: enabled_areas,
        ..ToolPolicy::passive()
    }
    .with_access(access);
    if !policy.admits(kind, area) {
        return false;
    }
    if handler.admin_only() && access.role_in(area) != Some(Role::Admin) {
        return false;
    }
    let writes = matches!(kind, ToolKind::Action | ToolKind::ContainerAction);
    !(writes && area == Area::Core) || access.global().can_write()
}

fn is_deploy_action(handler: &dyn AiToolHandler) -> bool {
    handler.mcp_exposure() == McpExposure::DeploysAction
}

/// Camada 3 (listagem): o token possui escopo para enxergar esta ferramenta?
#[must_use]
pub fn token_allows_listing(handler: &dyn AiToolHandler, token_scopes: &[McpScope]) -> bool {
    if !is_mcp_safe(handler) {
        return false;
    }
    if is_deploy_action(handler) {
        return token_scopes.contains(&McpScope::DeploysWrite)
            || token_scopes.contains(&McpScope::DeploysApprove);
    }
    scope_for_tool(handler).is_some_and(|scope| token_scopes.contains(&scope))
}

/// Autorização completa em 3 camadas para `tools/list`.
#[must_use]
pub fn authorize_listing(
    handler: &dyn AiToolHandler,
    user_active: bool,
    access: &AreaAccess,
    enabled_areas: AreaSet,
    token_scopes: &[McpScope],
) -> bool {
    user_may_use(handler, user_active, access, enabled_areas)
        && token_allows_listing(handler, token_scopes)
}

/// Autorização completa em 3 camadas para `tools/call`, incluindo a separação
/// estrita entre `deploys:write` e `deploys:approve` em `deploy_action`.
///
/// # Errors
///
/// `Forbidden` explicando qual camada bloqueou a chamada.
pub fn authorize_call(
    handler: &dyn AiToolHandler,
    args: &ToolArgs,
    user_active: bool,
    access: &AreaAccess,
    enabled_areas: AreaSet,
    token_scopes: &[McpScope],
) -> AppResult<()> {
    if !handler.enabled() || !is_mcp_safe(handler) {
        return Err(AppError::forbidden(format!(
            "A ferramenta '{}' não está disponível pelo servidor MCP.",
            handler.name()
        )));
    }
    if !user_may_use(handler, user_active, access, enabled_areas) {
        return Err(AppError::forbidden(format!(
            "O usuário dono do token não tem permissão para executar '{}'.",
            handler.name()
        )));
    }
    if is_deploy_action(handler) {
        let action = args
            .text("action")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match action.as_str() {
            "approve" | "reject" => {
                if !token_scopes.contains(&McpScope::DeploysApprove) {
                    return Err(AppError::forbidden(
                        "Este token MCP não possui o escopo 'deploys:approve' para aprovar ou recusar deploys.",
                    ));
                }
                return Ok(());
            }
            "deploy" | "build" | "restore" | "cancel" => {
                if !token_scopes.contains(&McpScope::DeploysWrite) {
                    return Err(AppError::forbidden(
                        "Este token MCP não possui o escopo 'deploys:write' para executar ações de deploy.",
                    ));
                }
                return Ok(());
            }
            _ => {
                return Err(AppError::forbidden(format!(
                    "Subação de deploy desconhecida ou não autorizada via MCP: '{action}'."
                )));
            }
        }
    }
    if !token_allows_listing(handler, token_scopes) {
        let needed = scope_for_tool(handler).map_or("desconhecido", McpScope::id);
        return Err(AppError::forbidden(format!(
            "Este token MCP não possui o escopo '{needed}' necessário para '{}'.",
            handler.name()
        )));
    }
    Ok(())
}

/// Conta quantas ferramentas MCP o usuário consegue expor com seu papel atual.
#[must_use]
pub fn available_tools_count(
    handlers: &[Box<dyn AiToolHandler>],
    user_active: bool,
    access: &AreaAccess,
    enabled_areas: AreaSet,
) -> usize {
    let granted_scopes: Vec<McpScope> = McpScope::ALL
        .into_iter()
        .filter(|scope| scope.granted_by_access(access))
        .collect();
    handlers
        .iter()
        .filter(|handler| {
            authorize_listing(
                handler.as_ref(),
                user_active,
                access,
                enabled_areas,
                &granted_scopes,
            )
        })
        .count()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use async_trait::async_trait;
    use loco_rs::prelude::AppContext;
    use serde_json::{json, Value};

    use netmonitor_core::ai::tools::ToolOutput;

    use super::*;

    struct StubTool {
        name: &'static str,
        area: Area,
        kind: ToolKind,
        admin: bool,
        exposure: McpExposure,
    }

    #[async_trait]
    impl AiToolHandler for StubTool {
        fn name(&self) -> &'static str {
            self.name
        }
        fn area(&self) -> Area {
            self.area
        }
        fn description(&self) -> &'static str {
            "stub"
        }
        fn parameters(&self) -> Value {
            json!({ "type": "object", "properties": {} })
        }
        fn kind(&self) -> ToolKind {
            self.kind
        }
        fn admin_only(&self) -> bool {
            self.admin
        }
        fn mcp_exposure(&self) -> McpExposure {
            self.exposure
        }
        async fn execute(&self, _ctx: &AppContext, _args: &ToolArgs) -> AppResult<ToolOutput> {
            Ok(ToolOutput::data(json!({ "ok": true })))
        }
    }

    fn access(global: Role, areas: &[(Area, Role)]) -> AreaAccess {
        AreaAccess::new(global, areas.iter().copied().collect::<BTreeMap<_, _>>())
    }

    #[test]
    fn exclui_interactive_device_access_e_segredos() {
        let ask = StubTool {
            name: "ask_user",
            area: Area::Core,
            kind: ToolKind::Interactive,
            admin: false,
            exposure: McpExposure::ByKind,
        };
        let ssh = StubTool {
            name: "device_ssh_exec",
            area: Area::Network,
            kind: ToolKind::DeviceAccess,
            admin: true,
            exposure: McpExposure::ByKind,
        };
        let secret = StubTool {
            name: "reveal_wifi_secret",
            area: Area::Network,
            kind: ToolKind::Passive,
            admin: true,
            exposure: McpExposure::Hidden,
        };
        let plugin_tests = StubTool {
            name: "run_plugin_tests",
            area: Area::Network,
            kind: ToolKind::Passive,
            admin: false,
            exposure: McpExposure::Hidden,
        };
        assert!(!is_mcp_safe(&ask));
        assert!(!is_mcp_safe(&ssh));
        assert!(!is_mcp_safe(&secret));
        assert_eq!(scope_for_tool(&plugin_tests), None);
        assert_eq!(scope_for_tool(&ask), None);
        assert_eq!(scope_for_tool(&ssh), None);
        assert_eq!(scope_for_tool(&secret), None);
    }

    #[test]
    fn mapeia_escopos_corretamente() {
        let deploy_read = StubTool {
            name: "get_deploy_readiness",
            area: Area::Servers,
            kind: ToolKind::Passive,
            admin: false,
            exposure: McpExposure::DeploysRead,
        };
        let docker_write = StubTool {
            name: "docker_container_action",
            area: Area::Servers,
            kind: ToolKind::ContainerAction,
            admin: true,
            exposure: McpExposure::ByKind,
        };
        let ping = StubTool {
            name: "ping_host",
            area: Area::Core,
            kind: ToolKind::Active,
            admin: false,
            exposure: McpExposure::ByKind,
        };
        let silence = StubTool {
            name: "silence_alert",
            area: Area::Core,
            kind: ToolKind::Action,
            admin: false,
            exposure: McpExposure::ByKind,
        };
        let summary = StubTool {
            name: "get_system_summary",
            area: Area::Core,
            kind: ToolKind::Passive,
            admin: false,
            exposure: McpExposure::ByKind,
        };

        assert_eq!(scope_for_tool(&deploy_read), Some(McpScope::DeploysRead));
        assert_eq!(scope_for_tool(&docker_write), Some(McpScope::DockerWrite));
        assert_eq!(scope_for_tool(&ping), Some(McpScope::Diagnostics));
        assert_eq!(scope_for_tool(&silence), Some(McpScope::OperationsWrite));
        assert_eq!(scope_for_tool(&summary), Some(McpScope::Read));
    }

    #[test]
    fn bloqueia_viewer_em_escrita_core_e_nao_admin_em_admin_only() {
        let admin_access = access(Role::Admin, &[]);
        let op_access = access(Role::Operator, &[(Area::Servers, Role::Operator)]);
        let viewer_access = access(Role::Viewer, &[(Area::Servers, Role::Viewer)]);

        let silence = StubTool {
            name: "silence_alert",
            area: Area::Core,
            kind: ToolKind::Action,
            admin: false,
            exposure: McpExposure::ByKind,
        };
        let audit_log = StubTool {
            name: "get_audit_log",
            area: Area::Core,
            kind: ToolKind::Passive,
            admin: true,
            exposure: McpExposure::ByKind,
        };
        let deploy_action = StubTool {
            name: "deploy_action",
            area: Area::Servers,
            kind: ToolKind::Action,
            admin: true,
            exposure: McpExposure::DeploysAction,
        };

        // Viewer nunca grava no Core mesmo se token carregasse OperationsWrite
        assert!(!authorize_listing(
            &silence,
            true,
            &viewer_access,
            AreaSet::ALL,
            &[McpScope::OperationsWrite],
        ));
        assert!(authorize_listing(
            &silence,
            true,
            &op_access,
            AreaSet::ALL,
            &[McpScope::OperationsWrite],
        ));

        // admin_only bloqueia Operator e libera Admin
        assert!(!authorize_listing(
            &audit_log,
            true,
            &op_access,
            AreaSet::ALL,
            &[McpScope::Read],
        ));
        assert!(authorize_listing(
            &audit_log,
            true,
            &admin_access,
            AreaSet::ALL,
            &[McpScope::Read],
        ));

        // Separação deploys:write vs deploys:approve em deploy_action
        let approve_args = ToolArgs::from_value(json!({ "action": "approve", "runId": 1 }));
        let deploy_args = ToolArgs::from_value(json!({ "action": "deploy", "targetId": 1 }));
        assert!(authorize_call(
            &deploy_action,
            &approve_args,
            true,
            &admin_access,
            AreaSet::ALL,
            &[McpScope::DeploysWrite],
        )
        .is_err());
        assert!(authorize_call(
            &deploy_action,
            &approve_args,
            true,
            &admin_access,
            AreaSet::ALL,
            &[McpScope::DeploysApprove],
        )
        .is_ok());
        assert!(authorize_call(
            &deploy_action,
            &deploy_args,
            true,
            &admin_access,
            AreaSet::ALL,
            &[McpScope::DeploysApprove],
        )
        .is_err());
        assert!(authorize_call(
            &deploy_action,
            &deploy_args,
            true,
            &admin_access,
            AreaSet::ALL,
            &[McpScope::DeploysWrite],
        )
        .is_ok());

        // Fail-closed: subação desconhecida nunca passa, mesmo com ambos os escopos
        let unknown_args = ToolArgs::from_value(json!({ "action": "promote", "targetId": 1 }));
        assert!(authorize_call(
            &deploy_action,
            &unknown_args,
            true,
            &admin_access,
            AreaSet::ALL,
            &[McpScope::DeploysWrite, McpScope::DeploysApprove],
        )
        .is_err());
    }

    #[test]
    fn bloqueia_escalacao_de_privilegio_na_criacao_de_escopos() {
        let viewer = access(Role::Viewer, &[]);
        assert!(McpScope::Read.granted_by_access(&viewer));
        assert!(McpScope::Diagnostics.granted_by_access(&viewer));
        assert!(!McpScope::DeploysRead.granted_by_access(&viewer));
        assert!(!McpScope::OperationsWrite.granted_by_access(&viewer));
        assert!(!McpScope::DeploysWrite.granted_by_access(&viewer));
        assert!(!McpScope::DeploysApprove.granted_by_access(&viewer));
        assert!(!McpScope::DockerWrite.granted_by_access(&viewer));

        let op_servers = access(Role::Operator, &[(Area::Servers, Role::Operator)]);
        assert!(McpScope::DeploysRead.granted_by_access(&op_servers));
        assert!(McpScope::OperationsWrite.granted_by_access(&op_servers));
        assert!(!McpScope::DeploysWrite.granted_by_access(&op_servers));
        assert!(!McpScope::DeploysApprove.granted_by_access(&op_servers));
        assert!(!McpScope::DockerWrite.granted_by_access(&op_servers));

        let admin = access(Role::Admin, &[]);
        for scope in McpScope::ALL {
            assert!(scope.granted_by_access(&admin), "{scope:?}");
        }
    }
}
