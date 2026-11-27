//! Acompanhar o contêiner: o endereço da conexão segue o banco quando o
//! contêiner reinicia ou é recriado com outro IP.
//!
//! A conexão criada a partir de um contêiner descoberto ([`super::discovery`])
//! guarda um [`ContainerLink`] — host, nome e projeto/serviço do compose, o
//! que sobrevive à recriação. Antes de cada backup e restauração, [`follow`]
//! reencontra o contêiner no host e confere se o endereço salvo ainda é dele:
//!
//! * **ainda é** — segue, sem testar nada (uma `inspect`, nenhum socket);
//! * **não é mais** (o IP mudou, ou passou a ser de outro contêiner) — testa os
//!   endereços atuais na ordem recomendada (o nome na rede Docker primeiro),
//!   grava o que respondeu e segue com ele;
//! * **não deu para saber** (agente fora, contêiner sumido) — segue com o
//!   endereço salvo: o backup tenta como antes e, se falhar, a falha diz por
//!   que o vínculo não ajudou.
//!
//! Conferir que o endereço **é do contêiner**, e não só que responde, é o que
//! impede copiar o banco errado quando o IP antigo foi parar em outro.

use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    discovery::{self, AddressAttempt, CandidateAddress, DatabaseContainer, Inspected},
    jobs::Job,
    service::{self, Connection},
    DatabaseEngine, DatabaseTarget,
};
use netmonitor_core::{
    agents::HostKey,
    models::database_connections,
    shared::errors::{AppError, AppResult},
};

/// De qual contêiner a conexão veio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct ContainerLink {
    /// `agent-<id>`.
    pub host_key: String,
    /// Nome do contêiner — o id muda a cada recriação, o nome não.
    pub name: String,
    #[serde(default)]
    pub project: Option<String>,
    /// Serviço do compose: acha o contêiner quando o nome mudou.
    #[serde(default)]
    pub service: Option<String>,
}

impl ContainerLink {
    /// O vínculo gravado na linha, quando há.
    #[must_use]
    pub fn of(row: &database_connections::Model) -> Option<Self> {
        Some(Self {
            host_key: row.container_host_key.clone()?,
            name: row.container_name.clone()?,
            project: row.container_project.clone(),
            service: row.container_service.clone(),
        })
    }

    /// O vínculo de um contêiner descoberto.
    #[must_use]
    pub fn from_container(container: &DatabaseContainer) -> Self {
        Self {
            host_key: container.host_key.clone(),
            name: container.container_name.clone(),
            project: container.project.clone(),
            service: container.service.clone(),
        }
    }

    /// É este o contêiner do vínculo? Mesmo host e o mesmo nome, ou o mesmo
    /// serviço do mesmo projeto do compose.
    #[must_use]
    pub fn matches(&self, container: &DatabaseContainer) -> bool {
        if self.host_key != container.host_key {
            return false;
        }
        if self.name.eq_ignore_ascii_case(&container.container_name) {
            return true;
        }
        self.service.is_some()
            && self.service == container.service
            && self.project == container.project
    }

    /// Aparado e conferido, para gravar.
    ///
    /// # Errors
    ///
    /// Host que não é `agent-<id>`, nome vazio ou longo demais.
    pub fn normalized(self) -> AppResult<Self> {
        let host_key = self.host_key.trim().to_string();
        host_key.parse::<HostKey>()?;
        let name = self.name.trim().trim_start_matches('/').to_string();
        if name.is_empty() || name.len() > 255 || name.chars().any(char::is_control) {
            return Err(AppError::validation("Nome de contêiner inválido"));
        }
        let optional = |value: Option<String>| {
            value
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty() && value.len() <= 255)
        };
        Ok(Self {
            host_key,
            name,
            project: optional(self.project),
            service: optional(self.service),
        })
    }

    /// Escreve o vínculo (ou a falta dele) na linha.
    pub fn apply(link: Option<&Self>, row: &mut database_connections::ActiveModel) {
        row.container_host_key = Set(link.map(|l| l.host_key.clone()));
        row.container_name = Set(link.map(|l| l.name.clone()));
        row.container_project = Set(link.and_then(|l| l.project.clone()));
        row.container_service = Set(link.and_then(|l| l.service.clone()));
    }
}

/// O endereço salvo ainda é um dos endereços do contêiner?
#[must_use]
pub fn still_points_to(host: &str, port: i32, addresses: &[CandidateAddress]) -> bool {
    let host = host.trim();
    addresses
        .iter()
        .any(|address| address.host.eq_ignore_ascii_case(host) && i32::from(address.port) == port)
}

/// Por que nenhum endereço serviu, para a tela e o histórico.
fn none_reachable(name: &str, attempts: &[AddressAttempt]) -> AppError {
    let tried = attempts
        .iter()
        .map(|attempt| {
            format!(
                "{}:{} ({})",
                attempt.address.host,
                attempt.address.port,
                attempt.message.as_deref().unwrap_or("sem resposta")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    AppError::business_rule(if tried.is_empty() {
        format!("O contêiner {name} não tem endereço pelo qual a central chegue ao banco")
    } else {
        format!("Nenhum endereço do contêiner {name} respondeu: {tried}")
    })
}

/// O que [`follow`] fez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Followed {
    /// A conexão não acompanha contêiner.
    NotLinked,
    /// O endereço salvo ainda é do contêiner.
    Unchanged,
    /// O contêiner mudou de endereço, e a conexão foi atualizada.
    Moved { from: String, to: String },
    /// Não deu para conferir; segue com o endereço salvo.
    Unavailable(String),
}

impl Followed {
    /// A frase que vai para o resumo do backup, quando há o que contar.
    #[must_use]
    pub fn note(&self) -> Option<String> {
        match self {
            Self::Moved { from, to } => Some(format!(
                "o contêiner mudou de endereço ({from} → {to}) e a conexão foi atualizada"
            )),
            Self::Unavailable(reason) => Some(format!(
                "não foi possível conferir o endereço do contêiner: {reason}"
            )),
            Self::NotLinked | Self::Unchanged => None,
        }
    }
}

/// Onde o contêiner está agora, para quem já tem a linha.
struct Located {
    inspected: Inspected,
    /// O endereço novo, quando o salvo não é mais do contêiner.
    moved_to: Option<CandidateAddress>,
}

async fn locate(
    ctx: &AppContext,
    row: &database_connections::Model,
    link: &ContainerLink,
) -> AppResult<Located> {
    let inspected = discovery::locate(ctx, link).await?;
    if still_points_to(&row.host, row.port, &inspected.addresses) {
        return Ok(Located {
            inspected,
            moved_to: None,
        });
    }
    let (chosen, attempts) =
        discovery::reachable(ctx, row.via_probe_id, &inspected.addresses).await;
    let address =
        chosen.ok_or_else(|| none_reachable(&inspected.container.container_name, &attempts))?;
    Ok(Located {
        inspected,
        moved_to: Some(address),
    })
}

/// Grava o endereço novo e o nome atual do contêiner (o compose pode tê-lo
/// recriado com outro), quando algo mudou.
async fn persist<C: ConnectionTrait>(
    db: &C,
    row: &database_connections::Model,
    located: &Located,
) -> AppResult<database_connections::Model> {
    let name = &located.inspected.container.container_name;
    let renamed = row.container_name.as_deref() != Some(name.as_str());
    if located.moved_to.is_none() && !renamed {
        return Ok(row.clone());
    }
    let mut active: database_connections::ActiveModel = row.clone().into();
    if let Some(address) = &located.moved_to {
        active.host = Set(address.host.clone());
        active.port = Set(i32::from(address.port));
    }
    active.container_name = Set(Some(name.clone()));
    active.updated_at = Set(Utc::now().into());
    Ok(active.update(db).await?)
}

fn moved(row: &database_connections::Model, located: &Located) -> Followed {
    located
        .moved_to
        .as_ref()
        .map_or(Followed::Unchanged, |to| Followed::Moved {
            from: format!("{}:{}", row.host, row.port),
            to: format!("{}:{}", to.host, to.port),
        })
}

/// Antes de um backup ou restauração: o alvo com o endereço atual do
/// contêiner. Nunca impede a operação — sem poder conferir, devolve o alvo
/// salvo e diz por quê.
pub async fn follow(
    ctx: &AppContext,
    connection: &Connection,
    job: &Job,
) -> (DatabaseTarget, Followed) {
    let mut target = connection.target();
    let Some(link) = ContainerLink::of(&connection.row) else {
        return (target, Followed::NotLinked);
    };
    job.milestone(&format!("Conferindo o endereço do contêiner {}", link.name));
    let located = match locate(ctx, &connection.row, &link).await {
        Ok(located) => located,
        Err(error) => {
            tracing::warn!(%error, conexao = connection.row.id, "vínculo com o contêiner não conferido");
            return (target, Followed::Unavailable(error.to_string()));
        }
    };
    let outcome = moved(&connection.row, &located);
    match persist(&ctx.db, &connection.row, &located).await {
        Ok(row) => {
            target.host = row.host;
            target.port = u16::try_from(row.port).unwrap_or(target.port);
        }
        Err(error) => {
            tracing::warn!(%error, "não foi possível gravar o endereço novo do contêiner");
            if let Some(address) = &located.moved_to {
                target.host.clone_from(&address.host);
                target.port = address.port;
            }
        }
    }
    if let Followed::Moved { from, to } = &outcome {
        tracing::info!(conexao = connection.row.id, %from, %to, "contêiner do banco mudou de endereço");
        job.milestone(&format!("O contêiner mudou de endereço: {from} → {to}"));
    }
    (target, outcome)
}

/// "Reencontrar endereço agora": o mesmo que o backup faz, mas a falha volta
/// para a tela.
///
/// # Errors
///
/// `404`; conexão sem vínculo; host desconectado; contêiner sumido; nenhum
/// endereço respondeu.
pub async fn relocate(
    ctx: &AppContext,
    id: i64,
) -> AppResult<(database_connections::Model, Followed)> {
    let row = service::find(&ctx.db, id).await?;
    let link = ContainerLink::of(&row)
        .ok_or_else(|| AppError::validation("Esta conexão não está vinculada a um contêiner"))?;
    let located = locate(ctx, &row, &link).await?;
    let outcome = moved(&row, &located);
    Ok((persist(&ctx.db, &row, &located).await?, outcome))
}

/// Vincula uma conexão existente a um contêiner: grava o vínculo, a rota e o
/// endereço que respondeu.
///
/// # Errors
///
/// `404`; SGBD diferente; host desconectado; nenhum endereço respondeu.
pub async fn link(
    ctx: &AppContext,
    id: i64,
    host_key: &str,
    container_id: &str,
) -> AppResult<database_connections::Model> {
    let row = service::find(&ctx.db, id).await?;
    let inspected = discovery::inspect(ctx, host_key, container_id).await?;
    let container = &inspected.container;
    let engine = DatabaseEngine::parse(&row.engine)?;
    if engine.family() != container.engine.family() {
        return Err(AppError::validation(format!(
            "O contêiner {} é {} e a conexão é {}",
            container.container_name,
            container.engine.label(),
            engine.label()
        )));
    }
    let (chosen, attempts) =
        discovery::reachable(ctx, container.via_probe_id, &inspected.addresses).await;
    let address = chosen.ok_or_else(|| none_reachable(&container.container_name, &attempts))?;
    let mut active: database_connections::ActiveModel = row.into();
    active.host = Set(address.host);
    active.port = Set(i32::from(address.port));
    active.via_probe_id = Set(container.via_probe_id);
    ContainerLink::apply(Some(&ContainerLink::from_container(container)), &mut active);
    active.updated_at = Set(Utc::now().into());
    Ok(active.update(&ctx.db).await?)
}

/// Desfaz o vínculo: o endereço salvo fica fixo, como digitado.
///
/// # Errors
///
/// `404` ou erro do banco.
pub async fn unlink<C: ConnectionTrait>(db: &C, id: i64) -> AppResult<database_connections::Model> {
    let row = service::find(db, id).await?;
    let mut active: database_connections::ActiveModel = row.into();
    ContainerLink::apply(None, &mut active);
    active.updated_at = Set(Utc::now().into());
    Ok(active.update(db).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::databases::discovery::AddressKind;

    fn address(kind: AddressKind, host: &str, port: u16) -> CandidateAddress {
        CandidateAddress {
            kind,
            host: host.into(),
            port,
            network: None,
        }
    }

    fn container(host_key: &str, name: &str, service: Option<&str>) -> DatabaseContainer {
        DatabaseContainer {
            host_key: host_key.into(),
            host_name: "central".into(),
            local: true,
            via_probe_id: None,
            route_allowed: true,
            container_id: "a1b2c3d4e5f6a7b8".into(),
            container_name: name.into(),
            project: Some("erp".into()),
            service: service.map(str::to_string),
            image: "postgres:16".into(),
            engine: DatabaseEngine::Postgres,
            running: true,
            status: "running".into(),
            port: 5432,
            protected_connection_id: None,
            protected_connection_name: None,
        }
    }

    #[test]
    fn o_ip_antigo_que_nao_e_mais_do_conteiner_pede_reencontrar() {
        let addresses = [
            address(AddressKind::Dns, "erp-db-1", 5432),
            address(AddressKind::ContainerIp, "172.18.0.7", 5432),
        ];
        assert!(still_points_to("172.18.0.7", 5432, &addresses));
        assert!(still_points_to(" ERP-DB-1 ", 5432, &addresses));
        // O contêiner voltou com outro IP: o salvo não é mais dele.
        assert!(!still_points_to("172.18.0.5", 5432, &addresses));
        // Mesmo host, outra porta: não é o mesmo endereço.
        assert!(!still_points_to("172.18.0.7", 5433, &addresses));
    }

    #[test]
    fn o_vinculo_acha_o_conteiner_pelo_nome_ou_pelo_servico_do_compose() {
        let link = ContainerLink {
            host_key: "agent-1".into(),
            name: "erp-db-1".into(),
            project: Some("erp".into()),
            service: Some("db".into()),
        };
        assert!(link.matches(&container("agent-1", "erp-db-1", None)));
        // Recriado com outro nome, mesmo serviço do mesmo projeto.
        assert!(link.matches(&container("agent-1", "erp_db_novo", Some("db"))));
        // Outro host nunca é o mesmo contêiner.
        assert!(!link.matches(&container("agent-2", "erp-db-1", Some("db"))));
        assert!(!link.matches(&container("agent-1", "outro", Some("cache"))));
    }

    #[test]
    fn vinculo_normalizado_recusa_host_invalido_e_apara_o_nome() {
        let link = ContainerLink {
            host_key: " agent-3 ".into(),
            name: "/erp-db-1 ".into(),
            project: Some("  ".into()),
            service: Some("db".into()),
        }
        .normalized()
        .unwrap();
        assert_eq!(link.host_key, "agent-3");
        assert_eq!(link.name, "erp-db-1");
        assert_eq!(link.project, None);
        assert_eq!(link.service.as_deref(), Some("db"));

        let bad = ContainerLink {
            host_key: "docker-local".into(),
            name: "x".into(),
            project: None,
            service: None,
        };
        assert!(bad.normalized().is_err());
    }

    #[test]
    fn so_o_que_mudou_vira_nota_do_backup() {
        assert_eq!(Followed::Unchanged.note(), None);
        assert_eq!(Followed::NotLinked.note(), None);
        let note = Followed::Moved {
            from: "172.18.0.5:5432".into(),
            to: "erp-db-1:5432".into(),
        }
        .note()
        .unwrap();
        assert!(note.contains("172.18.0.5:5432 → erp-db-1:5432"));
    }

    #[test]
    fn sem_endereco_que_responda_a_falha_lista_as_tentativas() {
        let attempts = [AddressAttempt {
            address: address(AddressKind::ContainerIp, "172.18.0.7", 5432),
            reachable: false,
            message: Some("A porta recusou a conexão".into()),
        }];
        let message = none_reachable("erp-db-1", &attempts).to_string();
        assert!(message.contains("172.18.0.7:5432 (A porta recusou a conexão)"));
        assert!(none_reachable("erp-db-1", &[])
            .to_string()
            .contains("não tem endereço"));
    }
}
