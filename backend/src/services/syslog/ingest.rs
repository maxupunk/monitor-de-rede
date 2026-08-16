//! O caminho de uma linha, do datagrama à fila. Compartilhado por UDP e TCP.
//!
//! A ordem das etapas é escolhida pelo custo: o que descarta mais barato vem
//! antes. Limitar por fonte custa uma consulta a um `HashMap`; parsear custa um
//! `nom`; resolver pode custar uma consulta ao banco. Inverter isso faria uma
//! fonte defeituosa pagar o preço mais caro justamente na hora em que ela mais
//! manda.

use std::{net::IpAddr, sync::Arc};

use chrono::Utc;
use sea_orm::DatabaseConnection;

use super::{
    config::SyslogConfig,
    matcher::PatternMatcher,
    parser,
    queue::{IngestMetrics, LogQueue, PendingLog, RateLimiter},
    resolver::Resolver,
    sources::SourceRegistry,
};

/// Tudo que o tratamento de uma linha precisa, montado uma vez por processo.
#[derive(Clone)]
pub struct Ingestor {
    /// Banco **principal** — é lá que moram `devices` e `networks`.
    inventory: DatabaseConnection,
    config: SyslogConfig,
    queue: LogQueue,
    resolver: Resolver,
    limiter: Arc<RateLimiter>,
    pub sources: Arc<SourceRegistry>,
    metrics: Arc<IngestMetrics>,
    matcher: Arc<PatternMatcher>,
}

impl Ingestor {
    #[must_use]
    pub fn new(
        inventory: DatabaseConnection,
        config: SyslogConfig,
        queue: LogQueue,
        sources: Arc<SourceRegistry>,
        matcher: Arc<PatternMatcher>,
    ) -> Self {
        let metrics = Arc::clone(queue.metrics());
        let limiter = Arc::new(RateLimiter::new(config.rate_limit_per_source));
        Self {
            inventory,
            config,
            queue,
            resolver: Resolver::create(),
            limiter,
            sources,
            metrics,
            matcher,
        }
    }

    #[must_use]
    pub fn metrics(&self) -> &Arc<IngestMetrics> {
        &self.metrics
    }

    #[must_use]
    pub fn resolver(&self) -> &Resolver {
        &self.resolver
    }

    #[must_use]
    pub const fn max_msg_bytes(&self) -> usize {
        self.config.max_msg_bytes
    }

    /// Trata uma linha recebida. Devolve `true` quando ela entrou na fila.
    ///
    /// Nunca falha: erro de banco na resolução vira aviso e a linha segue como
    /// fonte desconhecida. Derrubar o listener porque o inventário está
    /// indisponível calaria a ingestão inteira por um problema passageiro.
    pub async fn handle(&self, bruto: &[u8], origem: IpAddr) -> bool {
        self.metrics.record_received();
        let source_ip = origem.to_string();
        let masked = self.resolver.nat().is_masked(origem);

        // Atrás de NAT o limite por fonte precisa esperar o parse: a chave
        // deixa de ser o endereço — que é um só para o parque inteiro — e passa
        // a incluir o hostname. Sem isso os 50 msg/s de **um** roteador viram o
        // teto de todos, e trinta aparelhos dividiriam o orçamento de um.
        if !masked && !self.limiter.allow(&source_ip) {
            self.metrics.record_rate_limited();
            return false;
        }

        // Truncar, não descartar: uma linha de firewall gigante ainda diz o que
        // aconteceu, e o começo dela é a parte que importa.
        let bruto = if bruto.len() > self.config.max_msg_bytes {
            self.metrics.record_oversized();
            &bruto[..self.config.max_msg_bytes]
        } else {
            bruto
        };

        let recebido_em = Utc::now();
        let texto = String::from_utf8_lossy(bruto);
        let parsed = parser::parse(texto.trim(), recebido_em);

        if masked {
            let chave = match parsed.hostname.as_deref() {
                Some(nome) => format!("{source_ip}|{nome}"),
                None => source_ip.clone(),
            };
            if !self.limiter.allow(&chave) {
                self.metrics.record_rate_limited();
                return false;
            }
        }

        let resolucao = match self
            .resolver
            .resolve(&self.inventory, origem, parsed.hostname.as_deref())
            .await
        {
            Ok(resolucao) => resolucao,
            Err(error) => {
                tracing::warn!(%error, %source_ip, "falha ao resolver a origem do log");
                super::resolver::Resolution::Unknown
            }
        };

        self.sources.record(
            &source_ip,
            masked,
            &resolucao,
            parsed.hostname.as_deref(),
            &parsed.message,
            recebido_em,
        );

        if !resolucao.is_known() && !self.config.accept_unknown_sources {
            self.metrics.record_unknown_source();
            return false;
        }

        // O casamento de padrão acontece **aqui**, na ingestão, e não por
        // consulta: regex sobre a janela não usa índice nenhum. Sai de graça
        // quando não há regra de log cadastrada. Ver `matcher`.
        self.matcher
            .record(resolucao.device_id(), &parsed, recebido_em);

        self.queue.try_enqueue(PendingLog {
            device_id: resolucao.device_id(),
            source_ip,
            received_at: recebido_em,
            parsed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::syslog::sources::SourceKind;
    use migration::{Migrator, MigratorTrait};
    use serial_test::serial;

    /// Banco principal vazio em memória: sem `devices` e sem `networks`, toda
    /// origem é desconhecida — que é justamente o cenário a exercitar.
    async fn inventario_vazio() -> DatabaseConnection {
        let db = sea_orm::Database::connect(
            sea_orm::ConnectOptions::new("sqlite::memory:".to_owned())
                .max_connections(1)
                .min_connections(1)
                .to_owned(),
        )
        .await
        .expect("banco principal");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    fn ingestor(inventario: DatabaseConnection, config: SyslogConfig) -> (Ingestor, LogQueue) {
        let metrics = Arc::new(IngestMetrics::default());
        let (queue, receiver) = LogQueue::create(64, metrics);
        // O receptor precisa ficar vivo, senão a fila fecha e nada entra.
        std::mem::forget(receiver);
        let sources = Arc::new(SourceRegistry::create());
        (
            Ingestor::new(
                inventario,
                config,
                queue.clone(),
                sources,
                Arc::new(PatternMatcher::create()),
            ),
            queue,
        )
    }

    #[tokio::test]
    #[serial]
    async fn fonte_desconhecida_nao_grava_mas_vira_contador() {
        std::env::remove_var("SYSLOG_DB_URL");
        let (ingestor, _queue) = ingestor(inventario_vazio().await, SyslogConfig::default());
        let origem: IpAddr = "203.0.113.9".parse().unwrap();

        let entrou = ingestor
            .handle(b"<134>Aug 15 10:23:45 host app: teste", origem)
            .await;

        assert!(!entrou, "host solto não pode encher o disco");
        let metrics = ingestor.metrics().snapshot();
        assert_eq!(metrics.received, 1);
        assert_eq!(metrics.dropped_unknown_source, 1);
        assert_eq!(metrics.queued, 0);
        // E aparece na lista, que é o que dá ao usuário onde olhar.
        let fontes = ingestor.sources.list();
        assert_eq!(fontes.len(), 1);
        assert_eq!(fontes[0].kind, SourceKind::Unknown);
        assert_eq!(fontes[0].dropped_count, 1);
    }

    #[tokio::test]
    #[serial]
    async fn a_chave_de_escape_aceita_fonte_desconhecida() {
        std::env::remove_var("SYSLOG_DB_URL");
        let config = SyslogConfig {
            accept_unknown_sources: true,
            ..SyslogConfig::default()
        };
        let (ingestor, _queue) = ingestor(inventario_vazio().await, config);
        let origem: IpAddr = "203.0.113.9".parse().unwrap();

        assert!(ingestor.handle(b"<134>host app: teste", origem).await);
        assert_eq!(ingestor.metrics().snapshot().queued, 1);
    }

    #[tokio::test]
    #[serial]
    async fn o_limite_por_fonte_corta_antes_de_parsear() {
        std::env::remove_var("SYSLOG_DB_URL");
        let config = SyslogConfig {
            rate_limit_per_source: 1,
            accept_unknown_sources: true,
            ..SyslogConfig::default()
        };
        let (ingestor, _queue) = ingestor(inventario_vazio().await, config);
        let origem: IpAddr = "203.0.113.9".parse().unwrap();

        // Capacidade da rajada é 1 × 4 = 4.
        let mut aceitas = 0;
        for _ in 0..20 {
            if ingestor.handle(b"<134>host app: teste", origem).await {
                aceitas += 1;
            }
        }
        assert_eq!(aceitas, 4, "a rajada é o teto, não o limite sustentado");
        assert_eq!(ingestor.metrics().snapshot().dropped_rate_limit, 16);
    }

    #[tokio::test]
    #[serial]
    async fn linha_gigante_e_truncada_e_nao_descartada() {
        std::env::remove_var("SYSLOG_DB_URL");
        let config = SyslogConfig {
            max_msg_bytes: 64,
            accept_unknown_sources: true,
            ..SyslogConfig::default()
        };
        let (ingestor, _queue) = ingestor(inventario_vazio().await, config);
        let origem: IpAddr = "203.0.113.9".parse().unwrap();

        let mut bruto = b"<134>Aug 15 10:23:45 host app: ".to_vec();
        bruto.extend(std::iter::repeat_n(b'x', 5000));

        assert!(
            ingestor.handle(&bruto, origem).await,
            "truncada, não perdida"
        );
        let metrics = ingestor.metrics().snapshot();
        assert_eq!(metrics.dropped_oversized, 1, "o corte tem de ser visível");
        assert_eq!(metrics.queued, 1);
    }
}
