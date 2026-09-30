//! Categoria de cada padrão novo de log, em segundo plano.
//!
//! ```text
//! writer ── padrão inédito? ──try_send──► fila(256) ──► worker ──► Laya
//!   │ (nunca espera)                                     │
//!   └─ grava a linha com template_hash                   └─► log_templates
//! ```
//!
//! O caminho quente não muda: o escritor só calcula o padrão e, se for
//! inédito, tenta pôr na fila — cheia, descarta e tenta de novo da próxima
//! vez que o padrão aparecer. O worker grava o padrão **mesmo sem Laya**, para
//! a varredura periódica classificá-lo quando ele for ligado.
//!
//! Só linhas de syslog: os logs da própria aplicação também viram linhas
//! (`app_layer`), e classificar o aviso de que o Laya falhou geraria outro
//! aviso a classificar.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use chrono::Utc;
use loco_rs::app::AppContext;
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use tokio::sync::mpsc;

use super::{queue::PendingLog, template::normalize_message};
use crate::{
    models::logs::log_templates,
    services::{
        ai::laya::{
            config::LayaFeature,
            decisions::log_category::{LogCategory, LogPattern},
            runtime::{Lane, LayaRuntime},
            suggestion::LayaSuggestion,
        },
        events::EventBus,
        shared::text::truncate_chars,
    },
};

/// Padrões lembrados em memória antes de recomeçar do zero — um padrão
/// esquecido só custa uma consulta ao banco (o upsert não duplica).
const MAX_SEEN: usize = 50_000;
const QUEUE_CAPACITY: usize = 256;
const EXAMPLE_CHARS: usize = 500;
/// A cada minuto, classifica o que ficou sem categoria (Laya desligado ou
/// fora do ar quando o padrão apareceu).
const BACKFILL_EVERY: Duration = Duration::from_secs(60);
const BACKFILL_BATCH: u64 = 50;
pub const CLASSIFIED_EVENT: &str = "logs:template_classified";

/// Um padrão a gravar e classificar.
#[derive(Debug, Clone)]
pub struct TemplateJob {
    pub hash: i64,
    pub template: String,
    pub example: String,
    pub app_name: Option<String>,
    pub topics: Option<String>,
    pub severity: Option<i16>,
}

impl TemplateJob {
    fn pattern(&self) -> LogPattern<'_> {
        LogPattern {
            app: self.app_name.as_deref(),
            topics: self.topics.as_deref(),
            severity: self.severity,
            template: &self.template,
            example: &self.example,
        }
    }
}

/// Quem dá a categoria. O Laya em produção; um stub nos testes.
#[async_trait]
pub trait TemplateClassifier: Send + Sync {
    /// Vale a pena tentar agora? Falso com a frente desligada.
    async fn ready(&self) -> bool;
    async fn classify(&self, job: &TemplateJob) -> Option<LayaSuggestion>;
}

pub struct LayaTemplateClassifier {
    ctx: AppContext,
}

impl LayaTemplateClassifier {
    #[must_use]
    pub const fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl TemplateClassifier for LayaTemplateClassifier {
    async fn ready(&self) -> bool {
        LayaRuntime::from_context(&self.ctx)
            .settings_for(&self.ctx.db, LayaFeature::LogEvents)
            .await
            .is_some()
    }

    async fn classify(&self, job: &TemplateJob) -> Option<LayaSuggestion> {
        let runtime = LayaRuntime::from_context(&self.ctx);
        let settings = runtime
            .settings_for(&self.ctx.db, LayaFeature::LogEvents)
            .await?;
        let decision = LogCategory {
            min_confidence: settings.min_confidence,
        };
        let outcome = runtime
            .decide_with(
                &settings,
                Lane::Background,
                &job.pattern().state(),
                &decision,
            )
            .await?;
        outcome.value.map(|mut suggestion| {
            suggestion.model = outcome.model;
            suggestion
        })
    }
}

/// Os padrões já vistos e a fila para o worker. Vive com o escritor.
pub struct TemplateCatalog {
    seen: Mutex<HashSet<i64>>,
    jobs: mpsc::Sender<TemplateJob>,
}

impl TemplateCatalog {
    #[must_use]
    pub fn create() -> (Arc<Self>, mpsc::Receiver<TemplateJob>) {
        let (jobs, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let catalog = Self {
            seen: Mutex::new(HashSet::new()),
            jobs,
        };
        (Arc::new(catalog), receiver)
    }

    /// O escritor viu esta linha. Síncrono e sem espera: padrão inédito vai
    /// para a fila se couber; se não couber, é esquecido e volta na próxima.
    pub fn observe(&self, hash: i64, template: &str, log: &PendingLog) {
        let Ok(mut seen) = self.seen.lock() else {
            return;
        };
        if seen.contains(&hash) {
            return;
        }
        if seen.len() >= MAX_SEEN {
            seen.clear();
        }
        let job = TemplateJob {
            hash,
            template: template.to_string(),
            example: truncate_chars(&log.parsed.message, EXAMPLE_CHARS),
            app_name: log.parsed.app_name.clone(),
            topics: log.parsed.topics.clone(),
            severity: log.parsed.severity,
        };
        if self.jobs.try_send(job).is_ok() {
            seen.insert(hash);
        }
    }

    fn remember(&self, hashes: impl IntoIterator<Item = i64>) {
        if let Ok(mut seen) = self.seen.lock() {
            seen.extend(hashes);
        }
    }
}

/// O padrão e a chave de uma mensagem, como o escritor grava.
#[must_use]
pub fn template_of(message: &str) -> (String, i64) {
    let template = normalize_message(message);
    let hash = super::template::template_hash(&template);
    (template, hash)
}

/// Grava o padrão se ele ainda não existe. Nos dois bancos, `ON CONFLICT DO
/// NOTHING` sobre a chave primária.
async fn store_template(db: &DatabaseConnection, job: &TemplateJob) {
    let row = log_templates::ActiveModel {
        template_hash: Set(job.hash),
        template: Set(job.template.clone()),
        example: Set(job.example.clone()),
        app_name: Set(job.app_name.clone()),
        first_seen_at: Set(Utc::now().into()),
        ..Default::default()
    };
    let result = log_templates::Entity::insert(row)
        .on_conflict(
            OnConflict::column(log_templates::Column::TemplateHash)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await;
    if let Err(error) = result {
        tracing::debug!(%error, "log_templates: falha ao gravar padrão");
    }
}

/// Grava o palpite. A correção do operador (`user_category`) fica intocada.
async fn store_classification(
    db: &DatabaseConnection,
    hash: i64,
    suggestion: &LayaSuggestion,
    events: Option<&EventBus>,
) {
    #[allow(clippy::cast_possible_truncation)]
    let confidence = suggestion.confidence.round().clamp(0.0, 100.0) as i16;
    let result = log_templates::Entity::update_many()
        .col_expr(
            log_templates::Column::Category,
            Expr::value(suggestion.value.clone()),
        )
        .col_expr(log_templates::Column::Confidence, Expr::value(confidence))
        .col_expr(
            log_templates::Column::Model,
            Expr::value(suggestion.model.clone()),
        )
        .col_expr(
            log_templates::Column::ClassifiedAt,
            Expr::value(chrono::DateTime::<chrono::FixedOffset>::from(Utc::now())),
        )
        .filter(log_templates::Column::TemplateHash.eq(hash))
        .exec(db)
        .await;
    if let Err(error) = result {
        tracing::debug!(%error, "log_templates: falha ao gravar categoria");
        return;
    }
    // O evento leva o padrão como a tela o conhece (o mesmo da confirmação do
    // operador): um formato só, e a sugestão de alerta já vem junto.
    if let Some(events) = events {
        if let Ok(Some(row)) = log_templates::Entity::find_by_id(hash).one(db).await {
            events.publish_ephemeral(
                CLASSIFIED_EVENT,
                serde_json::to_value(super::templates::info(&row)).unwrap_or_default(),
            );
        }
    }
}

async fn handle(
    db: &DatabaseConnection,
    classifier: &dyn TemplateClassifier,
    events: Option<&EventBus>,
    job: &TemplateJob,
) {
    store_template(db, job).await;
    if let Some(suggestion) = classifier.classify(job).await {
        store_classification(db, job.hash, &suggestion, events).await;
    }
}

/// Classifica os padrões que ficaram sem categoria. Devolve quantos ganharam.
pub async fn backfill(
    db: &DatabaseConnection,
    classifier: &dyn TemplateClassifier,
    events: Option<&EventBus>,
    limit: u64,
) -> usize {
    if !classifier.ready().await {
        return 0;
    }
    let pending = log_templates::Entity::find()
        .filter(log_templates::Column::Category.is_null())
        .filter(log_templates::Column::UserCategory.is_null())
        .order_by_desc(log_templates::Column::FirstSeenAt)
        .limit(limit)
        .all(db)
        .await
        .unwrap_or_default();
    let mut classified = 0;
    for row in pending {
        let job = TemplateJob {
            hash: row.template_hash,
            template: row.template,
            example: row.example,
            app_name: row.app_name,
            topics: None,
            severity: None,
        };
        if let Some(suggestion) = classifier.classify(&job).await {
            store_classification(db, job.hash, &suggestion, events).await;
            classified += 1;
        }
    }
    classified
}

/// O worker: grava e classifica cada padrão novo; a cada minuto, recupera os
/// que ficaram para trás. Termina quando a fila fecha.
pub async fn run_worker(
    db: DatabaseConnection,
    catalog: Arc<TemplateCatalog>,
    mut jobs: mpsc::Receiver<TemplateJob>,
    classifier: Arc<dyn TemplateClassifier>,
    events: Option<EventBus>,
) {
    // Padrões já gravados não voltam para a fila depois de um restart.
    if let Ok(known) = log_templates::Entity::find()
        .select_only()
        .column(log_templates::Column::TemplateHash)
        .into_tuple::<i64>()
        .all(&db)
        .await
    {
        catalog.remember(known);
    }

    let mut tick = tokio::time::interval(BACKFILL_EVERY);
    tick.tick().await;
    loop {
        tokio::select! {
            job = jobs.recv() => match job {
                Some(job) => handle(&db, classifier.as_ref(), events.as_ref(), &job).await,
                None => return,
            },
            _ = tick.tick() => {
                backfill(&db, classifier.as_ref(), events.as_ref(), BACKFILL_BATCH).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use sea_orm::{ActiveModelTrait, PaginatorTrait};
    use serial_test::serial;

    use super::*;
    use crate::services::syslog::{db, parser::ParsedLog, queue::LogSource};

    /// Conta as consultas e responde `auth_failure` para tudo.
    struct Stub {
        calls: AtomicUsize,
        ready: bool,
    }

    #[async_trait]
    impl TemplateClassifier for Stub {
        async fn ready(&self) -> bool {
            self.ready
        }

        async fn classify(&self, _job: &TemplateJob) -> Option<LayaSuggestion> {
            if !self.ready {
                return None;
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            Some(LayaSuggestion {
                value: "auth_failure".into(),
                confidence: 91.0,
                model: "stub".into(),
            })
        }
    }

    fn pending(message: &str) -> PendingLog {
        PendingLog {
            device_id: Some(1),
            source_ip: "10.0.0.1".into(),
            received_at: Utc::now(),
            parsed: ParsedLog {
                message: message.into(),
                ..ParsedLog::default()
            },
            source: LogSource::Syslog,
        }
    }

    async fn banco() -> DatabaseConnection {
        std::env::remove_var("SYSLOG_DB_URL");
        db::connect("sqlite::memory:")
            .await
            .expect("banco de logs")
            .connection()
            .clone()
    }

    async fn drain(
        db: &DatabaseConnection,
        catalog: Arc<TemplateCatalog>,
        receiver: mpsc::Receiver<TemplateJob>,
        stub: Arc<Stub>,
    ) {
        // Fechar o remetente faz o worker processar a fila e sair.
        drop(catalog);
        run_worker(
            db.clone(),
            TemplateCatalog::create().0,
            receiver,
            stub,
            None,
        )
        .await;
    }

    #[tokio::test]
    #[serial]
    async fn um_padrao_uma_classificacao() {
        let db = banco().await;
        let (catalog, receiver) = TemplateCatalog::create();
        for n in 0..200 {
            let message = match n % 3 {
                0 => format!("login failure for user admin from 10.0.0.{n}"),
                1 => format!("ether{n} link down"),
                _ => format!("dhcp lease {n} renewed"),
            };
            let (template, hash) = template_of(&message);
            catalog.observe(hash, &template, &pending(&message));
        }
        let stub = Arc::new(Stub {
            calls: AtomicUsize::new(0),
            ready: true,
        });
        drain(&db, catalog, receiver, Arc::clone(&stub)).await;

        assert_eq!(
            stub.calls.load(Ordering::SeqCst),
            3,
            "uma consulta por padrão"
        );
        assert_eq!(log_templates::Entity::find().count(&db).await.unwrap(), 3);
        let row = log_templates::Entity::find()
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.category.as_deref(), Some("auth_failure"));
        assert_eq!(row.confidence, Some(91));
    }

    #[tokio::test]
    #[serial]
    async fn sem_laya_grava_o_padrao_e_a_varredura_classifica_depois() {
        let db = banco().await;
        let (catalog, receiver) = TemplateCatalog::create();
        let (template, hash) = template_of("ether3 link down");
        catalog.observe(hash, &template, &pending("ether3 link down"));
        let off = Arc::new(Stub {
            calls: AtomicUsize::new(0),
            ready: false,
        });
        drain(&db, catalog, receiver, off).await;

        let row = log_templates::Entity::find_by_id(hash)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.category, None, "gravado sem categoria");

        let on = Stub {
            calls: AtomicUsize::new(0),
            ready: true,
        };
        assert_eq!(backfill(&db, &on, None, 10).await, 1);
        let row = log_templates::Entity::find_by_id(hash)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.category.as_deref(), Some("auth_failure"));
    }

    #[tokio::test]
    #[serial]
    async fn correcao_do_operador_nao_e_sobrescrita_nem_reclassificada() {
        let db = banco().await;
        let (template, hash) = template_of("ether3 link down");
        log_templates::ActiveModel {
            template_hash: Set(hash),
            template: Set(template),
            example: Set("ether3 link down".into()),
            user_category: Set(Some("link_change".into())),
            first_seen_at: Set(Utc::now().into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let stub = Stub {
            calls: AtomicUsize::new(0),
            ready: true,
        };
        assert_eq!(
            backfill(&db, &stub, None, 10).await,
            0,
            "o operador já decidiu"
        );
        let row = log_templates::Entity::find_by_id(hash)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.effective_category(), Some("link_change"));
    }

    #[test]
    fn fila_cheia_nao_trava_o_escritor() {
        let (catalog, _receiver) = TemplateCatalog::create();
        for n in 0..(QUEUE_CAPACITY + 50) {
            // Só letras: dígito vira `#` e todos cairiam no mesmo padrão.
            let word: String = n
                .to_string()
                .bytes()
                .map(|d| char::from(d - b'0' + b'a'))
                .collect();
            let message = format!("evento {word} registrado");
            let (template, hash) = template_of(&message);
            catalog.observe(hash, &template, &pending(&message));
        }
        let seen = catalog.seen.lock().unwrap().len();
        assert_eq!(seen, QUEUE_CAPACITY, "o que não coube volta depois");
    }
}
