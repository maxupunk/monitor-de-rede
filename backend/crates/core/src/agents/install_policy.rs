//! O que o comando de instalação libera no host: `AGENT_ALLOW` e
//! `AGENT_DATABASE_TARGETS`.
//!
//! A política continua **do host** (ADR 011): a central só escreve a
//! sugestão no comando que o operador cola lá, e ele vê e escolhe o que
//! vai — a tela marca tudo por padrão. Depois de instalado, só o
//! `agent.env` do host muda a política; a central não a amplia.

use agent_protocol::policy::{Permission, Policy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::database_targets::{format_targets, parse_targets, TunnelTarget};
use crate::{
    models::probes,
    shared::errors::{AppError, AppResult},
};

/// Faixas em que o Docker cria as redes (a padrão e as do compose), nas
/// portas do PostgreSQL e do MySQL/MariaDB: os bancos em contêiner deste host.
const DOCKER_DATABASE_TARGETS: &str =
    "172.16.0.0/12:5432,172.16.0.0/12:3306,192.168.0.0/16:5432,192.168.0.0/16:3306";

/// A ponte de banco liberada sem nenhum destino.
pub const MISSING_DATABASE_TARGETS: &str = "A ponte de banco precisa de ao menos um destino      permitido (IP:porta ou faixa:porta, ex.: 10.0.0.20:5432). Informe os destinos ou desmarque      a permissão.";

/// Chave em `probes.configuration` com a política escolhida na instalação.
const CONFIG_KEY: &str = "installPolicy";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPolicy {
    policy: Policy,
    database_targets: Vec<TunnelTarget>,
}

/// A política como a tela a vê e a devolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct InstallPolicyView {
    pub allow: Vec<Permission>,
    /// `AGENT_DATABASE_TARGETS`: `IP:porta` ou `faixa:porta`, por vírgula.
    pub database_targets: String,
}

impl InstallPolicy {
    /// A sugestão da tela: tudo liberado e os bancos em contêiner deste host.
    #[must_use]
    pub fn suggested() -> Self {
        Self {
            policy: Policy::from_permissions(Permission::ALL),
            database_targets: parse_targets(DOCKER_DATABASE_TARGETS).unwrap_or_default(),
        }
    }

    /// # Errors
    ///
    /// `validation` para um destino inválido — ele vai para um comando de
    /// shell e para a política do host — e para a ponte de banco sem destino:
    /// a lista é obrigatória no agente (ADR 013) e, vazia, a permissão não
    /// alcança banco nenhum.
    pub fn new(allow: &[Permission], database_targets: &str) -> AppResult<Self> {
        let database_targets = parse_targets(database_targets).map_err(AppError::validation)?;
        if allow.contains(&Permission::Database) && database_targets.is_empty() {
            return Err(AppError::validation(MISSING_DATABASE_TARGETS));
        }
        Ok(Self {
            policy: Policy::from_permissions(allow.iter().copied()),
            database_targets,
        })
    }

    /// # Errors
    ///
    /// Como [`Self::new`].
    pub fn from_view(view: &InstallPolicyView) -> AppResult<Self> {
        Self::new(&view.allow, &view.database_targets)
    }

    /// Escolha da tela, ou a sugestão quando ela não veio.
    ///
    /// # Errors
    ///
    /// Como [`Self::new`].
    pub fn chosen_or_suggested(view: Option<&InstallPolicyView>) -> AppResult<Self> {
        view.map_or_else(|| Ok(Self::suggested()), Self::from_view)
    }

    /// A política gravada no cadastro do agente; sem ela, a sugestão.
    #[must_use]
    pub fn of(probe: &probes::Model) -> Self {
        probe
            .configuration
            .as_ref()
            .and_then(|value| value.get(CONFIG_KEY))
            .and_then(|value| <InstallPolicyView as serde::Deserialize>::deserialize(value).ok())
            .and_then(|view| Self::from_view(&view).ok())
            .unwrap_or_else(Self::suggested)
    }

    /// Grava a política na configuração do agente, para o script regerado
    /// (rotação de chaves) sair com a mesma escolha.
    pub fn store(&self, configuration: &mut Value) {
        if let Ok(view) = serde_json::to_value(self.view()) {
            configuration[CONFIG_KEY] = view;
        }
    }

    #[must_use]
    pub fn view(&self) -> InstallPolicyView {
        InstallPolicyView {
            allow: self.policy.permissions(),
            database_targets: self.database_targets_env(),
        }
    }

    /// Valor de `AGENT_ALLOW`.
    #[must_use]
    pub fn allow_env(&self) -> String {
        let mut out = String::new();
        for (idx, permission) in self.policy.permissions().into_iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            out.push_str(permission.as_str());
        }
        out
    }

    /// Valor de `AGENT_DATABASE_TARGETS`; vazio quando a ponte não está
    /// liberada — a lista sem a permissão não serve para nada.
    #[must_use]
    pub fn database_targets_env(&self) -> String {
        if self.policy.allows(Permission::Database) {
            format_targets(&self.database_targets)
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sugestao_libera_tudo_e_os_bancos_do_docker() {
        let suggested = InstallPolicy::suggested();
        assert_eq!(suggested.view().allow.len(), Permission::ALL.len());
        assert!(suggested.allow_env().contains("database"));
        assert_eq!(suggested.database_targets_env(), DOCKER_DATABASE_TARGETS);
    }

    #[test]
    fn sem_a_ponte_nao_sai_lista_de_bancos() {
        let policy = InstallPolicy::new(&[Permission::Lifecycle], "10.0.0.5:5432").unwrap();
        assert_eq!(policy.allow_env(), "read,lifecycle");
        assert_eq!(policy.database_targets_env(), "");
    }

    #[test]
    fn destino_invalido_nao_vira_comando() {
        for raw in ["10.0.0.5; rm -rf /", "$(id):5432", "banco:5432"] {
            assert!(
                InstallPolicy::new(&[Permission::Database], raw).is_err(),
                "{raw}"
            );
        }
    }

    #[test]
    fn ponte_de_banco_sem_destino_e_recusada() {
        let err = InstallPolicy::new(&[Permission::Database], " , ").unwrap_err();
        assert_eq!(err.to_string(), MISSING_DATABASE_TARGETS);
        assert!(InstallPolicy::new(&[Permission::Lifecycle], "").is_ok());
    }

    #[test]
    fn a_escolha_gravada_volta_igual() {
        let chosen = InstallPolicy::new(&[Permission::Database], "10.0.0.5:5432").unwrap();
        let mut configuration = serde_json::json!({ "role": "agent" });
        chosen.store(&mut configuration);
        let probe = probes::Model {
            configuration: Some(configuration),
            ..test_probe()
        };
        assert_eq!(InstallPolicy::of(&probe), chosen);
        assert_eq!(InstallPolicy::of(&test_probe()), InstallPolicy::suggested());
    }

    fn test_probe() -> probes::Model {
        serde_json::from_value(serde_json::json!({
            "id": 1,
            "name": "srv",
            "token_hash": "x",
            "status": "pending",
            "created_at": "2026-01-01T00:00:00+00:00",
            "updated_at": "2026-01-01T00:00:00+00:00",
        }))
        .expect("probe")
    }
}
