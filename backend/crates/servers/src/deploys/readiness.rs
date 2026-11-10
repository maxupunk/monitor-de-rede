//! A checagem de prontidão: tudo o que falta para o primeiro deploy, de uma
//! vez, cada item com o que fazer.
//!
//! O operador não pode descobrir no meio do primeiro deploy que o agente não
//! libera `deploy`, que a raiz das stacks não está montada ou que um serviço
//! não tem healthcheck. A montagem dos itens é pura ([`checks`]); só a coleta
//! fala com o agente.

use loco_rs::app::AppContext;

use agent_protocol::{
    docker::stacks::{validate_project, StackReadiness},
    policy::Permission,
};
use netmonitor_core::{
    agents::{hub::AgentHub, identity, HostKey},
    models::{deploy_applications, probes},
    shared::{
        errors::{AppError, AppResult},
        text::count_noun,
    },
};
use sea_orm::EntityTrait;

use super::env_versions;
use crate::{
    docker::builds::BuilderReadiness,
    docker::{hosts, stacks::DEFAULT_STACKS_DIR},
    dtos::deploys::Builder,
    views::deploys::{DeployReadinessView, ReadinessCheck, ReadinessLevel},
};

/// Abaixo disso, avisar: cada versão de imagem ocupa centenas de MB.
const LOW_DISK_BYTES: u64 = 5 * 1024 * 1024 * 1024;

/// O que foi coletado.
pub struct Facts<'a> {
    pub agent_name: &'a str,
    pub local_agent: bool,
    pub connected: bool,
    pub allows_deploy: bool,
    pub stack: Option<&'a StackReadiness>,
    /// Erro ao perguntar ao agente.
    pub stack_error: Option<String>,
    pub project: &'a str,
    pub has_credential: Option<bool>,
    pub has_env: Option<bool>,
    /// Este agente compila a aplicação (ADR 023).
    pub build: Option<BuildFacts>,
}

/// O que se sabe do builder deste agente.
pub struct BuildFacts {
    pub allows_build: bool,
    pub readiness: Option<BuilderReadiness>,
    pub error: Option<String>,
}

fn build_checks(facts: &BuildFacts, allow_variable: &str, out: &mut Vec<ReadinessCheck>) {
    use ReadinessLevel::{Error, Ok, Warning};
    if !facts.allows_build {
        out.push(check(
            "build",
            "Permissão de compilar",
            Error,
            format!("A política local ({allow_variable}) não libera compilar neste servidor"),
            Some(format!(
                "No servidor, acrescente `build` a {allow_variable} e recrie o agente — ou                  escolha outro servidor em \"Onde compilar\""
            )),
        ));
        return;
    }
    out.push(check(
        "build",
        "Permissão de compilar",
        Ok,
        format!("A política local ({allow_variable}) libera compilar"),
        None,
    ));
    if let Some(error) = &facts.error {
        out.push(check(
            "builder",
            "Builder",
            Error,
            error.clone(),
            Some("Atualize o agente para a mesma versão da central".into()),
        ));
        return;
    }
    let Some(readiness) = &facts.readiness else {
        return;
    };
    out.push(match &readiness.buildx_version {
        Some(version) => check(
            "builder",
            "Builder (buildx)",
            Ok,
            if readiness.builder_ready {
                format!("{version} — builder dedicado pronto")
            } else {
                format!("{version} — o builder dedicado é criado na primeira compilação")
            },
            None,
        ),
        None => check(
            "builder",
            "Builder (buildx)",
            Error,
            "O plugin docker buildx não está instalado neste servidor".into(),
            Some("Use a imagem do agente NetMonitor (já traz o buildx) ou instale docker-buildx-plugin".into()),
        ),
    });
    if let Some(free) = readiness.free_bytes {
        if free < LOW_DISK_BYTES {
            out.push(check(
                "build_disk",
                "Espaço para compilar",
                Warning,
                format!("Só {} livres onde o agente compila", gib(free)),
                Some(
                    "Compilar usa o cache do BuildKit e os arquivos das imagens: libere espaço"
                        .into(),
                ),
            ));
        }
    }
    let limits = [readiness.cpu_limit.clone(), readiness.memory_limit.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    out.push(check(
        "build_limits",
        "Limites do builder",
        Ok,
        if limits.is_empty() {
            "Sem teto de CPU e memória: o builder disputa o servidor com as aplicações".into()
        } else {
            format!("Teto: {}", limits.join(", "))
        },
        limits.is_empty().then(|| {
            format!("Defina AGENT_BUILD_CPUS e AGENT_BUILD_MEMORY no agente ({allow_variable} fica igual)")
        }),
    ));
}

fn check(
    id: &str,
    label: &str,
    level: ReadinessLevel,
    detail: String,
    fix: Option<String>,
) -> ReadinessCheck {
    ReadinessCheck {
        id: id.to_string(),
        label: label.to_string(),
        level,
        detail,
        fix,
    }
}

fn gib(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let value = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    format!("{value:.1} GiB")
}

/// Os itens, na ordem em que o operador resolve.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn checks(facts: &Facts<'_>) -> Vec<ReadinessCheck> {
    use ReadinessLevel::{Error, Ok, Warning};
    let allow_variable = if facts.local_agent {
        "LOCAL_AGENT_ALLOW"
    } else {
        "AGENT_ALLOW"
    };
    let mut out = vec![if facts.connected {
        check(
            "agent",
            "Agente conectado",
            Ok,
            format!("{} está conectado à central", facts.agent_name),
            None,
        )
    } else {
        check(
            "agent",
            "Agente conectado",
            Error,
            format!("{} não está conectado agora", facts.agent_name),
            Some("Confira se o container netmonitor-agent está rodando no servidor".into()),
        )
    }];
    if facts.connected {
        out.push(if facts.allows_deploy {
            check(
                "permission",
                "Permissão de deploy",
                Ok,
                format!("A política local ({allow_variable}) libera deploy"),
                None,
            )
        } else {
            check(
                "permission",
                "Permissão de deploy",
                Error,
                format!("A política local ({allow_variable}) não libera deploy"),
                Some(format!(
                    "No servidor, acrescente `deploy` a {allow_variable} e recrie o agente"
                )),
            )
        });
    }

    if let (true, Some(build)) = (facts.connected, &facts.build) {
        build_checks(build, allow_variable, &mut out);
    }

    if let Some(error) = &facts.stack_error {
        out.push(check(
            "stack",
            "Resposta do agente",
            Error,
            error.clone(),
            Some("Atualize o agente para a mesma versão da central".into()),
        ));
    }
    if let Some(stack) = facts.stack {
        out.push(match (&stack.stacks_dir, &stack.stacks_dir_issue) {
            (Some(dir), _) => check(
                "stacks_dir",
                "Diretório das stacks",
                Ok,
                format!("{dir} montado e gravável"),
                None,
            ),
            (None, issue) => check(
                "stacks_dir",
                "Diretório das stacks",
                Error,
                issue
                    .clone()
                    .unwrap_or_else(|| "Diretório indisponível".into()),
                Some(format!(
                    "No compose do agente, monte o diretório no mesmo caminho: \
                     `- {DEFAULT_STACKS_DIR}:{DEFAULT_STACKS_DIR}`"
                )),
            ),
        });
        out.push(match &stack.git_version {
            Some(version) => check("git", "Git no agente", Ok, version.clone(), None),
            None => check(
                "git",
                "Git no agente",
                Error,
                "O agente não tem o git".into(),
                Some("Atualize a imagem do agente: as versões com deploy já trazem o git".into()),
            ),
        });
        out.push(match &stack.compose_version {
            Some(version) => check(
                "compose",
                "Docker Compose",
                Ok,
                format!("Compose {version}"),
                None,
            ),
            None => check(
                "compose",
                "Docker Compose",
                Error,
                "O plugin docker compose não está disponível no agente".into(),
                Some("Atualize a imagem do agente".into()),
            ),
        });
        if let Some(free) = stack.free_bytes {
            out.push(if free < LOW_DISK_BYTES {
                check(
                    "disk",
                    "Espaço em disco",
                    Warning,
                    format!("{} livres", gib(free)),
                    Some("Libere espaço ou reduza as versões guardadas para o rollback".into()),
                )
            } else {
                check(
                    "disk",
                    "Espaço em disco",
                    Ok,
                    format!("{} livres", gib(free)),
                    None,
                )
            });
        }
        if stack.protected {
            out.push(check(
                "protected",
                "Projeto da instalação",
                Error,
                format!(
                    "\"{}\" é o projeto do próprio NetMonitor: o deploy derrubaria a central",
                    facts.project
                ),
                Some("Escolha outro nome de projeto".into()),
            ));
        }
        if let Some(dir) = &stack.running_working_dir {
            let managed = stack
                .stacks_dir
                .as_ref()
                .is_some_and(|root| dir.starts_with(root.as_str()));
            if !managed {
                out.push(check(
                    "adopt",
                    "Projeto em outro diretório",
                    Warning,
                    format!(
                        "\"{}\" roda hoje de {dir}. No primeiro deploy ele passa a rodar do \
                         diretório das stacks, com os mesmos containers e volumes nomeados",
                        facts.project
                    ),
                    Some(
                        "Pastas montadas que não estão no repositório (uploads, ./storage) \
                         precisam ser copiadas para o novo diretório antes"
                            .into(),
                    ),
                ));
            }
        }
        if !stack.services_without_healthcheck.is_empty() {
            out.push(check(
                "healthcheck",
                "Healthchecks",
                Warning,
                format!(
                    "{} sem healthcheck: {}",
                    count_noun(
                        u64::try_from(stack.services_without_healthcheck.len()).unwrap_or(0),
                        "serviço",
                        "serviços"
                    ),
                    stack.services_without_healthcheck.join(", ")
                ),
                Some(
                    "Sem healthcheck, o deploy só confere que o container subiu — não que \
                     responde. Declare um no docker-compose.yml"
                        .into(),
                ),
            ));
        }
    }
    if let Some(has_credential) = facts.has_credential {
        if !has_credential {
            out.push(check(
                "credential",
                "Acesso ao GitHub",
                Warning,
                "A aplicação não tem conexão com o GitHub nem token".into(),
                Some(
                    "Só funciona com repositório e imagens públicos. Para privados, conecte o \
                     GitHub (Deploys → Conexões) e, para imagens privadas, cadastre um token \
                     clássico com read:packages"
                        .into(),
                ),
            ));
        }
    }
    if let Some(has_env) = facts.has_env {
        if !has_env {
            out.push(check(
                "env",
                "Variáveis (.env)",
                Warning,
                "O alvo ainda não tem .env salvo".into(),
                Some("Cole o .env de produção em Variáveis antes do primeiro deploy".into()),
            ));
        }
    }
    out
}

/// Coleta e monta a checagem de um agente e projeto — e, com `target_id`,
/// também da aplicação e do `.env` dele.
///
/// # Errors
///
/// `Validation` para projeto inválido; `404` para agente inexistente.
pub async fn inspect(
    ctx: &AppContext,
    probe_id: i64,
    project: &str,
    application: Option<&deploy_applications::Model>,
    target_id: Option<i64>,
) -> AppResult<DeployReadinessView> {
    validate_project(project).map_err(|error| AppError::validation(error.to_string()))?;
    let agent = probes::Entity::find_by_id(probe_id)
        .one(&ctx.db)
        .await?
        .filter(identity::is_agent)
        .ok_or_else(|| AppError::not_found("Agente não encontrado"))?;
    let session = AgentHub::from_context(ctx)
        .ok()
        .and_then(|hub| hub.get(probe_id));
    let connected = session.is_some();
    let allows_deploy = session
        .as_ref()
        .is_some_and(|session| session.allows(Permission::Deploy));
    let (stack, stack_error) = if connected && allows_deploy {
        match hosts::resolve(ctx, HostKey::agent(probe_id)) {
            Ok(host) => match host.stacks.stack_readiness(project).await {
                Ok(stack) => (Some(stack), None),
                Err(error) => (None, Some(error.to_string())),
            },
            Err(error) => (None, Some(error.to_string())),
        }
    } else {
        (None, None)
    };
    let compiles_here = application.is_some_and(|app| {
        Builder::parse(&app.builder) == Builder::Netmonitor
            && app.build_probe_id.is_none_or(|id| id == probe_id)
    });
    let build = if compiles_here {
        let allows_build = session
            .as_ref()
            .is_some_and(|session| session.allows(Permission::Build));
        let (readiness, error) = if connected {
            match hosts::resolve(ctx, HostKey::agent(probe_id)) {
                Ok(host) => match host.images.builder_readiness().await {
                    Ok(readiness) => (Some(readiness), None),
                    Err(error) => (None, Some(error.to_string())),
                },
                Err(error) => (None, Some(error.to_string())),
            }
        } else {
            (None, None)
        };
        Some(BuildFacts {
            allows_build,
            readiness,
            error,
        })
    } else {
        None
    };
    let has_env = match target_id {
        Some(id) => Some(env_versions::current(&ctx.db, id).await?.is_some()),
        None => None,
    };
    let checks = checks(&Facts {
        agent_name: &agent.name,
        local_agent: identity::is_local(&agent),
        connected,
        allows_deploy,
        stack: stack.as_ref(),
        stack_error,
        project,
        has_credential: application
            .map(|app| app.credential_id.is_some() || app.github_connection_id.is_some()),
        has_env,
        build,
    });
    Ok(DeployReadinessView {
        ready: checks
            .iter()
            .all(|check| check.level != ReadinessLevel::Error),
        checks,
        stack,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack() -> StackReadiness {
        StackReadiness {
            stacks_dir: Some("/srv/netmonitor/stacks".into()),
            stacks_dir_issue: None,
            git_version: Some("git version 2.47".into()),
            compose_version: Some("2.32.0".into()),
            free_bytes: Some(50 * 1024 * 1024 * 1024),
            project_dir_exists: false,
            running_working_dir: Some("/opt/sgc".into()),
            services_without_healthcheck: vec!["sgc-front-menu".into()],
            protected: false,
        }
    }

    fn facts(stack: &StackReadiness) -> Facts<'_> {
        Facts {
            agent_name: "debian-prod",
            local_agent: false,
            connected: true,
            allows_deploy: true,
            stack: Some(stack),
            stack_error: None,
            project: "sgc",
            has_credential: Some(true),
            has_env: Some(true),
            build: None,
        }
    }

    #[test]
    fn compilar_sem_permissao_ou_sem_buildx_e_erro() {
        let stack = stack();
        let mut without = facts(&stack);
        without.build = Some(BuildFacts {
            allows_build: false,
            readiness: None,
            error: None,
        });
        let found = checks(&without);
        assert_eq!(level(&found, "build"), Some(ReadinessLevel::Error));
        let fix = found
            .iter()
            .find(|check| check.id == "build")
            .unwrap()
            .fix
            .clone()
            .unwrap();
        assert!(fix.contains("AGENT_ALLOW"));

        let mut no_buildx = facts(&stack);
        no_buildx.build = Some(BuildFacts {
            allows_build: true,
            readiness: Some(BuilderReadiness::default()),
            error: None,
        });
        let found = checks(&no_buildx);
        assert_eq!(level(&found, "build"), Some(ReadinessLevel::Ok));
        assert_eq!(level(&found, "builder"), Some(ReadinessLevel::Error));
    }

    fn level(checks: &[ReadinessCheck], id: &str) -> Option<ReadinessLevel> {
        checks
            .iter()
            .find(|check| check.id == id)
            .map(|check| check.level)
    }

    #[test]
    fn host_pronto_avisa_adocao_e_healthcheck() {
        let stack = stack();
        let checks = checks(&facts(&stack));
        assert!(checks
            .iter()
            .all(|check| check.level != ReadinessLevel::Error));
        assert_eq!(level(&checks, "adopt"), Some(ReadinessLevel::Warning));
        assert_eq!(level(&checks, "healthcheck"), Some(ReadinessLevel::Warning));
    }

    #[test]
    fn sem_permissao_diz_qual_variavel_mudar() {
        let stack = stack();
        let mut facts = facts(&stack);
        facts.allows_deploy = false;
        facts.stack = None;
        facts.local_agent = true;
        let checks = checks(&facts);
        let permission = checks
            .iter()
            .find(|check| check.id == "permission")
            .expect("permissão");
        assert_eq!(permission.level, ReadinessLevel::Error);
        assert!(permission
            .fix
            .as_deref()
            .unwrap_or_default()
            .contains("LOCAL_AGENT_ALLOW"));
    }

    #[test]
    fn raiz_sem_montagem_e_erro_com_a_linha_do_compose() {
        let mut stack = stack();
        stack.stacks_dir = None;
        stack.stacks_dir_issue = Some("não existe".into());
        stack.running_working_dir = None;
        let checks = checks(&facts(&stack));
        let dir = checks
            .iter()
            .find(|check| check.id == "stacks_dir")
            .expect("dir");
        assert_eq!(dir.level, ReadinessLevel::Error);
        assert!(dir
            .fix
            .as_deref()
            .unwrap_or_default()
            .contains(DEFAULT_STACKS_DIR));
    }

    #[test]
    fn projeto_ja_no_diretorio_das_stacks_nao_e_adocao() {
        let mut stack = stack();
        stack.running_working_dir = Some("/srv/netmonitor/stacks/sgc".into());
        assert_eq!(level(&checks(&facts(&stack)), "adopt"), None);
    }
}
