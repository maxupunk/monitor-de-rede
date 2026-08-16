//! Servidor de syslog nativo — recebe log de roteador e vincula ao inventário.
//!
//! ```text
//! udp/tcp:5514 → parser → resolvedor → fila(10k) → escritor em lote
//!                            │                          ↓
//!                       fontes vistas              logs.sqlite
//! ```
//!
//! Roteiro completo em `docs/roadmap_syslog_nativo.md`; as decisões medidas, na
//! [ADR 008](../../../../docs/adr/008-syslog-parser.md).
//!
//! **Onde cada peça é registrada, e por quê.** O `run_task` do Loco não executa
//! `Initializer` (ADR 007): a conexão com o banco de logs vai em
//! `Hooks::after_context`, porque a purga de retenção roda no ciclo do
//! `scheduler`, que pode ser invocado como `task`. Os listeners vão num
//! `Initializer`, porque abrir porta é coisa de servidor — um comando de CLI
//! não deve fazê-lo.

pub mod bus;
pub mod config;
pub mod db;
pub mod hints;
pub mod ingest;
pub mod listener;
pub mod mactelnet;
pub mod matcher;
pub mod nat;
pub mod parser;
pub mod provision;
pub mod queue;
pub mod repository;
pub mod resolver;
pub mod retention;
pub mod search;
pub mod snippets;
pub mod sources;
pub mod writer;

use std::sync::Arc;

use loco_rs::app::AppContext;

pub use bus::LogBus;
pub use config::SyslogConfig;
pub use db::LogsDb;
pub use ingest::Ingestor;
pub use matcher::PatternMatcher;
pub use nat::NatDetector;
pub use queue::{IngestMetrics, IngestSnapshot, LogQueue};
pub use sources::SourceRegistry;

use crate::services::shared::errors::AppResult;

/// O serviço montado, guardado no `shared_store` para a API alcançar.
#[derive(Clone)]
pub struct SyslogService {
    pub ingestor: Ingestor,
    pub sources: Arc<SourceRegistry>,
    /// Barramento do live tail. Separado do `EventBus` de dominio de proposito
    /// -- ver `bus`.
    pub bus: LogBus,
    /// Casamento de padrões de log contra as regras de alerta (Fase 6).
    pub matcher: Arc<PatternMatcher>,
}

impl SyslogService {
    /// Recupera o serviço do contexto, se a ingestão estiver de pé.
    #[must_use]
    pub fn from_context(ctx: &AppContext) -> Option<Self> {
        ctx.shared_store.get::<Self>()
    }
}

/// Monta o pipeline — fila, escritor, barramento — **sem abrir porta**.
///
/// Separado de [`spawn_listeners`] porque as duas coisas têm pré-requisitos
/// diferentes: a API de logs (`/sources`, `/stream`) precisa do serviço, não de
/// socket. Juntando os dois, o ambiente de teste — que não pode bindar porta —
/// ficaria sem serviço nenhum e os endpoints responderiam 400; e, em produção,
/// uma porta ocupada derrubaria junto a tela de diagnóstico, que é justamente
/// onde o operador iria procurar o motivo.
///
/// # Errors
///
/// Banco de logs indisponível.
pub fn build(ctx: &AppContext, config: &SyslogConfig) -> AppResult<SyslogService> {
    let logs = LogsDb::from_context(ctx)?;
    let metrics = Arc::new(IngestMetrics::default());
    let (queue, receiver) = LogQueue::create(config.queue_capacity, metrics);
    let sources = Arc::new(SourceRegistry::create());
    let bus = LogBus::create();
    let matcher = Arc::new(PatternMatcher::create());

    let ingestor = Ingestor::new(
        ctx.db.clone(),
        config.clone(),
        queue.clone(),
        Arc::clone(&sources),
        Arc::clone(&matcher),
    );

    tokio::spawn(writer::run(
        logs.connection().clone(),
        receiver,
        Arc::clone(queue.metrics()),
        bus.clone(),
    ));

    let servico = SyslogService {
        ingestor,
        sources,
        bus,
        matcher,
    };
    ctx.shared_store.insert(servico.clone());
    Ok(servico)
}

/// Abre os sockets. Devolve as portas efetivas — no teste elas vêm de `porta 0`,
/// e é assim que dois testes de rede não colidem.
///
/// # Errors
///
/// Falha ao abrir um dos sockets.
pub async fn spawn_listeners(
    servico: &SyslogService,
    config: &SyslogConfig,
) -> AppResult<(u16, u16)> {
    let udp = listener::spawn_udp(config.udp_port, servico.ingestor.clone()).await?;
    let tcp = listener::spawn_tcp(config.tcp_port, servico.ingestor.clone()).await?;
    Ok((udp, tcp))
}
