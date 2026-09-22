//! O que se marca com `@` no chat: dispositivos, monitores, containers e
//! fontes de dados inteiras (logs, alertas, checagens, Docker).
//!
//! A tela pede sugestões conforme o usuário digita ([`search`]); a pergunta
//! volta com as marcações e o prompt diz à IA de qual recurso se trata
//! ([`sanitize`] limita o que chega). A comparação sem caixa roda em memória,
//! como em `harness::tools::lookup`: as listas são pequenas e o `LIKE` difere
//! entre SQLite e PostgreSQL.

use std::collections::HashSet;

use sea_orm::{ConnectionTrait, EntityTrait, QueryOrder};

use super::harness::tools::device_matches;
use crate::{
    dtos::ai::{AiMention, AiMentionKind},
    models::{devices, monitors},
    services::{docker, shared::errors::AppResult},
};

/// Sugestões por tipo na lista do `@`.
const PER_KIND: usize = 6;

/// Marcações aproveitadas por pergunta; o resto é descartado.
pub const MAX_MENTIONS: usize = 8;

const MAX_LABEL_CHARS: usize = 80;

/// Uma fonte de dados que pode ser marcada inteira.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MentionSource {
    pub id: &'static str,
    pub label: &'static str,
    /// Como a IA consulta a fonte — vai para o prompt.
    pub hint: &'static str,
    /// Palavras que também encontram a fonte na busca.
    keywords: &'static str,
}

pub const SOURCES: [MentionSource; 4] = [
    MentionSource {
        id: "logs",
        label: "Logs",
        hint: "syslog dos equipamentos e da aplicação — get_logs_overview para o panorama, grep source='logs' para procurar",
        keywords: "logs syslog eventos",
    },
    MentionSource {
        id: "alerts",
        label: "Alertas",
        hint: "alertas abertos e resolvidos — get_alerts, ou grep source='alerts' para procurar nas mensagens",
        keywords: "alertas alerts incidentes",
    },
    MentionSource {
        id: "checks",
        label: "Checagens",
        hint: "falhas das checagens dos monitores — grep source='checks'",
        keywords: "checagens checks falhas monitores",
    },
    MentionSource {
        id: "docker",
        label: "Docker",
        hint: "containers da Docker Engine — get_docker_containers; logs de um container com grep source='docker' container=<nome>",
        keywords: "docker containers",
    },
];

#[must_use]
pub fn source(id: &str) -> Option<&'static MentionSource> {
    SOURCES.iter().find(|source| source.id == id)
}

fn fold(text: &str) -> String {
    text.trim().to_lowercase()
}

fn contains(haystack: &str, term: &str) -> bool {
    term.is_empty() || fold(haystack).contains(term)
}

fn device_mention(device: &devices::Model) -> AiMention {
    AiMention {
        kind: AiMentionKind::Device,
        id: device.id.to_string(),
        label: device.name.clone(),
        detail: Some(format!(
            "{} · {} · {}",
            device.r#type,
            device.ip_address.as_deref().unwrap_or("sem IP"),
            device.status
        )),
    }
}

fn monitor_mention(monitor: &monitors::Model) -> AiMention {
    AiMention {
        kind: AiMentionKind::Monitor,
        id: monitor.id.to_string(),
        label: monitor.name.clone(),
        detail: Some(format!("{} · {}", monitor.r#type, monitor.status)),
    }
}

fn source_mentions(term: &str) -> Vec<AiMention> {
    SOURCES
        .iter()
        .filter(|source| contains(source.label, term) || contains(source.keywords, term))
        .map(|source| AiMention {
            kind: AiMentionKind::Source,
            id: source.id.to_string(),
            label: source.label.to_string(),
            detail: Some("fonte de dados".into()),
        })
        .collect()
}

/// Containers só quando a Docker Engine responde; sem ela, a lista segue sem eles.
async fn container_mentions(term: &str) -> Vec<AiMention> {
    if !docker::enabled() {
        return Vec::new();
    }
    let Ok(containers) = docker::engine::list_containers().await else {
        return Vec::new();
    };
    containers
        .iter()
        .filter(|container| contains(&container.display_name(), term))
        .take(PER_KIND)
        .map(|container| AiMention {
            kind: AiMentionKind::Container,
            id: container.id.chars().take(12).collect(),
            label: container.display_name(),
            detail: Some(format!("{} · {}", container.image, container.state)),
        })
        .collect()
}

/// Sugestões para o que o usuário digitou depois do `@`: dispositivos,
/// monitores, fontes e containers. Termo vazio lista os primeiros de cada.
///
/// # Errors
///
/// Falha do banco.
pub async fn search<C: ConnectionTrait>(db: &C, query: &str) -> AppResult<Vec<AiMention>> {
    let term = fold(query);
    let mut mentions: Vec<AiMention> = devices::Entity::find()
        .order_by_asc(devices::Column::Name)
        .all(db)
        .await?
        .iter()
        .filter(|device| device_matches(device, &term))
        .take(PER_KIND)
        .map(device_mention)
        .collect();
    mentions.extend(
        monitors::Entity::find()
            .order_by_asc(monitors::Column::Name)
            .all(db)
            .await?
            .iter()
            .filter(|monitor| contains(&monitor.name, &term))
            .take(PER_KIND)
            .map(monitor_mention),
    );
    mentions.extend(source_mentions(&term));
    mentions.extend(container_mentions(&term).await);
    Ok(mentions)
}

/// O que vale das marcações recebidas: sem repetição, sem rótulo vazio, sem
/// fonte desconhecida, rótulo curto e no máximo [`MAX_MENTIONS`].
#[must_use]
pub fn sanitize(mentions: Vec<AiMention>) -> Vec<AiMention> {
    let mut seen = HashSet::new();
    mentions
        .into_iter()
        .filter_map(|mut mention| {
            mention.id = mention.id.trim().to_string();
            mention.label = mention.label.trim().chars().take(MAX_LABEL_CHARS).collect();
            mention.detail = None;
            let valid = !mention.id.is_empty()
                && !mention.label.is_empty()
                && (mention.kind != AiMentionKind::Source || source(&mention.id).is_some());
            (valid && seen.insert((mention.kind, mention.id.clone()))).then_some(mention)
        })
        .take(MAX_MENTIONS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mention(kind: AiMentionKind, id: &str, label: &str) -> AiMention {
        AiMention {
            kind,
            id: id.into(),
            label: label.into(),
            detail: Some("x".into()),
        }
    }

    #[test]
    fn fonte_e_encontrada_pelo_nome_ou_por_palavra_chave() {
        let ids = |term: &str| -> Vec<String> {
            source_mentions(term).into_iter().map(|m| m.id).collect()
        };
        assert_eq!(ids("syslog"), vec!["logs"]);
        assert_eq!(ids("doc"), vec!["docker"]);
        assert_eq!(ids("").len(), SOURCES.len());
    }

    #[test]
    fn sanitize_descarta_repetida_vazia_e_fonte_inventada() {
        let limpas = sanitize(vec![
            mention(AiMentionKind::Device, "12", " MPPT "),
            mention(AiMentionKind::Device, "12", "MPPT de novo"),
            mention(AiMentionKind::Monitor, "3", "  "),
            mention(AiMentionKind::Source, "etc", "Etc"),
            mention(AiMentionKind::Source, "logs", "Logs"),
        ]);
        assert_eq!(limpas.len(), 2);
        assert_eq!(limpas[0].label, "MPPT");
        assert!(limpas[0].detail.is_none(), "detalhe é só da lista");
        assert_eq!(limpas[1].id, "logs");
    }

    #[test]
    fn sanitize_respeita_o_teto() {
        let muitas = (0..20)
            .map(|i| mention(AiMentionKind::Device, &i.to_string(), "d"))
            .collect();
        assert_eq!(sanitize(muitas).len(), MAX_MENTIONS);
    }
}
