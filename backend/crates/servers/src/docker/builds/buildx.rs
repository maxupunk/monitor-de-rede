//! Os comandos `docker buildx` de uma compilação, montados como lista de
//! argumentos (nunca por shell) — funções puras, testáveis sem Docker.
//!
//! O builder é **dedicado** (ADR 023): um container BuildKit próprio
//! (`docker-container`), numa rede própria, com teto de CPU e memória. Ele
//! não monta o `docker.sock` nem o disco do host, e o cache dele não se
//! mistura com o das imagens em produção.

use std::path::Path;

use agent_protocol::docker::images::{BuildImageSpec, ImageBuildRequest};

/// Nome do builder dedicado.
pub const BUILDER: &str = "netmonitor-builder";
/// Rede só do builder: não enxerga as redes das stacks.
pub const BUILD_NETWORK: &str = "netmonitor-build";
/// Teto de CPU do builder (núcleos, ex.: `2` ou `1.5`).
pub const CPUS_ENV: &str = "AGENT_BUILD_CPUS";
/// Teto de memória do builder (ex.: `4g`).
pub const MEMORY_ENV: &str = "AGENT_BUILD_MEMORY";
/// Prefixo das variáveis de ambiente que levam os segredos ao `buildx`.
const SECRET_ENV_PREFIX: &str = "NETMONITOR_BUILD_SECRET_";

/// Limites do builder, lidos da política local do host.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuilderLimits {
    pub cpus: Option<f64>,
    pub memory: Option<String>,
}

impl BuilderLimits {
    /// `AGENT_BUILD_CPUS`/`AGENT_BUILD_MEMORY`. Sem CPU definida, deixa um
    /// núcleo livre para o resto do host (a medição da central inclusive).
    #[must_use]
    pub fn from_env(host_cpus: Option<u32>) -> Self {
        let cpus = std::env::var(CPUS_ENV)
            .ok()
            .and_then(|raw| raw.trim().parse::<f64>().ok())
            .filter(|cpus| *cpus > 0.0)
            .or_else(|| {
                host_cpus
                    .filter(|cpus| *cpus > 1)
                    .map(|cpus| f64::from(cpus - 1))
            });
        let memory = std::env::var(MEMORY_ENV)
            .ok()
            .map(|raw| raw.trim().to_ascii_lowercase())
            .filter(|raw| valid_memory(raw));
        Self { cpus, memory }
    }

    #[must_use]
    pub fn cpu_label(&self) -> Option<String> {
        self.cpus.map(|cpus| format!("{cpus} núcleo(s)"))
    }
}

fn valid_memory(raw: &str) -> bool {
    let digits = raw.strip_suffix(['b', 'k', 'm', 'g']).unwrap_or(raw);
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}

/// `docker buildx create` do builder dedicado.
#[must_use]
pub fn create_args(limits: &BuilderLimits) -> Vec<String> {
    let mut args = strings(&[
        "buildx",
        "create",
        "--name",
        BUILDER,
        "--driver",
        "docker-container",
        "--driver-opt",
    ]);
    args.push(format!("network={BUILD_NETWORK}"));
    // Prioridade baixa: compilando junto da central, quem perde é o build.
    args.extend(strings(&["--driver-opt", "cpu-shares=256"]));
    if let Some(cpus) = limits.cpus {
        let quota = (cpus * 100_000.0).round();
        args.extend(strings(&[
            "--driver-opt",
            "cpu-period=100000",
            "--driver-opt",
        ]));
        args.push(format!("cpu-quota={quota:.0}"));
    }
    if let Some(memory) = &limits.memory {
        args.push("--driver-opt".into());
        args.push(format!("memory={memory}"));
    }
    args.push("--bootstrap".into());
    args
}

/// A variável que leva o segredo `id` ao `buildx`.
#[must_use]
pub fn secret_env(id: &str) -> String {
    format!("{SECRET_ENV_PREFIX}{id}")
}

/// Ambiente do processo filho: os segredos (nunca na linha de comando).
#[must_use]
pub fn secret_envs(request: &ImageBuildRequest) -> Vec<(String, String)> {
    request
        .secrets
        .iter()
        .map(|secret| (secret_env(&secret.id), secret.value.expose().to_string()))
        .collect()
}

/// O que todo `buildx build` desta compilação leva: builder, saída legível,
/// argumentos, segredos (por referência à variável) e cache.
fn common(request: &ImageBuildRequest, workdir: &Path, dockerfile: &str) -> Vec<String> {
    let mut args = strings(&[
        "buildx",
        "build",
        "--builder",
        BUILDER,
        "--progress",
        "plain",
    ]);
    args.push("--file".into());
    args.push(workdir.join(dockerfile).display().to_string());
    for arg in &request.build_args {
        args.push("--build-arg".into());
        args.push(format!("{}={}", arg.name, arg.value));
    }
    for secret in &request.secrets {
        args.push("--secret".into());
        args.push(format!("id={},env={}", secret.id, secret_env(&secret.id)));
    }
    if request.no_cache {
        args.push("--no-cache".into());
    }
    args
}

/// O estágio de teste: roda e descarta o resultado (`type=cacheonly`).
#[must_use]
pub fn test_args(request: &ImageBuildRequest, workdir: &Path, target: &str) -> Option<Vec<String>> {
    let first = request.images.first()?;
    let mut args = common(request, workdir, &first.dockerfile);
    args.extend(strings(&["--target", target, "--output", "type=cacheonly"]));
    args.push(workdir.join(&first.context).display().to_string());
    Some(args)
}

/// Uma imagem: arquivo OCI com SBOM e proveniência anexados (a varredura e o
/// registro de origem da central leem dali).
#[must_use]
pub fn image_args(
    request: &ImageBuildRequest,
    image: &BuildImageSpec,
    workdir: &Path,
    output: &Path,
) -> Vec<String> {
    let mut args = common(request, workdir, &image.dockerfile);
    if let Some(target) = &image.target {
        args.push("--target".into());
        args.push(target.clone());
    }
    args.extend(strings(&[
        "--sbom",
        "true",
        "--provenance",
        "mode=max",
        "--label",
    ]));
    args.push(format!(
        "org.opencontainers.image.revision={}",
        request.source.commit
    ));
    args.push("--label".into());
    args.push(format!(
        "org.opencontainers.image.source={}",
        request.source.repository_url.trim_end_matches(".git")
    ));
    args.push("--output".into());
    args.push(format!("type=oci,dest={}", output.display()));
    args.push(workdir.join(&image.context).display().to_string());
    args
}

#[cfg(test)]
mod tests {
    use agent_protocol::docker::{
        images::{BuildArg, BuildSecret},
        stacks::{GitSource, Secret},
    };

    use super::*;

    fn request() -> ImageBuildRequest {
        ImageBuildRequest {
            source: GitSource {
                repository_url: "https://github.com/acme/loja.git".into(),
                commit: "c".repeat(40),
                token: Some(Secret::new("ghs_token")),
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
            no_cache: true,
            timeout_secs: 600,
        }
    }

    #[test]
    fn comando_da_imagem_completo_e_sem_segredo() {
        let request = request();
        let args = image_args(
            &request,
            &request.images[0],
            Path::new("/b/src"),
            Path::new("/b/out/a.tar"),
        );
        let line = args.join(" ");
        assert!(line.starts_with("buildx build --builder netmonitor-builder --progress plain"));
        assert!(line.contains("--build-arg NODE_ENV=production"));
        assert!(line.contains("--secret id=NPM_TOKEN,env=NETMONITOR_BUILD_SECRET_NPM_TOKEN"));
        assert!(line.contains("--no-cache"));
        assert!(line.contains("--target runner"));
        assert!(line.contains("--sbom true --provenance mode=max"));
        assert!(line.contains("org.opencontainers.image.source=https://github.com/acme/loja"));
        assert!(line.contains("type=oci,dest="));
        assert!(!line.contains("npm_segredo"));
        assert!(!line.contains("ghs_token"));
        assert_eq!(
            secret_envs(&request),
            vec![(
                "NETMONITOR_BUILD_SECRET_NPM_TOKEN".to_string(),
                "npm_segredo".to_string()
            )]
        );
    }

    #[test]
    fn teste_descarta_o_resultado() {
        let request = request();
        let args = test_args(&request, Path::new("/b/src"), "test").unwrap();
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--output", "type=cacheonly"]));
        assert!(args.windows(2).any(|pair| pair == ["--target", "test"]));
    }

    #[test]
    fn builder_com_limites() {
        let args = create_args(&BuilderLimits {
            cpus: Some(1.5),
            memory: Some("4g".into()),
        });
        let line = args.join(" ");
        assert!(line.contains("--driver docker-container"));
        assert!(line.contains("network=netmonitor-build"));
        assert!(line.contains("cpu-quota=150000"));
        assert!(line.contains("memory=4g"));
        assert!(!create_args(&BuilderLimits::default())
            .join(" ")
            .contains("cpu-quota"));
    }

    #[test]
    fn memoria_valida() {
        for good in ["4g", "512m", "1073741824"] {
            assert!(valid_memory(good), "{good}");
        }
        for bad in ["", "quatro", "-1g", "4gg", "g"] {
            assert!(!valid_memory(bad), "{bad}");
        }
    }
}
