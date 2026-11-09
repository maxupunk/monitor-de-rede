//! Quais imagens antigas da stack apagar depois de um deploy bem-sucedido.
//!
//! Cada deploy baixa uma tag nova (o SHA do commit). Sem limpeza, o disco
//! do servidor enche em meses; limpando tudo, o rollback teria de baixar a
//! versão anterior de novo — justo quando o registry pode estar fora. A
//! regra: por repositório usado pela stack, manter as `keep` tags mais
//! recentes **além** das em uso, e nunca tocar numa imagem com container.

use std::collections::{BTreeMap, BTreeSet};

use agent_protocol::docker::views::DockerImageSummary;

/// `ghcr.io/dono/api:abc` → `ghcr.io/dono/api`. A porta do registry
/// (`host:5000/x`) não é confundida com a tag.
#[must_use]
pub fn repository_of(reference: &str) -> &str {
    let reference = reference.split('@').next().unwrap_or(reference);
    match reference.rfind(':') {
        Some(colon) if !reference[colon..].contains('/') => &reference[..colon],
        _ => reference,
    }
}

/// Ids das imagens a remover.
///
/// `in_use` são os ids das imagens dos containers da stack; `repositories`,
/// os repositórios delas.
#[must_use]
pub fn to_remove(
    images: &[DockerImageSummary],
    repositories: &BTreeSet<String>,
    in_use: &BTreeSet<String>,
    keep: u32,
) -> Vec<String> {
    let mut by_repository: BTreeMap<&str, Vec<&DockerImageSummary>> = BTreeMap::new();
    for image in images {
        if in_use.contains(&image.id) || image.containers > 0 {
            continue;
        }
        let Some(repository) = image
            .repo_tags
            .iter()
            .map(|tag| repository_of(tag))
            .find(|repository| repositories.contains(*repository))
        else {
            continue;
        };
        by_repository.entry(repository).or_default().push(image);
    }
    let mut output = Vec::new();
    for mut candidates in by_repository.into_values() {
        candidates.sort_by_key(|image| std::cmp::Reverse(image.created));
        output.extend(
            candidates
                .into_iter()
                .skip(usize::try_from(keep).unwrap_or(usize::MAX))
                .map(|image| image.id.clone()),
        );
    }
    output
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn image(id: &str, tag: &str, created: i64) -> DockerImageSummary {
        DockerImageSummary {
            id: id.into(),
            parent_id: String::new(),
            repo_tags: vec![tag.into()],
            repo_digests: Vec::new(),
            created,
            size: 0,
            shared_size: 0,
            labels: HashMap::new(),
            containers: 0,
        }
    }

    #[test]
    fn repositorio_sem_tag_e_sem_confundir_porta() {
        assert_eq!(repository_of("ghcr.io/a/api:abc"), "ghcr.io/a/api");
        assert_eq!(
            repository_of("registry:5000/a/api:abc"),
            "registry:5000/a/api"
        );
        assert_eq!(repository_of("registry:5000/a/api"), "registry:5000/a/api");
        assert_eq!(repository_of("nginx"), "nginx");
        assert_eq!(repository_of("a/api@sha256:ff"), "a/api");
    }

    #[test]
    fn mantem_as_mais_recentes_e_a_em_uso() {
        let images = vec![
            image("atual", "ghcr.io/a/api:5", 50),
            image("v4", "ghcr.io/a/api:4", 40),
            image("v3", "ghcr.io/a/api:3", 30),
            image("v2", "ghcr.io/a/api:2", 20),
            image("outra", "redis:7", 10),
        ];
        let repositories = BTreeSet::from(["ghcr.io/a/api".to_string()]);
        let in_use = BTreeSet::from(["atual".to_string()]);
        assert_eq!(
            to_remove(&images, &repositories, &in_use, 2),
            vec!["v2".to_string()]
        );
        assert_eq!(
            to_remove(&images, &repositories, &in_use, 0),
            vec!["v4".to_string(), "v3".to_string(), "v2".to_string()]
        );
    }

    #[test]
    fn imagem_com_container_nunca_sai() {
        let mut parada = image("parada", "ghcr.io/a/api:1", 1);
        parada.containers = 1;
        let repositories = BTreeSet::from(["ghcr.io/a/api".to_string()]);
        assert!(to_remove(&[parada], &repositories, &BTreeSet::new(), 0).is_empty());
    }
}
