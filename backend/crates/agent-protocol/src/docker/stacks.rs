//! Deploy contínuo de stacks compose (ADR 021): o que a central pede ao
//! agente e o que ele devolve.
//!
//! Um deploy é **uma** operação fechada (`DockerCall::StackDeploy`), nunca um
//! shell: o agente sincroniza o repositório no commit pedido, grava o `.env`,
//! baixa as imagens já publicadas e sobe o compose esperando os healthchecks.
//! Tudo acontece dentro de `AGENT_STACKS_DIR` — o único diretório do host que
//! o agente escreve —, e a validação abaixo roda **dos dois lados** do fio:
//! a central recusa antes de enviar, o agente recusa antes de executar.
//!
//! Segredos (token do Git, senha do registry, o `.env`) viajam como
//! [`Secret`], cujo `Debug` não mostra o valor: o comando aparece em log de
//! erro e em `tracing`.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::DockerError;

/// Texto sensível no fio. Serializa como string comum; só o `Debug` esconde.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<oculto>")
    }
}

/// De onde vêm os arquivos de orquestração: o repositório, no commit exato
/// da imagem — o compose e os arquivos montados andam junto com o código.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSource {
    /// `https://github.com/dono/repo.git`. Só HTTPS: o token vai num
    /// cabeçalho, nunca na URL.
    pub repository_url: String,
    /// SHA completo (ou abreviado, 7+) do commit.
    pub commit: String,
    /// Token de leitura para repositório privado.
    pub token: Option<Secret>,
}

/// Login de uso único no registry, só para o `pull` deste deploy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryAuth {
    /// `ghcr.io`.
    pub registry: String,
    pub username: String,
    pub secret: Secret,
}

/// O pedido de deploy de uma stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackDeployRequest {
    /// Nome do projeto compose; também o diretório em `AGENT_STACKS_DIR`.
    pub project: String,
    pub source: GitSource,
    /// Arquivos compose, relativos à raiz do repositório.
    pub compose_files: Vec<String>,
    /// Conteúdo do `.env`. `None` mantém o que estiver no diretório.
    pub env_file: Option<Secret>,
    /// Variável que o compose usa na tag das imagens (`IMAGE_TAG`).
    pub image_tag_variable: String,
    /// A tag publicada pelo CI — o SHA do commit, imutável.
    pub image_tag: String,
    pub registry: Option<RegistryAuth>,
    /// Teto do `up --wait`: quanto esperar os healthchecks.
    pub wait_timeout_secs: u32,
    /// Quantas tags antigas de cada imagem manter em disco para o rollback.
    pub keep_images: u32,
    /// Baixar as imagens antes de subir. Falso quando a central já as
    /// entregou pelo canal (`Command::ImageImport`): o `up` não tenta o
    /// registry (`--pull never`).
    #[serde(default = "pull_by_default")]
    pub pull: bool,
}

/// Teto do `--wait`: além disso, o deploy segura a fila sem motivo.
const fn pull_by_default() -> bool {
    true
}

pub const MAX_WAIT_TIMEOUT_SECS: u32 = 1800;
/// Mais que isso só enche o disco.
pub const MAX_KEEP_IMAGES: u32 = 20;

impl StackDeployRequest {
    /// Confere cada campo antes de qualquer efeito.
    ///
    /// # Errors
    ///
    /// `Validation` com o motivo, em português, para a tela.
    pub fn validate(&self) -> Result<(), DockerError> {
        validate_project(&self.project)?;
        validate_repository_url(&self.source.repository_url)?;
        validate_commit(&self.source.commit)?;
        if self.compose_files.is_empty() {
            return Err(invalid("Informe ao menos um arquivo compose"));
        }
        for file in &self.compose_files {
            validate_relative_path(file)?;
        }
        validate_variable(&self.image_tag_variable)?;
        validate_image_tag(&self.image_tag)?;
        if let Some(registry) = &self.registry {
            validate_registry(&registry.registry)?;
            if registry.username.trim().is_empty() {
                return Err(invalid("Informe o usuário do registry"));
            }
        }
        if self.wait_timeout_secs == 0 || self.wait_timeout_secs > MAX_WAIT_TIMEOUT_SECS {
            return Err(invalid(format!(
                "O tempo de espera dos healthchecks vai de 1 a {MAX_WAIT_TIMEOUT_SECS} segundos"
            )));
        }
        if self.keep_images > MAX_KEEP_IMAGES {
            return Err(invalid(format!(
                "Mantenha no máximo {MAX_KEEP_IMAGES} versões antigas de cada imagem"
            )));
        }
        Ok(())
    }
}

/// Etapa em curso, para a barra de progresso da tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum StackStage {
    /// Repositório no commit pedido.
    Sync,
    /// `.env` gravado.
    Env,
    /// Imagens baixadas.
    Pull,
    /// Containers recriados, esperando os healthchecks.
    Start,
    /// Tags antigas removidas.
    Prune,
}

/// O que o agente manda em cada `Chunk` de um deploy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StackChunk {
    Stage { stage: StackStage },
    Line { text: String },
}

/// Um serviço da stack depois do deploy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StackServiceState {
    pub name: String,
    pub image: String,
    /// `running`, `exited`…
    pub state: String,
    /// `healthy`, `unhealthy`, `starting`; `None` sem healthcheck.
    pub health: Option<String>,
}

/// Resposta de um deploy bem-sucedido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackDeployOutcome {
    /// O commit que ficou no diretório (SHA completo).
    pub commit: String,
    pub services: Vec<StackServiceState>,
    pub removed_images: u32,
}

/// O que o agente sabe dizer antes do primeiro deploy (a checagem de
/// prontidão da tela). Nada aqui muda o host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct StackReadiness {
    /// A raiz das stacks, quando montada e gravável.
    pub stacks_dir: Option<String>,
    /// Por que a raiz não serve (não montada, sem escrita).
    pub stacks_dir_issue: Option<String>,
    /// Versão do `git` no agente.
    pub git_version: Option<String>,
    /// Versão do plugin compose.
    pub compose_version: Option<String>,
    /// Espaço livre na raiz das stacks.
    #[ts(type = "number | null")]
    pub free_bytes: Option<u64>,
    /// O diretório da stack já existe (deploys anteriores).
    pub project_dir_exists: bool,
    /// O projeto já roda neste host — de onde (label `working_dir`).
    pub running_working_dir: Option<String>,
    /// Serviços em execução sem healthcheck: o `--wait` só confere que
    /// estão de pé.
    pub services_without_healthcheck: Vec<String>,
    /// Projeto da instalação NetMonitor: o agente recusa o deploy.
    pub protected: bool,
}

fn invalid(message: impl Into<String>) -> DockerError {
    DockerError::Validation(message.into())
}

/// Nome de projeto compose: minúsculas, dígitos, `-` e `_`, começando por
/// letra ou dígito. É também nome de diretório — por isso sem `.` nem `/`.
///
/// # Errors
///
/// `Validation` fora do formato.
pub fn validate_project(name: &str) -> Result<(), DockerError> {
    let valid = !name.is_empty()
        && name.len() <= 63
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(invalid(
            "Nome do projeto: só letras minúsculas, números, '-' e '_' (até 63)",
        ))
    }
}

/// # Errors
///
/// `Validation` para URL que não é HTTPS ou que carrega credencial.
pub fn validate_repository_url(url: &str) -> Result<(), DockerError> {
    let Some(rest) = url.strip_prefix("https://") else {
        return Err(invalid("O repositório precisa de uma URL https://"));
    };
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() || host.contains('@') || rest.split('/').count() < 3 {
        return Err(invalid(
            "URL do repositório inválida (use https://host/dono/repo.git, sem usuário na URL)",
        ));
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid("URL do repositório com espaço"));
    }
    Ok(())
}

/// # Errors
///
/// `Validation` para o que não é SHA hexadecimal de 7 a 64 dígitos.
pub fn validate_commit(commit: &str) -> Result<(), DockerError> {
    if (7..=64).contains(&commit.len()) && commit.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(invalid(
            "Commit inválido: informe o SHA (7 a 64 dígitos hexadecimais)",
        ))
    }
}

/// Caminho relativo dentro do repositório, sem subir de nível.
///
/// # Errors
///
/// `Validation` para absoluto, `..`, vazio ou caractere de controle.
pub fn validate_relative_path(path: &str) -> Result<(), DockerError> {
    let valid = !path.is_empty()
        && path.len() <= 255
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains(':')
        && !path.starts_with('-')
        && path
            .split(['/', '\\'])
            .all(|part| !part.is_empty() && part != "..")
        && !path.chars().any(char::is_control);
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "Arquivo compose inválido: \"{path}\" (use um caminho relativo ao repositório)"
        )))
    }
}

/// Nome de variável de ambiente: `IMAGE_TAG`.
///
/// # Errors
///
/// `Validation` fora de `[A-Z_][A-Z0-9_]*`.
pub fn validate_variable(name: &str) -> Result<(), DockerError> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if valid {
        Ok(())
    } else {
        Err(invalid(
            "Variável da tag: letras maiúsculas, números e '_' (ex.: IMAGE_TAG)",
        ))
    }
}

/// Tag de imagem Docker: `[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}`.
///
/// # Errors
///
/// `Validation` fora do formato.
pub fn validate_image_tag(tag: &str) -> Result<(), DockerError> {
    let valid = !tag.is_empty()
        && tag.len() <= 128
        && tag
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if valid {
        Ok(())
    } else {
        Err(invalid("Tag de imagem inválida"))
    }
}

/// Host do registry, com porta opcional: `ghcr.io`, `registry.local:5000`.
///
/// # Errors
///
/// `Validation` para o que não é um host.
pub fn validate_registry(registry: &str) -> Result<(), DockerError> {
    let valid = !registry.is_empty()
        && registry.len() <= 255
        && registry
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
        && !registry.starts_with(['.', '-', ':']);
    if valid {
        Ok(())
    } else {
        Err(invalid(
            "Registry inválido: informe só o host (ex.: ghcr.io)",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> StackDeployRequest {
        StackDeployRequest {
            project: "sgc".into(),
            source: GitSource {
                repository_url: "https://github.com/master/sgcv4.git".into(),
                commit: "a1b2c3d4e5f6".into(),
                token: Some(Secret::new("ghp_segredo")),
            },
            compose_files: vec!["docker-compose.yml".into()],
            env_file: Some(Secret::new("APP_KEY=x")),
            image_tag_variable: "IMAGE_TAG".into(),
            image_tag: "a1b2c3d4e5f6".into(),
            registry: Some(RegistryAuth {
                registry: "ghcr.io".into(),
                username: "bot".into(),
                secret: Secret::new("ghp_registry"),
            }),
            wait_timeout_secs: 180,
            keep_images: 3,
            pull: true,
        }
    }

    #[test]
    fn pedido_completo_e_valido() {
        assert!(request().validate().is_ok());
    }

    #[test]
    fn debug_nao_mostra_segredo() {
        let shown = format!("{:?}", request());
        assert!(!shown.contains("ghp_segredo"));
        assert!(!shown.contains("ghp_registry"));
        assert!(!shown.contains("APP_KEY"));
        assert!(shown.contains("<oculto>"));
    }

    #[test]
    fn segredo_viaja_como_string() {
        let json = serde_json::to_value(Secret::new("abc")).expect("json");
        assert_eq!(json, serde_json::json!("abc"));
    }

    #[test]
    fn projeto_nao_vira_caminho() {
        for bad in ["", "../etc", "SGC", "sgc/x", ".sgc", "-sgc", "a b"] {
            assert!(validate_project(bad).is_err(), "{bad}");
        }
        assert!(validate_project("sgc-prod_2").is_ok());
    }

    #[test]
    fn repositorio_so_https_sem_credencial() {
        assert!(validate_repository_url("https://github.com/a/b.git").is_ok());
        for bad in [
            "http://github.com/a/b.git",
            "git@github.com:a/b.git",
            "https://user:token@github.com/a/b.git",
            "https://github.com",
            "https://github.com/a b/c",
        ] {
            assert!(validate_repository_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn commit_e_hexadecimal() {
        assert!(validate_commit("a1b2c3d").is_ok());
        assert!(validate_commit("a1b2c3").is_err());
        assert!(validate_commit("main").is_err());
        assert!(validate_commit("--upload-pack=x").is_err());
    }

    #[test]
    fn arquivo_compose_fica_no_repositorio() {
        assert!(validate_relative_path("deploy/docker-compose.yml").is_ok());
        for bad in [
            "/etc/passwd",
            "../x.yml",
            "a/../../x.yml",
            "C:\\x.yml",
            "-f",
            "",
        ] {
            assert!(validate_relative_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn variavel_e_tag_tem_formato_fechado() {
        assert!(validate_variable("IMAGE_TAG").is_ok());
        assert!(validate_variable("image_tag").is_err());
        assert!(validate_variable("1TAG").is_err());
        assert!(validate_image_tag("a1b2c3d").is_ok());
        assert!(validate_image_tag("v4.2.0").is_ok());
        assert!(validate_image_tag("-x").is_err());
        assert!(validate_image_tag("a;b").is_err());
    }

    #[test]
    fn limites_de_espera_e_retencao() {
        let mut bad = request();
        bad.wait_timeout_secs = 0;
        assert!(bad.validate().is_err());
        let mut bad = request();
        bad.keep_images = MAX_KEEP_IMAGES + 1;
        assert!(bad.validate().is_err());
        let mut bad = request();
        bad.compose_files.clear();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn registry_e_so_host() {
        assert!(validate_registry("ghcr.io").is_ok());
        assert!(validate_registry("registry.local:5000").is_ok());
        assert!(validate_registry("https://ghcr.io").is_err());
        assert!(validate_registry("ghcr.io/x").is_err());
    }

    #[test]
    fn chunk_tem_formato_estavel() {
        let stage = serde_json::to_value(StackChunk::Stage {
            stage: StackStage::Pull,
        })
        .expect("json");
        assert_eq!(stage, serde_json::json!({"kind": "stage", "stage": "pull"}));
        let line = serde_json::to_value(StackChunk::Line { text: "ok".into() }).expect("json");
        assert_eq!(line, serde_json::json!({"kind": "line", "text": "ok"}));
    }
}
