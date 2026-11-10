//! O que o deploy precisa do registry da central (ADR 022), como contrato.
//!
//! O registry mora num crate próprio da área (`servers-registry`), que
//! depende deste — então aqui só existe o trait; a implementação é injetada
//! pelo ponto de montagem no `shared_store` ([`ImageStores`]). O deploy guarda
//! a imagem que o agente compilou, entrega a do registry ao host de destino e
//! pergunta o que a varredura achou, sem saber como nada disso é feito.

use std::sync::Arc;

use async_trait::async_trait;
use loco_rs::app::AppContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use agent_protocol::tunnel::TunnelStream;
use netmonitor_core::shared::errors::{AppError, AppResult};

/// Um componente de nome do registry a partir de um texto livre:
/// minúsculas, números e `-` (`Loja Online` → `loja-online`).
#[must_use]
pub fn component(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// O repositório de uma imagem de aplicação no registry: `<slug>/<imagem>`.
#[must_use]
pub fn repository_for(slug: &str, image: &str) -> String {
    format!("{}/{}", component(slug), component(image))
}

/// Uma tag a partir de um ref (`feature/x` → `feature-x`).
#[must_use]
pub fn tag_for(reference: &str) -> Option<String> {
    let tag: String = reference
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .take(128)
        .collect();
    let tag = tag.trim_start_matches(['.', '-']).to_string();
    (!tag.is_empty()).then_some(tag)
}

/// Uma imagem guardada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployStoredImage"
)]
pub struct StoredImage {
    /// Nome curto (`api`).
    pub name: String,
    /// Repositório no registry (`loja/api`).
    pub repository: String,
    pub tag: String,
    pub digest: String,
    /// Manifest + config + camadas.
    #[ts(type = "number")]
    pub bytes: i64,
}

/// Como está a análise de uma imagem.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployScanStatus"
)]
pub enum ScanStatus {
    /// Ainda não analisada.
    #[default]
    Pending,
    Done,
    /// Sem lista de pacotes (nem SBOM nem banco de pacotes nas camadas).
    NoSbom,
    /// A base de vulnerabilidades não respondeu.
    Failed,
    /// A imagem não está no registry.
    Missing,
}

impl ScanStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Done => "done",
            Self::NoSbom => "no_sbom",
            Self::Failed => "failed",
            Self::Missing => "missing",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "done" => Self::Done,
            "no_sbom" => Self::NoSbom,
            "failed" => Self::Failed,
            "missing" => Self::Missing,
            _ => Self::Pending,
        }
    }

    /// A análise terminou (com ou sem achados).
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(self, Self::Done | Self::NoSbom)
    }
}

/// De onde veio uma imagem (o `source` da proveniência gravada).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployOriginKind"
)]
pub enum OriginKind {
    /// Compilada pela central, a partir do repositório da aplicação (ADR 023).
    Build,
    /// Enviada pelo workflow do repositório (modo GitHub Actions).
    Workflow,
    /// `docker push` de alguém da equipe.
    ManualPush,
}

impl OriginKind {
    /// A origem gravada numa proveniência.
    #[must_use]
    pub fn of(provenance: &Value) -> Option<Self> {
        serde_json::from_value(provenance.get("source")?.clone()).ok()
    }

    /// "Origem conhecida" para o alvo que exige proveniência (RF-28): veio
    /// do repositório da aplicação, nunca de um push manual.
    #[must_use]
    pub fn trusted_for(provenance: &Value, repository: &str) -> bool {
        matches!(Self::of(provenance), Some(Self::Build | Self::Workflow))
            && provenance
                .get("repository")
                .and_then(Value::as_str)
                .is_some_and(|repo| repo.eq_ignore_ascii_case(repository))
    }
}

/// O que a varredura achou numa imagem, para a política do alvo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployScanVerdict"
)]
pub struct ScanVerdict {
    pub status: ScanStatus,
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub unknown: u32,
    /// Achados cobertos por exceção válida (não contam acima).
    pub excepted: u32,
    /// Os mais graves, para a mensagem (`CVE-…` em `pacote`).
    pub top: Vec<String>,
}

/// Onde a política barra (`vulnerability_policy` do alvo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../frontend/src/bindings/",
    rename = "DeployVulnerabilityPolicy"
)]
pub enum VulnerabilityPolicy {
    /// Só avisa (padrão).
    Warn,
    BlockCritical,
    BlockHigh,
}

impl VulnerabilityPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::BlockCritical => "block_critical",
            Self::BlockHigh => "block_high",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "block_critical" => Self::BlockCritical,
            "block_high" => Self::BlockHigh,
            _ => Self::Warn,
        }
    }

    /// O motivo do bloqueio, quando a política barra este veredito.
    #[must_use]
    pub fn blocks(self, verdict: &ScanVerdict) -> Option<String> {
        let (count, level) = match self {
            Self::Warn => return None,
            Self::BlockCritical => (verdict.critical, "crítica"),
            Self::BlockHigh => (verdict.critical + verdict.high, "alta ou crítica"),
        };
        (count > 0).then(|| {
            let sample = verdict
                .top
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{count} vulnerabilidade(s) de severidade {level} na imagem ({sample}). \
                 Corrija a dependência ou registre uma exceção em Registry › Vulnerabilidades"
            )
        })
    }
}

/// De onde a imagem veio (gravado uma vez, junto do manifest).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageOrigin {
    pub source: OriginKind,
    pub repository: String,
    pub commit: String,
    pub reference: Option<String>,
    pub build_id: Option<i64>,
    pub agent: Option<String>,
    pub triggered_by: Option<String>,
    pub at: String,
}

impl ImageOrigin {
    #[must_use]
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// O registry, visto pelo deploy.
#[async_trait]
pub trait ImageStore: Send + Sync {
    /// Guarda o arquivo OCI que o agente compilou em `repository:tag`.
    ///
    /// # Errors
    ///
    /// Arquivo inválido, digest que não confere, cota estourada.
    async fn ingest(
        &self,
        ctx: &AppContext,
        repository: &str,
        application_id: i64,
        tags: &[String],
        archive: TunnelStream,
        origin: &ImageOrigin,
    ) -> AppResult<String>;

    /// A imagem guardada em `repository:tag`.
    ///
    /// # Errors
    ///
    /// Erro do banco.
    async fn find(
        &self,
        ctx: &AppContext,
        repository: &str,
        tag: &str,
    ) -> AppResult<Option<(String, i64)>>;

    /// Escreve em `sink` o tar que o `docker load` do host lê, com a imagem
    /// da arquitetura dele marcada como `reference`, e fecha a escrita.
    ///
    /// # Errors
    ///
    /// Imagem ausente, sem a arquitetura do host, ou ponte caída.
    async fn deliver(
        &self,
        ctx: &AppContext,
        repository: &str,
        tag: &str,
        architecture: &str,
        reference: &str,
        sink: &mut TunnelStream,
    ) -> AppResult<()>;

    /// Analisa a imagem agora (se ainda não foi) e devolve o veredito, com as
    /// exceções da aplicação já descontadas.
    ///
    /// # Errors
    ///
    /// Erro do banco.
    async fn verdict(
        &self,
        ctx: &AppContext,
        repository: &str,
        tag: &str,
        application_id: i64,
    ) -> AppResult<ScanVerdict>;

    /// A origem gravada da imagem (para "só origem conhecida").
    ///
    /// # Errors
    ///
    /// Erro do banco.
    async fn provenance(
        &self,
        ctx: &AppContext,
        repository: &str,
        tag: &str,
    ) -> AppResult<Option<Value>>;
}

/// O registry do processo, no `shared_store`.
#[derive(Clone)]
pub struct ImageStores(pub Arc<dyn ImageStore>);

/// O registry injetado pelo ponto de montagem.
///
/// # Errors
///
/// `ServiceUnavailable` quando o processo foi montado sem registry.
pub fn image_store(ctx: &AppContext) -> AppResult<Arc<dyn ImageStore>> {
    ctx.shared_store
        .get::<ImageStores>()
        .map(|stores| stores.0)
        .ok_or_else(|| {
            AppError::service_unavailable("O registry da central não está montado neste processo")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn politica_barra_so_o_que_deve() {
        let verdict = ScanVerdict {
            status: ScanStatus::Done,
            high: 2,
            top: vec!["CVE-1 em openssl".into()],
            ..ScanVerdict::default()
        };
        assert!(VulnerabilityPolicy::Warn.blocks(&verdict).is_none());
        assert!(VulnerabilityPolicy::BlockCritical
            .blocks(&verdict)
            .is_none());
        let reason = VulnerabilityPolicy::BlockHigh.blocks(&verdict).unwrap();
        assert!(reason.contains("CVE-1"));
        assert_eq!(
            VulnerabilityPolicy::parse("block_high"),
            VulnerabilityPolicy::BlockHigh
        );
        assert_eq!(VulnerabilityPolicy::parse("??"), VulnerabilityPolicy::Warn);
    }
}
