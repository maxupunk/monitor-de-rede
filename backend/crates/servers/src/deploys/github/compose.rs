//! Lê um `docker-compose.yml` e diz o que o assistente pode preencher
//! sozinho — puro, sem rede.
//!
//! * **imagens que o CI compila**: cada serviço com `build:`; o nome vem da
//!   `image:` (`ghcr.io/dono/sgc-api:${IMAGE_TAG:-latest}` → `sgc-api`) e o
//!   Dockerfile, do `build:` — relativo à raiz do repositório, que é como o
//!   workflow o usa;
//! * **a variável da tag**: o `${VAR}` na tag dessas imagens;
//! * **o que falta**: serviço que compila sem apontar para uma imagem
//!   publicada (no servidor nada é compilado), imagem fora do ghcr.io.

use serde_yaml::Value;

use crate::dtos::deploys::BuildImage;

/// O que saiu do compose.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposeAnalysis {
    pub services: Vec<String>,
    pub images: Vec<BuildImage>,
    pub image_tag_variable: Option<String>,
    /// Avisos para quem configura, já em palavras.
    pub notes: Vec<String>,
}

/// Os nomes que o `docker compose` procura, na ordem de preferência.
pub const DEFAULT_FILES: [&str; 4] = [
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

/// É um arquivo compose (inclusive os `override`, `prod`, `dev`)?
#[must_use]
pub fn is_compose_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some(stem) = lower
        .strip_suffix(".yml")
        .or_else(|| lower.strip_suffix(".yaml"))
    else {
        return false;
    };
    let base = stem.strip_prefix("docker-").unwrap_or(stem);
    base == "compose" || base.starts_with("compose.") || base.starts_with("compose-")
}

/// O arquivo principal entre os encontrados: o que o `docker compose` usaria
/// sozinho.
#[must_use]
pub fn main_file(files: &[String]) -> Option<String> {
    DEFAULT_FILES
        .iter()
        .find_map(|wanted| files.iter().find(|file| file.as_str() == *wanted))
        .cloned()
}

/// `a/./b/../c` → `a/c`; sem `./` na frente; `.` para a raiz.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

fn join(base: &str, path: &str) -> String {
    if base == "." {
        normalize(path)
    } else {
        normalize(&format!("{base}/{path}"))
    }
}

/// `ghcr.io/dono/sgc-api:${IMAGE_TAG:-latest}` → (`ghcr.io`, `sgc-api`,
/// `Some("IMAGE_TAG")`).
fn parse_image(image: &str) -> (Option<String>, String, Option<String>) {
    // A tag começa no primeiro `:` depois da última barra — o `:` de
    // `host:porta` vem antes dela, e o de `${VAR:-x}` vem depois.
    let name_start = image.rfind('/').map_or(0, |index| index + 1);
    let (path, tag) = match image[name_start..].find(':') {
        Some(offset) => {
            let index = name_start + offset;
            (&image[..index], Some(&image[index + 1..]))
        }
        None => (image, None),
    };
    let segments: Vec<&str> = path.split('/').collect();
    let registry = (segments.len() > 1
        && (segments[0].contains('.') || segments[0].contains(':') || segments[0] == "localhost"))
        .then(|| segments[0].to_ascii_lowercase());
    let name = segments.last().copied().unwrap_or_default().to_string();
    let variable = tag.and_then(|tag| {
        let start = tag.find("${")? + 2;
        let rest = &tag[start..];
        let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
        (end > 0).then(|| rest[..end].to_string())
    });
    (registry, name, variable)
}

/// O `build:` de um serviço: (contexto, Dockerfile), relativos à raiz.
fn parse_build(build: &Value, compose_dir: &str) -> Option<(String, String)> {
    let (context, dockerfile) = match build {
        Value::String(context) => (context.as_str(), "Dockerfile"),
        Value::Mapping(map) => (
            map.get("context").and_then(Value::as_str).unwrap_or("."),
            map.get("dockerfile")
                .and_then(Value::as_str)
                .unwrap_or("Dockerfile"),
        ),
        _ => return None,
    };
    // Contexto remoto (git, URL) não é deste repositório.
    if context.contains("://") || context.starts_with("git@") {
        return None;
    }
    let context = join(compose_dir, context);
    let dockerfile = join(&context, dockerfile);
    Some((context, dockerfile))
}

fn slug(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Analisa o compose. `path` é onde ele mora no repositório (os caminhos do
/// `build:` são relativos à pasta dele); `repository` (`dono/repo`) dá o nome
/// sugerido de imagem a quem não tem.
///
/// # Errors
///
/// YAML inválido ou sem `services`.
pub fn analyze(content: &str, path: &str, repository: &str) -> Result<ComposeAnalysis, String> {
    let document: Value = serde_yaml::from_str(content)
        .map_err(|error| format!("{path} não é um YAML válido: {error}"))?;
    let services = document
        .get("services")
        .and_then(Value::as_mapping)
        .ok_or_else(|| format!("{path} não tem a seção services"))?;
    let compose_dir = path.rsplit_once('/').map_or(".", |(dir, _)| dir);
    let repo_name = repository.rsplit('/').next().unwrap_or(repository);

    let mut out = ComposeAnalysis::default();
    for (key, service) in services {
        let Some(service_name) = key.as_str() else {
            continue;
        };
        out.services.push(service_name.to_string());
        let Some((context, dockerfile)) = service
            .get("build")
            .and_then(|b| parse_build(b, compose_dir))
        else {
            continue;
        };
        let image = service.get("image").and_then(Value::as_str);
        let name = match image {
            Some(image) => {
                let (registry, name, variable) = parse_image(image);
                if registry.as_deref() != Some("ghcr.io") {
                    out.notes.push(format!(
                        "O serviço {service_name} usa a imagem {image}, fora do ghcr.io: o \
                         workflow gerado publica no ghcr.io"
                    ));
                }
                match variable {
                    Some(variable) => {
                        if out.image_tag_variable.is_none() {
                            out.image_tag_variable = Some(variable);
                        }
                    }
                    None => out.notes.push(format!(
                        "A imagem do serviço {service_name} não usa uma variável na tag: use \
                         {image_base}:${{IMAGE_TAG:-latest}} para o deploy escolher a versão",
                        image_base = image.split(':').next().unwrap_or(image)
                    )),
                }
                name
            }
            None => {
                let suggested = slug(&format!("{repo_name}-{service_name}"));
                out.notes.push(format!(
                    "O serviço {service_name} compila (build:) mas não aponta para uma imagem \
                     publicada: acrescente image: ghcr.io/{owner}/{suggested}:${{IMAGE_TAG:-latest}}",
                    owner = repository.split('/').next().unwrap_or_default().to_ascii_lowercase()
                ));
                suggested
            }
        };
        if name.is_empty() || out.images.iter().any(|known| known.name == name) {
            continue;
        }
        out.images.push(BuildImage {
            name,
            dockerfile,
            context,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SGC: &str = r#"
name: sgc
services:
  sgc-api:
    image: ghcr.io/maxupunk/sgc-api:${IMAGE_TAG:-latest}
    build:
      context: .
      dockerfile: apps/api/Dockerfile
  worker:
    image: ghcr.io/maxupunk/sgc-api:${IMAGE_TAG:-latest}
    build:
      context: .
      dockerfile: apps/api/Dockerfile
  front:
    image: ghcr.io/maxupunk/sgc-front-tenants:${IMAGE_TAG:-latest}
    build:
      context: ./docker/frontend
  postgres:
    image: postgres:18
"#;

    #[test]
    fn imagens_e_variavel_do_sgc() {
        let analysis = analyze(SGC, "docker-compose.yml", "maxupunk/SGCv4").expect("ok");
        assert_eq!(analysis.image_tag_variable.as_deref(), Some("IMAGE_TAG"));
        assert_eq!(
            analysis.images,
            vec![
                BuildImage {
                    name: "sgc-api".into(),
                    dockerfile: "apps/api/Dockerfile".into(),
                    context: ".".into(),
                },
                BuildImage {
                    name: "sgc-front-tenants".into(),
                    dockerfile: "docker/frontend/Dockerfile".into(),
                    context: "docker/frontend".into(),
                },
            ]
        );
        assert_eq!(analysis.services.len(), 4);
        assert!(analysis.notes.is_empty(), "{:?}", analysis.notes);
    }

    #[test]
    fn servico_que_so_compila_ganha_nome_e_aviso() {
        let compose = "services:\n  web:\n    build: ./web\n";
        let analysis = analyze(compose, "deploy/compose.yml", "Dono/App").expect("ok");
        assert_eq!(analysis.images[0].name, "app-web");
        assert_eq!(analysis.images[0].context, "deploy/web");
        assert_eq!(analysis.images[0].dockerfile, "deploy/web/Dockerfile");
        assert!(analysis.notes[0].contains("ghcr.io/dono/app-web"));
    }

    #[test]
    fn imagem_sem_variavel_ou_fora_do_ghcr_avisa() {
        let compose = "services:\n  api:\n    image: docker.io/acme/api:1.0\n    build: .\n";
        let analysis = analyze(compose, "docker-compose.yml", "acme/api").expect("ok");
        assert_eq!(analysis.notes.len(), 2);
        assert!(analysis.image_tag_variable.is_none());
    }

    #[test]
    fn imagem_com_porta_no_registry() {
        let (registry, name, variable) = parse_image("localhost:5000/time/api:${TAG}");
        assert_eq!(registry.as_deref(), Some("localhost:5000"));
        assert_eq!(name, "api");
        assert_eq!(variable.as_deref(), Some("TAG"));
        let (registry, name, variable) = parse_image("nginx");
        assert!(registry.is_none());
        assert_eq!(name, "nginx");
        assert!(variable.is_none());
        let (_, name, variable) = parse_image("ghcr.io/a/api:${IMAGE_TAG:-latest}");
        assert_eq!(name, "api");
        assert_eq!(variable.as_deref(), Some("IMAGE_TAG"));
    }

    #[test]
    fn yaml_invalido_ou_sem_servicos() {
        assert!(analyze("services: [", "c.yml", "a/b").is_err());
        assert!(analyze("version: '3'", "c.yml", "a/b").is_err());
    }

    #[test]
    fn reconhece_os_arquivos_compose() {
        for name in [
            "docker-compose.yml",
            "compose.yaml",
            "docker-compose.prod.yml",
            "compose.override.yaml",
            "docker-compose-dev.yml",
        ] {
            assert!(is_compose_file(name), "{name}");
        }
        for name in [
            "composer.json",
            "compose.txt",
            "my-compose.yml",
            "docker.yml",
        ] {
            assert!(!is_compose_file(name), "{name}");
        }
        let files = vec![
            "docker-compose.dev.yml".to_string(),
            "docker-compose.yml".to_string(),
        ];
        assert_eq!(main_file(&files).as_deref(), Some("docker-compose.yml"));
    }

    #[test]
    fn caminhos_relativos_a_raiz() {
        assert_eq!(join(".", "./apps/../api/Dockerfile"), "api/Dockerfile");
        assert_eq!(join("deploy", "../Dockerfile"), "Dockerfile");
        assert_eq!(normalize("./"), ".");
    }
}
