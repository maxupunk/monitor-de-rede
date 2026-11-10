//! Compilação no servidor e trânsito de imagens pelo canal (ADR 022 e 023).
//!
//! * [`ImageBuildRequest`] — `DockerCall::ImageBuild`, permissão `build`: o
//!   agente clona o commit, roda o estágio de teste (se houver) e compila cada
//!   imagem no builder BuildKit dedicado, em arquivos OCI guardados no próprio
//!   agente até a central buscá-los;
//! * `Command::ImageExport` — o arquivo de uma compilação, do agente para a
//!   central, pela ponte binária (permissão `build`);
//! * `Command::ImageImport` — uma imagem do registry da central para o Docker
//!   do agente (`docker load`), pela mesma ponte no outro sentido (permissão
//!   `deploy`).
//!
//! Nada aqui leva comando de shell nem caminho absoluto: o que a central manda
//! são nomes, caminhos relativos ao repositório e valores validados dos dois
//! lados.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{
    stacks::{
        validate_commit, validate_image_tag, validate_relative_path, validate_repository_url,
        validate_variable, GitSource, Secret,
    },
    DockerError,
};

/// Teto de uma compilação inteira (clone, teste e todas as imagens).
pub const MAX_BUILD_TIMEOUT_SECS: u32 = 3 * 60 * 60;
/// Imagens de uma aplicação compiladas num pedido só.
pub const MAX_BUILD_IMAGES: usize = 16;
/// Argumentos e segredos de build de um pedido.
pub const MAX_BUILD_VALUES: usize = 64;
/// Teto do valor de um argumento de build.
const MAX_BUILD_ARG_BYTES: usize = 4096;

/// Uma imagem a compilar: um serviço com `build:` no compose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildImageSpec {
    /// Nome curto (`api`, `frontend`), usado no registry da central.
    pub name: String,
    /// Dockerfile, relativo à raiz do repositório.
    pub dockerfile: String,
    /// Contexto, relativo à raiz do repositório.
    pub context: String,
    /// Estágio final (`--target`), quando o Dockerfile tem vários.
    #[serde(default)]
    pub target: Option<String>,
}

/// Argumento de build (`--build-arg`). Não é lugar de segredo: vai para o
/// histórico da imagem. Segredo vai em [`BuildSecret`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildArg {
    pub name: String,
    pub value: String,
}

/// Segredo de build (`--secret id=NOME`), lido no Dockerfile com
/// `RUN --mount=type=secret,id=NOME`. Vai ao `buildx` pelo ambiente do
/// processo filho, nunca pela linha de comando, e não entra em camada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildSecret {
    pub id: String,
    pub value: Secret,
}

/// `DockerCall::ImageBuild`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageBuildRequest {
    /// O commit a compilar e o token de clone (uma hora, só leitura).
    pub source: GitSource,
    pub images: Vec<BuildImageSpec>,
    /// Estágio do Dockerfile que roda os testes antes das imagens. Usa o
    /// Dockerfile da primeira imagem.
    #[serde(default)]
    pub test_target: Option<String>,
    #[serde(default)]
    pub build_args: Vec<BuildArg>,
    #[serde(default)]
    pub secrets: Vec<BuildSecret>,
    /// Ignora o cache do builder (`--no-cache`).
    #[serde(default)]
    pub no_cache: bool,
    pub timeout_secs: u32,
}

fn invalid(message: impl Into<String>) -> DockerError {
    DockerError::Validation(message.into())
}

/// Nome de imagem no registry: um componente de caminho OCI.
///
/// # Errors
///
/// `Validation` com a regra.
pub fn validate_image_name(name: &str) -> Result<(), DockerError> {
    let bytes = name.as_bytes();
    let valid = !name.is_empty()
        && name.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
        && !name.contains("..")
        && !name.contains("--")
        && !name.contains("__");
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "Nome de imagem inválido: '{name}' (minúsculas, números, '.', '_' e '-')"
        )))
    }
}

/// Estágio de Dockerfile (`--target`).
///
/// # Errors
///
/// `Validation` com a regra.
pub fn validate_target(target: &str) -> Result<(), DockerError> {
    let valid = !target.is_empty()
        && target.len() <= 64
        && target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "Estágio do Dockerfile inválido: '{target}'"
        )))
    }
}

/// Referência completa de imagem (`registry/caminho/nome:tag`), como o
/// `image:` do compose a declara.
///
/// # Errors
///
/// `Validation` com a regra.
pub fn validate_reference(reference: &str) -> Result<(), DockerError> {
    // A tag começa no último `:` depois da última `/` (o `:` antes é porta).
    let slash = reference.rfind('/').map_or(0, |at| at + 1);
    let (repository, tag) = match reference[slash..].rfind(':') {
        Some(at) => (&reference[..slash + at], Some(&reference[slash + at + 1..])),
        None => (reference, None),
    };
    let valid_repository = !repository.is_empty()
        && repository.len() <= 255
        && !repository.starts_with('/')
        && !repository.ends_with('/')
        && !repository.contains("//")
        && repository.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-' | '/' | ':')
        });
    if !valid_repository {
        return Err(invalid(format!(
            "Referência de imagem inválida: '{reference}'"
        )));
    }
    if let Some(tag) = tag {
        validate_image_tag(tag)?;
    }
    Ok(())
}

impl ImageBuildRequest {
    /// # Errors
    ///
    /// `Validation` com o primeiro problema encontrado.
    pub fn validate(&self) -> Result<(), DockerError> {
        validate_repository_url(&self.source.repository_url)?;
        validate_commit(&self.source.commit)?;
        if self.images.is_empty() {
            return Err(invalid("Nada a compilar: nenhuma imagem com Dockerfile"));
        }
        if self.images.len() > MAX_BUILD_IMAGES {
            return Err(invalid(format!(
                "No máximo {MAX_BUILD_IMAGES} imagens por compilação"
            )));
        }
        for image in &self.images {
            validate_image_name(&image.name)?;
            validate_relative_path(&image.dockerfile)?;
            validate_relative_path(&image.context)?;
            if let Some(target) = &image.target {
                validate_target(target)?;
            }
        }
        let mut names: Vec<&str> = self
            .images
            .iter()
            .map(|image| image.name.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        if names.len() != self.images.len() {
            return Err(invalid("Duas imagens com o mesmo nome"));
        }
        if let Some(target) = &self.test_target {
            validate_target(target)?;
        }
        if self.build_args.len() + self.secrets.len() > MAX_BUILD_VALUES {
            return Err(invalid(format!(
                "No máximo {MAX_BUILD_VALUES} argumentos e segredos de build"
            )));
        }
        for arg in &self.build_args {
            validate_variable(&arg.name)?;
            if arg.value.len() > MAX_BUILD_ARG_BYTES || arg.value.chars().any(char::is_control) {
                return Err(invalid(format!(
                    "Valor do argumento {} inválido (sem quebra de linha, até 4 KB)",
                    arg.name
                )));
            }
        }
        for secret in &self.secrets {
            validate_variable(&secret.id)?;
        }
        if self.timeout_secs == 0 || self.timeout_secs > MAX_BUILD_TIMEOUT_SECS {
            return Err(invalid(format!(
                "O tempo limite da compilação vai de 1 a {MAX_BUILD_TIMEOUT_SECS} segundos"
            )));
        }
        Ok(())
    }
}

/// Etapa da compilação, para o console ao vivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum BuildStage {
    /// Prepara o builder dedicado.
    Prepare,
    /// Busca o commit.
    Clone,
    /// Roda o estágio de teste.
    Test,
    /// Compila uma imagem.
    Build,
    /// Leva as imagens ao registry da central.
    Store,
    /// Confere vulnerabilidades.
    Scan,
}

/// O que sai do agente durante a compilação (em `Chunk`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BuildChunk {
    Stage {
        stage: BuildStage,
        #[serde(default)]
        image: Option<String>,
    },
    Line {
        text: String,
    },
}

/// Uma imagem compilada, guardada no agente até a central buscá-la.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltArtifact {
    pub image: String,
    pub artifact: Uuid,
    pub bytes: u64,
}

/// Resposta de `DockerCall::ImageBuild`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageBuildOutcome {
    pub commit: String,
    pub artifacts: Vec<BuiltArtifact>,
}

/// `Command::ImageImport`: as referências que a imagem ganha no Docker do
/// agente (as do `image:` do compose, com a tag do commit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageImportRequest {
    pub references: Vec<String>,
}

impl ImageImportRequest {
    /// # Errors
    ///
    /// `Validation` com o primeiro problema encontrado.
    pub fn validate(&self) -> Result<(), DockerError> {
        if self.references.is_empty() || self.references.len() > MAX_BUILD_IMAGES {
            return Err(invalid("Informe de 1 a 16 referências"));
        }
        self.references
            .iter()
            .try_for_each(|reference| validate_reference(reference))
    }
}

/// Como está o builder deste host (`DockerCall::BuilderReadiness`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct BuilderReadiness {
    /// `docker buildx version`, quando o plugin existe.
    pub buildx_version: Option<String>,
    /// O builder dedicado já existe (é criado na primeira compilação).
    pub builder_ready: bool,
    pub git_version: Option<String>,
    /// Espaço livre onde as compilações trabalham.
    #[ts(type = "number | null")]
    pub free_bytes: Option<u64>,
    /// Núcleos do host.
    pub cpus: Option<u32>,
    /// Limites aplicados ao builder (`AGENT_BUILD_CPUS`, `AGENT_BUILD_MEMORY`).
    pub cpu_limit: Option<String>,
    pub memory_limit: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ImageBuildRequest {
        ImageBuildRequest {
            source: GitSource {
                repository_url: "https://github.com/acme/loja.git".into(),
                commit: "a".repeat(40),
                token: Some(Secret::new("ghs_x")),
            },
            images: vec![BuildImageSpec {
                name: "loja-api".into(),
                dockerfile: "api/Dockerfile".into(),
                context: ".".into(),
                target: Some("runner".into()),
            }],
            test_target: Some("test".into()),
            build_args: vec![BuildArg {
                name: "NODE_ENV".into(),
                value: "production".into(),
            }],
            secrets: vec![BuildSecret {
                id: "NPM_TOKEN".into(),
                value: Secret::new("npm_segredo"),
            }],
            no_cache: false,
            timeout_secs: 1800,
        }
    }

    #[test]
    fn pedido_completo_e_valido() {
        request().validate().unwrap();
    }

    #[test]
    fn debug_nao_mostra_segredo_nem_token() {
        let text = format!("{:?}", request());
        assert!(!text.contains("npm_segredo"));
        assert!(!text.contains("ghs_x"));
    }

    #[test]
    fn caminho_fora_do_repositorio_e_nome_ruim_sao_recusados() {
        let mut escaping = request();
        escaping.images[0].dockerfile = "../etc/passwd".into();
        assert!(escaping.validate().is_err());
        let mut absolute = request();
        absolute.images[0].context = "/".into();
        assert!(absolute.validate().is_err());
        let mut name = request();
        name.images[0].name = "Loja API".into();
        assert!(name.validate().is_err());
        let mut arg = request();
        arg.build_args[0].value = "a\nb".into();
        assert!(arg.validate().is_err());
        let mut twice = request();
        twice.images.push(twice.images[0].clone());
        assert!(twice.validate().is_err());
    }

    #[test]
    fn referencias_do_compose() {
        for good in [
            "ghcr.io/maxupunk/brainblaster-backend:abc123",
            "registry.local:5000/acme/api:v1.2.3",
            "nginx",
            "acme/api",
        ] {
            validate_reference(good).unwrap_or_else(|_| panic!("recusou {good}"));
        }
        for bad in [
            "",
            "Acme/api",
            "acme//api",
            "acme/api:tag com espaço",
            "/api",
        ] {
            assert!(validate_reference(bad).is_err(), "aceitou {bad}");
        }
    }
}
