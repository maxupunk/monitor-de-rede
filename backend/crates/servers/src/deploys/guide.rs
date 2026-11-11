//! O caminho até a primeira versão no ar: GitHub → agente → aplicação → alvo
//! → `.env` → primeiro deploy. Uma fonte só para a tela (o guia no topo de
//! Deploys, que anda sozinho pelo SSE) e para a IA (`get_deploy_guide`).
//!
//! [`facts_of`] lê o que a visão geral já carregou; [`steps`] é puro e decide
//! o estado e o texto de cada etapa.

use crate::dtos::deploys::Builder;
use crate::{
    deploys::{builds::BuildStatus, RunStatus},
    views::deploys::{
        DeployAgentView, DeployGithubConnectionView, DeployGuideAction, DeployGuideStatus,
        DeployGuideStep, DeployGuideStepKey, DeployGuideView, DeployOverview, DeployRunView,
    },
};

/// O que o guia precisa saber, já tirado da visão geral.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub github: Option<GithubFact>,
    pub agents: Vec<AgentFact>,
    pub applications: Vec<ApplicationFact>,
    /// O deploy mais recente (de qualquer alvo).
    pub latest_run: Option<RunFact>,
    /// A compilação mais recente (de qualquer aplicação).
    pub latest_build: Option<BuildFact>,
}

#[derive(Debug, Clone)]
pub struct GithubFact {
    pub account: String,
    pub installed: bool,
    pub install_url: String,
}

#[derive(Debug, Clone)]
pub struct AgentFact {
    pub name: String,
    pub connected: bool,
    pub allows_deploy: bool,
    pub allows_build: bool,
}

#[derive(Debug, Clone)]
pub struct ApplicationFact {
    pub id: i64,
    pub name: String,
    /// O NetMonitor compila (ADR 023), em vez do GitHub Actions.
    pub compiles: bool,
    pub targets: Vec<TargetFact>,
}

#[derive(Debug, Clone)]
pub struct TargetFact {
    pub id: i64,
    pub name: String,
    pub reference: String,
    pub has_env: bool,
    /// O commit no ar, curto.
    pub live: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunFact {
    pub id: i64,
    pub status: RunStatus,
    pub target: String,
    pub short_sha: String,
}

#[derive(Debug, Clone)]
pub struct BuildFact {
    pub status: BuildStatus,
    pub short_sha: String,
}

/// Os fatos da visão geral (que a tela já recebe) e dos agentes.
#[must_use]
pub fn facts_of(
    overview: &DeployOverview,
    agents: &[DeployAgentView],
    latest_build: Option<BuildFact>,
) -> Facts {
    let github = pick_connection(&overview.github_connections).map(|connection| GithubFact {
        account: connection.account.clone(),
        installed: connection.installed,
        install_url: connection.install_url.clone(),
    });
    Facts {
        github,
        agents: agents
            .iter()
            .map(|agent| AgentFact {
                name: agent.name.clone(),
                connected: agent.connected,
                allows_deploy: agent.allows_deploy,
                allows_build: agent.allows_build,
            })
            .collect(),
        applications: overview
            .applications
            .iter()
            .map(|application| ApplicationFact {
                id: application.id,
                name: application.name.clone(),
                compiles: application.builder == Builder::Netmonitor,
                targets: application
                    .targets
                    .iter()
                    .map(|target| TargetFact {
                        id: target.id,
                        name: target.name.clone(),
                        reference: target.reference.clone(),
                        has_env: target.env.is_some(),
                        live: target.live.as_ref().map(|run| run.short_sha.clone()),
                    })
                    .collect(),
            })
            .collect(),
        latest_run: overview.recent_runs.first().map(run_fact),
        latest_build,
    }
}

/// A conexão que vale para o guia: a primeira instalada, ou a primeira.
fn pick_connection(
    connections: &[DeployGithubConnectionView],
) -> Option<&DeployGithubConnectionView> {
    connections
        .iter()
        .find(|connection| connection.installed)
        .or_else(|| connections.first())
}

fn run_fact(run: &DeployRunView) -> RunFact {
    RunFact {
        id: run.id,
        status: run.status,
        target: run.target_name.clone(),
        short_sha: run.short_sha.clone(),
    }
}

/// Como fazer cada etapa: o texto fixo que a tela mostra e a IA explica.
const HOW_GITHUB: &str = "Clique em Conectar GitHub: a central cria o GitHub App dela na sua conta (ou organização) num clique. Na instalação, escolha só os repositórios que vão ter deploy. É por esse App que o push avisa a central e o servidor clona o código, com um token de uma hora e só de leitura — nada de token na URL.";
const HOW_AGENT: &str = "Instale o agente do NetMonitor no servidor (Servidores remotos → Adicionar servidor) e libere deploy e build no AGENT_ALLOW dele. O agente só disca para fora: nenhuma porta é aberta no servidor. É ele que clona, compila num builder isolado, recebe a imagem e sobe o compose.";
const HOW_APPLICATION: &str = "Em Automatizar deploy, escolha o repositório e deixe \"O NetMonitor compila\". Confira a imagem de cada serviço do compose: o Dockerfile, o estágio final e o nome do image: — que precisa usar a tag do commit, como image: minha-app:${IMAGE_TAG:-latest}. Com um estágio test no Dockerfile, os testes rodam antes de cada compilação e teste vermelho não vira versão.";
const HOW_TARGET: &str = "Diga onde a aplicação roda: o servidor (agente), o nome do projeto compose (o mesmo de docker compose ls, para preservar containers e volumes) e a branch que ele acompanha. Escolha também aprovação, janela de manutenção e a política de vulnerabilidades. A checagem de prontidão aponta o que falta no servidor.";
const HOW_ENV: &str = "Cole o .env de produção do servidor na tela do alvo (botão .env). Ele fica cifrado na central, ganha versões e é gravado no servidor a cada deploy, legível só pelo dono. Pule se o compose não usa .env.";
const HOW_FIRST_DEPLOY: &str = "Dê push na branch do alvo — ou use Compilar agora. O servidor roda os testes, compila, guarda a imagem no registry da central, confere as vulnerabilidades e sobe o compose esperando os healthchecks; se não ficar saudável, volta a versão anterior. Tudo aparece ao vivo no console.";

/// O guia, etapa por etapa. Uma etapa só é a atual: a primeira obrigatória
/// que falta. O `.env` é recomendado, mas não segura as outras.
#[must_use]
pub fn steps(facts: &Facts) -> DeployGuideView {
    let drafts = vec![
        github_step(facts),
        agent_step(facts),
        application_step(facts),
        target_step(facts),
        env_step(facts),
        first_deploy_step(facts),
    ];
    let mut current_taken = false;
    let mut steps = Vec::with_capacity(drafts.len());
    let mut done = true;
    for draft in drafts {
        let status = if draft.done {
            DeployGuideStatus::Done
        } else if draft.optional {
            DeployGuideStatus::Optional
        } else if current_taken {
            done = false;
            DeployGuideStatus::Todo
        } else {
            current_taken = true;
            done = false;
            DeployGuideStatus::Current
        };
        steps.push(DeployGuideStep {
            key: draft.key,
            status,
            title: draft.title.to_string(),
            how: draft.how.to_string(),
            detail: draft.detail,
            action: draft.action,
        });
    }
    DeployGuideView { steps, done }
}

/// Uma etapa antes de decidir qual é a atual.
struct Draft {
    key: DeployGuideStepKey,
    title: &'static str,
    how: &'static str,
    done: bool,
    optional: bool,
    detail: String,
    action: Option<DeployGuideAction>,
}

impl Draft {
    const fn new(key: DeployGuideStepKey, title: &'static str, how: &'static str) -> Self {
        Self {
            key,
            title,
            how,
            done: false,
            optional: false,
            detail: String::new(),
            action: None,
        }
    }
}

fn github_step(facts: &Facts) -> Draft {
    let mut draft = Draft::new(DeployGuideStepKey::Github, "Conectar o GitHub", HOW_GITHUB);
    match &facts.github {
        Some(github) if github.installed => {
            draft.done = true;
            draft.detail = format!("Conectado como {}.", github.account);
        }
        Some(github) => {
            draft.detail = format!(
                "O App da central existe em {}, mas não está instalado em nenhum repositório: instale nos que vão ter deploy.",
                github.account
            );
            draft.action = Some(DeployGuideAction::InstallGithub {
                url: github.install_url.clone(),
            });
        }
        None => {
            draft.detail = "Nenhuma conta do GitHub conectada ainda.".to_string();
            draft.action = Some(DeployGuideAction::ConnectGithub);
        }
    }
    draft
}

fn agent_step(facts: &Facts) -> Draft {
    let mut draft = Draft::new(
        DeployGuideStepKey::Agent,
        "Ter um servidor com o agente",
        HOW_AGENT,
    );
    draft.action = Some(DeployGuideAction::OpenAgents);
    let needs_build = facts.applications.iter().any(|app| app.compiles);
    let ready = facts
        .agents
        .iter()
        .find(|agent| agent.connected && agent.allows_deploy);
    if let Some(agent) = ready {
        draft.done = true;
        draft.action = None;
        draft.detail = if agent.allows_build {
            format!("{} conectado: libera deploy e compilação.", agent.name)
        } else if needs_build {
            format!(
                "{} conectado e libera deploy, mas não compila: acrescente build ao AGENT_ALLOW dele.",
                agent.name
            )
        } else {
            format!("{} conectado: libera deploy.", agent.name)
        };
        return draft;
    }
    draft.detail = match facts.agents.iter().find(|agent| agent.connected) {
        Some(agent) => format!(
            "{} está conectado, mas não libera deploy: acrescente deploy,build ao AGENT_ALLOW no servidor e reinicie o agente.",
            agent.name
        ),
        None if facts.agents.is_empty() => "Nenhum servidor com o agente ainda.".to_string(),
        None => "Os agentes cadastrados estão desconectados: confira se o serviço está rodando no servidor.".to_string(),
    };
    draft
}

fn application_step(facts: &Facts) -> Draft {
    let mut draft = Draft::new(
        DeployGuideStepKey::Application,
        "Cadastrar a aplicação",
        HOW_APPLICATION,
    );
    if facts.applications.is_empty() {
        draft.detail = "Nenhuma aplicação cadastrada.".to_string();
        draft.action = Some(DeployGuideAction::Setup);
    } else {
        draft.done = true;
        let names: Vec<&str> = facts
            .applications
            .iter()
            .map(|app| app.name.as_str())
            .collect();
        draft.detail = format!("Cadastrada: {}.", names.join(", "));
    }
    draft
}

fn target_step(facts: &Facts) -> Draft {
    let mut draft = Draft::new(
        DeployGuideStepKey::Target,
        "Dizer onde ela roda",
        HOW_TARGET,
    );
    let target = facts
        .applications
        .iter()
        .flat_map(|app| app.targets.iter())
        .next();
    match target {
        Some(target) => {
            draft.done = true;
            draft.detail = format!("{} acompanha a branch {}.", target.name, target.reference);
        }
        None => {
            draft.detail = "A aplicação ainda não tem servidor (alvo).".to_string();
            draft.action = facts
                .applications
                .first()
                .map(|app| DeployGuideAction::AddTarget {
                    application_id: app.id,
                });
        }
    }
    draft
}

fn env_step(facts: &Facts) -> Draft {
    let mut draft = Draft::new(
        DeployGuideStepKey::Env,
        "Gravar o .env do servidor",
        HOW_ENV,
    );
    draft.optional = true;
    let targets: Vec<&TargetFact> = facts
        .applications
        .iter()
        .flat_map(|app| app.targets.iter())
        .collect();
    match targets.iter().find(|target| !target.has_env) {
        Some(missing) => {
            draft.detail = format!("{} ainda não tem .env gravado.", missing.name);
            draft.action = Some(DeployGuideAction::EditEnv {
                target_id: missing.id,
            });
        }
        None if targets.is_empty() => {
            draft.detail = "Vem depois do alvo.".to_string();
        }
        None => {
            draft.done = true;
            draft.detail = "Todos os alvos têm .env gravado.".to_string();
        }
    }
    draft
}

fn first_deploy_step(facts: &Facts) -> Draft {
    let mut draft = Draft::new(
        DeployGuideStepKey::FirstDeploy,
        "Primeira versão no ar",
        HOW_FIRST_DEPLOY,
    );
    let live = facts
        .applications
        .iter()
        .flat_map(|app| app.targets.iter())
        .find_map(|target| target.live.as_ref().map(|sha| (target, sha)));
    if let Some((target, sha)) = live {
        draft.done = true;
        draft.detail = format!(
            "{} no ar com o commit {sha}. Daqui em diante, cada push na branch publica sozinho.",
            target.name
        );
        return draft;
    }
    let compiles = facts.applications.first().filter(|app| app.compiles);
    if let Some(run) = &facts.latest_run {
        match run.status {
            RunStatus::Running | RunStatus::Queued => {
                draft.detail = format!(
                    "Publicando {} em {} agora — acompanhe pelo console.",
                    run.short_sha, run.target
                );
                draft.action = Some(DeployGuideAction::OpenRun { run_id: run.id });
                return draft;
            }
            RunStatus::AwaitingApproval => {
                draft.detail = format!(
                    "A versão {} espera a sua aprovação para ir a {}.",
                    run.short_sha, run.target
                );
                draft.action = Some(DeployGuideAction::OpenRun { run_id: run.id });
                return draft;
            }
            RunStatus::Failed | RunStatus::RolledBack => {
                draft.detail = format!(
                    "O deploy de {} em {} falhou: abra-o para ver o console — a IA explica o erro.",
                    run.short_sha, run.target
                );
                draft.action = Some(DeployGuideAction::OpenRun { run_id: run.id });
                return draft;
            }
            _ => {}
        }
    }
    if let Some(build) = &facts.latest_build {
        match build.status {
            BuildStatus::Queued | BuildStatus::Running => {
                draft.detail = format!(
                    "Compilando {} — os testes e a compilação aparecem ao vivo no console.",
                    build.short_sha
                );
                return draft;
            }
            BuildStatus::Failed => {
                draft.detail = format!(
                    "A compilação de {} falhou: abra-a em Compilações recentes para ver o console.",
                    build.short_sha
                );
            }
            _ => {}
        }
    }
    if draft.detail.is_empty() {
        draft.detail = match facts
            .applications
            .iter()
            .flat_map(|app| app.targets.iter())
            .next()
        {
            Some(target) => format!("Dê push na branch {} para publicar.", target.reference),
            None => "Vem depois do alvo.".to_string(),
        };
    }
    draft.action = compiles.map(|app| DeployGuideAction::BuildNow {
        application_id: app.id,
    });
    draft
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(connected: bool, deploy: bool, build: bool) -> AgentFact {
        AgentFact {
            name: "srv-01".into(),
            connected,
            allows_deploy: deploy,
            allows_build: build,
        }
    }

    fn app(targets: Vec<TargetFact>) -> ApplicationFact {
        ApplicationFact {
            id: 7,
            name: "Loja".into(),
            compiles: true,
            targets,
        }
    }

    fn target(env: bool, live: Option<&str>) -> TargetFact {
        TargetFact {
            id: 3,
            name: "Produção".into(),
            reference: "main".into(),
            has_env: env,
            live: live.map(String::from),
        }
    }

    fn github() -> Option<GithubFact> {
        Some(GithubFact {
            account: "acme".into(),
            installed: true,
            install_url: "https://github.com/apps/x/installations/new".into(),
        })
    }

    fn status(view: &DeployGuideView, key: DeployGuideStepKey) -> DeployGuideStatus {
        view.steps
            .iter()
            .find(|step| step.key == key)
            .unwrap()
            .status
    }

    #[test]
    fn do_zero_a_atual_e_conectar_o_github() {
        let view = steps(&Facts::default());
        assert_eq!(
            status(&view, DeployGuideStepKey::Github),
            DeployGuideStatus::Current
        );
        assert_eq!(
            status(&view, DeployGuideStepKey::Agent),
            DeployGuideStatus::Todo
        );
        assert_eq!(view.steps[0].action, Some(DeployGuideAction::ConnectGithub));
        assert!(!view.done);
        // Só uma etapa é a atual.
        let current = view
            .steps
            .iter()
            .filter(|step| step.status == DeployGuideStatus::Current)
            .count();
        assert_eq!(current, 1);
    }

    #[test]
    fn app_sem_instalacao_pede_para_instalar() {
        let facts = Facts {
            github: Some(GithubFact {
                installed: false,
                ..github().unwrap()
            }),
            ..Facts::default()
        };
        let view = steps(&facts);
        assert_eq!(
            status(&view, DeployGuideStepKey::Github),
            DeployGuideStatus::Current
        );
        assert!(matches!(
            view.steps[0].action,
            Some(DeployGuideAction::InstallGithub { .. })
        ));
    }

    #[test]
    fn agente_sem_permissao_diz_o_que_acrescentar() {
        let facts = Facts {
            github: github(),
            agents: vec![agent(true, false, false)],
            ..Facts::default()
        };
        let view = steps(&facts);
        assert_eq!(
            status(&view, DeployGuideStepKey::Agent),
            DeployGuideStatus::Current
        );
        assert!(view.steps[1].detail.contains("AGENT_ALLOW"));
    }

    #[test]
    fn env_e_recomendado_mas_nao_segura_o_primeiro_deploy() {
        let facts = Facts {
            github: github(),
            agents: vec![agent(true, true, true)],
            applications: vec![app(vec![target(false, None)])],
            ..Facts::default()
        };
        let view = steps(&facts);
        assert_eq!(
            status(&view, DeployGuideStepKey::Env),
            DeployGuideStatus::Optional
        );
        assert_eq!(
            status(&view, DeployGuideStepKey::FirstDeploy),
            DeployGuideStatus::Current
        );
        assert_eq!(
            view.steps[5].action,
            Some(DeployGuideAction::BuildNow { application_id: 7 })
        );
        assert!(view.steps[5].detail.contains("main"));
    }

    #[test]
    fn deploy_falhou_aponta_o_console() {
        let facts = Facts {
            github: github(),
            agents: vec![agent(true, true, true)],
            applications: vec![app(vec![target(true, None)])],
            latest_run: Some(RunFact {
                id: 42,
                status: RunStatus::Failed,
                target: "Produção".into(),
                short_sha: "abc1234".into(),
            }),
            ..Facts::default()
        };
        let view = steps(&facts);
        assert_eq!(
            view.steps[5].action,
            Some(DeployGuideAction::OpenRun { run_id: 42 })
        );
        assert!(view.steps[5].detail.contains("falhou"));
    }

    #[test]
    fn versao_no_ar_conclui_o_guia() {
        let facts = Facts {
            github: github(),
            agents: vec![agent(true, true, true)],
            applications: vec![app(vec![target(true, Some("abc1234"))])],
            ..Facts::default()
        };
        let view = steps(&facts);
        assert!(view.done);
        assert!(view
            .steps
            .iter()
            .all(|step| step.status == DeployGuideStatus::Done));
    }
}
