//! As vulnerabilidades das imagens viram alerta em `/alerts`.
//!
//! A varredura (ADR 022 §8) já sabe o que está aberto em cada versão; aqui
//! esse resultado vira **fato** para o motor de alertas, que decide pelas
//! regras se abre, notifica e fecha — com reconhecimento, silêncio, canais de
//! notificação e resumo da IA de graça, como qualquer outro alerta. Nenhum
//! `alert_events` é escrito aqui.
//!
//! O alvo é a **versão mais recente** de cada repositório (a tag empurrada por
//! último): é a que um deploy publicaria e a que a tela do registry mostra.
//! Versões antigas não alertam — corrigir é publicar uma versão nova. As
//! exceções aceitas descontam como no veredito do deploy
//! ([`super::open_findings`]), e cada severidade (crítica, alta) tem o próprio
//! alerta: as críticas corrigidas fecham o alerta crítico mesmo com altas
//! ainda abertas.
//!
//! Reavaliado quando a imagem é analisada, quando a base do OSV muda ou passa
//! a rotina diária (exceção vencida volta a contar), quando uma exceção é
//! criada ou removida, quando uma versão sai e quando o ajuste muda.

use std::collections::HashSet;

use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Map, Value};

use netmonitor_core::{
    alerts::{
        contracts::{
            AlertDataset, AlertEvaluationContext, AlertEvaluationScope, AlertFacts, AlertScopeKey,
            AlertStatus, AlertTarget,
        },
        fields,
    },
    models::{_entities::alert_events, registry_findings, registry_repositories, registry_tags},
    shared::errors::AppResult,
};
use netmonitor_servers::{
    central::Central,
    deploys::{images::ScanStatus, short_digest},
};

use super::{advisory::Severity, finding_label, open_findings};
use crate::{
    catalog,
    settings::{self, VulnerabilityAlerts},
};

/// `eventType` gravado em `alert_events.data` — a tela o usa para levar ao
/// registry.
pub const EVENT_TYPE: &str = "image_vulnerabilities";

/// Quantos achados a mensagem cita antes do "e mais N".
const CITED: usize = 3;

/// Prefixo das chaves de escopo ([`AlertScopeKey::registry_image`]).
const SCOPE_PREFIX: &str = "registry_image:";

/// A severidade que tem alerta próprio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Critical,
    High,
}

impl Level {
    pub const ALL: [Self; 2] = [Self::Critical, Self::High];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
        }
    }

    const fn field(self) -> &'static str {
        match self {
            Self::Critical => fields::IMAGE_VULN_CRITICAL,
            Self::High => fields::IMAGE_VULN_HIGH,
        }
    }

    const fn severity(self) -> Severity {
        match self {
            Self::Critical => Severity::Critical,
            Self::High => Severity::High,
        }
    }

    /// "1 vulnerabilidade crítica", "12 vulnerabilidades altas".
    fn count_noun(self, count: usize) -> String {
        let (one, many) = match self {
            Self::Critical => ("crítica", "críticas"),
            Self::High => ("alta", "altas"),
        };
        if count == 1 {
            format!("1 vulnerabilidade {one}")
        } else {
            format!("{count} vulnerabilidades {many}")
        }
    }

    fn reported_by(self, setting: VulnerabilityAlerts) -> bool {
        setting.reports(self == Self::Critical)
    }
}

/// O que se sabe de uma severidade na versão mais recente de uma imagem.
#[derive(Debug, Clone)]
pub struct ImageVulnerabilities {
    pub level: Level,
    /// `repositório:tag`.
    pub image: String,
    pub count: usize,
    /// Quantas já têm versão corrigida do pacote.
    pub fixable: usize,
    /// Os primeiros achados, para a mensagem (`CVE-… em pacote`).
    pub cited: Vec<String>,
}

impl ImageVulnerabilities {
    /// Monta a partir dos achados abertos da versão (já sem as exceções).
    #[must_use]
    pub fn of(level: Level, image: String, open: &[registry_findings::Model]) -> Self {
        let mine: Vec<&registry_findings::Model> = open
            .iter()
            .filter(|finding| Severity::parse(&finding.severity) == level.severity())
            .collect();
        Self {
            level,
            image,
            count: mine.len(),
            fixable: mine
                .iter()
                .filter(|finding| finding.fixed.is_some())
                .count(),
            cited: mine
                .iter()
                .take(CITED)
                .map(|finding| finding_label(finding))
                .collect(),
        }
    }

    /// Nada a alertar nesta severidade (versão limpa, ou ajuste que não a
    /// acompanha).
    #[must_use]
    pub fn clear(level: Level, image: String) -> Self {
        Self {
            level,
            image,
            count: 0,
            fixable: 0,
            cited: Vec::new(),
        }
    }
}

impl AlertDataset for ImageVulnerabilities {
    fn event_type(&self) -> &'static str {
        EVENT_TYPE
    }

    fn facts(&self) -> AlertFacts {
        let mut facts = Map::new();
        facts.insert(self.level.field().into(), json!(self.count));
        facts
    }

    fn describe(&self) -> String {
        if self.count == 0 {
            return format!(
                "{}: nenhuma vulnerabilidade {} em aberto",
                self.image,
                match self.level {
                    Level::Critical => "crítica",
                    Level::High => "alta",
                }
            );
        }
        let mut text = format!(
            "{}: {} — {}",
            self.image,
            self.level.count_noun(self.count),
            self.cited.join(", ")
        );
        let rest = self.count.saturating_sub(self.cited.len());
        if rest > 0 {
            text.push_str(&format!(" e mais {rest}"));
        }
        text.push('.');
        if self.fixable > 0 {
            text.push_str(&format!(
                " {} já {} versão corrigida do pacote: recompile a imagem (sem cache) e publique.",
                self.fixable,
                if self.fixable == 1 { "tem" } else { "têm" }
            ));
        } else {
            text.push_str(
                " Ainda sem versão corrigida: acompanhe a base ou aceite o risco com uma exceção.",
            );
        }
        text
    }

    fn is_recovery(&self) -> bool {
        self.count == 0
    }

    /// A contagem muda enquanto o alerta está aberto (uma parte corrigida, a
    /// base nova): a mensagem acompanha.
    fn refreshes_open_alert(&self) -> bool {
        true
    }

    fn source_label(&self) -> Option<&'static str> {
        Some("Registry · vulnerabilidades")
    }

    fn closes_when(&self) -> Option<&'static str> {
        Some(
            "Fecha sozinho quando uma versão corrigida da imagem é publicada ou o risco é aceito com uma exceção no Registry — não precisa encerrar à mão.",
        )
    }
}

/// O alvo de um repositório no motor de alertas.
fn target(
    repository: &registry_repositories::Model,
    level: Level,
    tag: Option<&str>,
    digest: Option<&str>,
) -> AlertTarget {
    let mut data = Map::new();
    data.insert("repositoryId".into(), json!(repository.id));
    data.insert("repository".into(), json!(repository.name));
    data.insert("severityLevel".into(), json!(level.as_str()));
    if let Some(id) = repository.application_id {
        data.insert("applicationId".into(), json!(id));
    }
    if let Some(tag) = tag {
        data.insert("tag".into(), json!(tag));
    }
    if let Some(digest) = digest {
        data.insert("digest".into(), json!(digest));
    }
    AlertTarget {
        scope: AlertEvaluationScope::default(),
        scope_key: AlertScopeKey::registry_image(repository.id, level.as_str()),
        label: repository.name.clone(),
        data,
    }
}

/// O que a versão mais recente do repositório tem em aberto, por severidade.
/// `None` quando ainda não há o que dizer (análise pendente ou falha da base):
/// o alerta fica como está, nem abre nem fecha por um resultado incompleto.
async fn assess(
    ctx: &AppContext,
    repository: &registry_repositories::Model,
    setting: VulnerabilityAlerts,
) -> AppResult<Option<Vec<(ImageVulnerabilities, AlertTarget)>>> {
    let latest = registry_tags::Entity::find()
        .filter(registry_tags::Column::RepositoryId.eq(repository.id))
        .order_by_desc(registry_tags::Column::UpdatedAt)
        .one(&ctx.db)
        .await?;
    let manifest = match &latest {
        Some(tag) => {
            catalog::manifest_by_digest(&ctx.db, repository.id, &tag.manifest_digest).await?
        }
        None => None,
    };
    let image = match &latest {
        Some(tag) => format!("{}:{}", repository.name, tag.name),
        None => repository.name.clone(),
    };
    let tag = latest.as_ref().map(|tag| tag.name.as_str());
    let digest = manifest
        .as_ref()
        .map(|row| short_digest(&row.digest).to_string());

    let open = match &manifest {
        Some(row) => match row.scan_status.as_deref().map(ScanStatus::parse) {
            Some(ScanStatus::Done) => {
                open_findings(&ctx.db, row, repository.application_id)
                    .await?
                    .0
            }
            // Sem pacotes reconhecidos não há vulnerabilidade conhecida.
            Some(ScanStatus::NoSbom) => Vec::new(),
            _ => return Ok(None),
        },
        // Repositório sem versão: nada a vigiar.
        None => Vec::new(),
    };

    Ok(Some(
        Level::ALL
            .into_iter()
            .map(|level| {
                let dataset = if level.reported_by(setting) {
                    ImageVulnerabilities::of(level, image.clone(), &open)
                } else {
                    ImageVulnerabilities::clear(level, image.clone())
                };
                (dataset, target(repository, level, tag, digest.as_deref()))
            })
            .collect(),
    ))
}

/// Entrega os fatos ao motor, criando as regras na primeira vulnerabilidade.
async fn deliver(
    ctx: &AppContext,
    central: &Central,
    assessed: Vec<(ImageVulnerabilities, AlertTarget)>,
) -> AppResult<()> {
    if assessed.iter().any(|(dataset, _)| dataset.count > 0) {
        central.ensure_image_vulnerability_rules(&ctx.db).await?;
    }
    for (dataset, target) in assessed {
        central
            .evaluate_alert(ctx, &AlertEvaluationContext::of(&dataset, target))
            .await?;
    }
    Ok(())
}

async fn evaluate_one(
    ctx: &AppContext,
    central: &Central,
    setting: VulnerabilityAlerts,
    repository: &registry_repositories::Model,
) -> AppResult<()> {
    if let Some(assessed) = assess(ctx, repository, setting).await? {
        deliver(ctx, central, assessed).await?;
    }
    Ok(())
}

/// Fecha os alertas de repositórios que já não existem (apagados pela
/// limpeza ou à mão).
async fn close_orphans(
    ctx: &AppContext,
    central: &Central,
    existing: &HashSet<i64>,
) -> AppResult<()> {
    let open = alert_events::Entity::find()
        .filter(alert_events::Column::ScopeKey.starts_with(SCOPE_PREFIX))
        .filter(alert_events::Column::Status.is_in(AlertStatus::OPEN))
        .all(&ctx.db)
        .await?;
    let mut closed = HashSet::new();
    for event in open {
        let Some((repository_id, level)) = event.scope_key.as_deref().and_then(parse_scope) else {
            continue;
        };
        if existing.contains(&repository_id) || !closed.insert((repository_id, level.as_str())) {
            continue;
        }
        let name = event
            .data
            .as_ref()
            .and_then(|data| data.get("repository"))
            .and_then(Value::as_str)
            .unwrap_or("imagem removida")
            .to_string();
        let gone = registry_repositories::Model {
            id: repository_id,
            name: name.clone(),
            application_id: None,
            created_at: event.created_at,
            updated_at: event.updated_at,
        };
        let dataset = ImageVulnerabilities::clear(level, name);
        central
            .evaluate_alert(
                ctx,
                &AlertEvaluationContext::of(&dataset, target(&gone, level, None, None)),
            )
            .await?;
    }
    Ok(())
}

/// `registry_image:<id>:<severidade>` de volta em partes.
fn parse_scope(key: &str) -> Option<(i64, Level)> {
    let (id, level) = key.strip_prefix(SCOPE_PREFIX)?.split_once(':')?;
    let level = Level::ALL
        .into_iter()
        .find(|candidate| candidate.as_str() == level)?;
    Some((id.parse().ok()?, level))
}

/// Os serviços da central, quando o ponto de montagem os registrou.
fn central(ctx: &AppContext) -> Option<Central> {
    match Central::from_context(ctx) {
        Ok(central) => Some(central),
        Err(error) => {
            tracing::debug!(%error, "alertas de vulnerabilidade sem o motor de alertas");
            None
        }
    }
}

/// Reavalia um repositório (a imagem dele acabou de ser analisada). Nunca
/// falha: um alerta que não abriu não pode derrubar a varredura.
pub async fn evaluate_repository(ctx: &AppContext, repository_id: i64) {
    let Some(central) = central(ctx) else {
        return;
    };
    let outcome = async {
        let setting = settings::load(&ctx.db).await?.vulnerability_alerts;
        if let Some(repository) = registry_repositories::Entity::find_by_id(repository_id)
            .one(&ctx.db)
            .await?
        {
            evaluate_one(ctx, &central, setting, &repository).await?;
        }
        AppResult::Ok(())
    }
    .await;
    if let Err(error) = outcome {
        tracing::warn!(repository_id, %error, "alerta de vulnerabilidade não reavaliado");
    }
}

/// Reavalia todos os repositórios e fecha os alertas dos que sumiram. Nunca
/// falha (ver [`evaluate_repository`]).
pub async fn evaluate_all(ctx: &AppContext) {
    let Some(central) = central(ctx) else {
        return;
    };
    let outcome = async {
        let setting = settings::load(&ctx.db).await?.vulnerability_alerts;
        let repositories = registry_repositories::Entity::find().all(&ctx.db).await?;
        for repository in &repositories {
            evaluate_one(ctx, &central, setting, repository).await?;
        }
        let existing = repositories.iter().map(|repo| repo.id).collect();
        close_orphans(ctx, &central, &existing).await
    }
    .await;
    if let Err(error) = outcome {
        tracing::warn!(%error, "alertas de vulnerabilidade não reavaliados");
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn finding(advisory: &str, severity: &str, fixed: Option<&str>) -> registry_findings::Model {
        registry_findings::Model {
            id: 0,
            manifest_id: 1,
            advisory: advisory.into(),
            aliases: json!([]),
            package: "openssl".into(),
            ecosystem: "Debian:12".into(),
            installed: "3.0.1".into(),
            fixed: fixed.map(ToString::to_string),
            severity: severity.into(),
            summary: None,
            created_at: Utc::now().into(),
        }
    }

    #[test]
    fn cada_severidade_conta_so_as_suas_e_diz_o_que_fazer() {
        let open = vec![
            finding("CVE-1", "high", Some("3.0.2")),
            finding("CVE-2", "high", None),
            finding("CVE-3", "high", Some("3.0.2")),
            finding("CVE-4", "high", None),
            finding("CVE-9", "critical", None),
            finding("CVE-5", "medium", None),
        ];
        let high = ImageVulnerabilities::of(Level::High, "api:bfcbff4".into(), &open);
        assert_eq!(high.count, 4);
        assert_eq!(high.fixable, 2);
        assert_eq!(high.facts()[fields::IMAGE_VULN_HIGH], json!(4));
        assert!(high.facts().get(fields::IMAGE_VULN_CRITICAL).is_none());
        assert!(!high.is_recovery());
        assert_eq!(
            high.describe(),
            "api:bfcbff4: 4 vulnerabilidades altas — CVE-1 em openssl, CVE-2 em openssl, \
             CVE-3 em openssl e mais 1. 2 já têm versão corrigida do pacote: recompile a \
             imagem (sem cache) e publique."
        );

        let critical = ImageVulnerabilities::of(Level::Critical, "api:bfcbff4".into(), &open);
        assert_eq!(critical.count, 1);
        assert!(critical
            .describe()
            .contains("1 vulnerabilidade crítica — CVE-9 em openssl."));
        assert!(critical.describe().contains("Ainda sem versão corrigida"));
    }

    #[test]
    fn versao_limpa_e_recuperacao() {
        let clear = ImageVulnerabilities::clear(Level::Critical, "api:v2".into());
        assert!(clear.is_recovery());
        assert_eq!(clear.facts()[fields::IMAGE_VULN_CRITICAL], json!(0));
        assert_eq!(
            clear.describe(),
            "api:v2: nenhuma vulnerabilidade crítica em aberto"
        );
    }

    #[test]
    fn o_ajuste_decide_quais_severidades_alertam() {
        assert!(Level::Critical.reported_by(VulnerabilityAlerts::Critical));
        assert!(!Level::High.reported_by(VulnerabilityAlerts::Critical));
        assert!(Level::High.reported_by(VulnerabilityAlerts::High));
        assert!(!Level::Critical.reported_by(VulnerabilityAlerts::Off));
    }

    #[test]
    fn a_chave_de_escopo_volta_em_partes() {
        let key = AlertScopeKey::registry_image(42, Level::High.as_str());
        assert_eq!(parse_scope(&key), Some((42, Level::High)));
        assert_eq!(parse_scope("registry_image:x:high"), None);
        assert_eq!(parse_scope("registry_image:4:medium"), None);
        assert_eq!(parse_scope("monitor:4"), None);
    }

    #[test]
    fn o_alvo_leva_o_que_a_tela_precisa_para_chegar_a_imagem() {
        let repo = registry_repositories::Model {
            id: 7,
            name: "vx-academia/vx-academia-api".into(),
            application_id: Some(3),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        };
        let target = target(
            &repo,
            Level::High,
            Some("bfcbff4"),
            Some("sha256:f273689b5669"),
        );
        assert_eq!(target.scope_key, "registry_image:7:high");
        assert_eq!(target.label, "vx-academia/vx-academia-api");
        assert_eq!(target.data["repositoryId"], json!(7));
        assert_eq!(target.data["applicationId"], json!(3));
        assert_eq!(target.data["tag"], json!("bfcbff4"));
        assert_eq!(target.data["severityLevel"], json!("high"));
    }
}
