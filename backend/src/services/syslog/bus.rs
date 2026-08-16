//! Barramento do *live tail* — **separado do `EventBus` de domínio**.
//!
//! Esta separação não é organização de código, é isolamento de falha. O
//! `EventBus` é um `broadcast` de capacidade 1024 compartilhado com o SSE do
//! dashboard (`services/events/bus.rs`). A 12 msg/s de log, esse anel rola
//! inteiro em ~85 segundos: qualquer cliente do `/api/events/stream` que
//! atrasasse um pouco receberia `RecvError::Lagged` e **perderia eventos de
//! domínio** — `monitor:result`, `alert:triggered`. O controller já trata isso
//! emitindo `stream:resync`, então o sintoma seria o painel inteiro
//! ressincronizando sem parar por causa de tráfego de log.
//!
//! Com anel próprio, log atrasado só atrapalha quem está olhando log.
//!
//! **Nunca passa pelo `event_outbox`.** Ingestão e SSE vivem no mesmo processo;
//! persistir cada linha uma segunda vez para retransmiti-la dobraria a escrita
//! do recurso de maior volume do sistema.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::dtos::logs::LogEntry;

/// Fôlego do anel do live tail.
///
/// Menor que os 1024 do barramento de domínio de propósito: aqui o atraso é
/// esperado (a tela pinta uma tabela) e a perda é aceitável — quem está com o
/// tail ligado quer ver o que está chegando **agora**, não recuperar o que
/// passou. Para isso existe a paginação.
const CAPACITY: usize = 512;

/// O barramento, guardado no `shared_store`.
#[derive(Clone)]
pub struct LogBus {
    sender: Arc<broadcast::Sender<LogEntry>>,
}

impl Default for LogBus {
    fn default() -> Self {
        Self::create()
    }
}

impl LogBus {
    #[must_use]
    pub fn create() -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self {
            sender: Arc::new(sender),
        }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.sender.subscribe()
    }

    /// Há alguém com o live tail aberto?
    ///
    /// O escritor consulta isto **antes** de serializar o lote: sem assinante,
    /// montar `LogEntry` para cada linha seria trabalho puro de aquecer o
    /// processador. No caso comum — ninguém olhando — o custo do live tail é
    /// uma leitura atômica por lote.
    #[must_use]
    pub fn has_subscribers(&self) -> bool {
        self.sender.receiver_count() > 0
    }

    /// Publica. Sem assinante, o `send` falha e o erro é o resultado esperado.
    pub fn publish(&self, entry: LogEntry) {
        let _ = self.sender.send(entry);
    }

    /// Publica um lote inteiro, uma vez só se ninguém estiver ouvindo.
    pub fn publish_batch(&self, entries: impl IntoIterator<Item = LogEntry>) {
        if !self.has_subscribers() {
            return;
        }
        for entry in entries {
            self.publish(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrada(id: i64) -> LogEntry {
        LogEntry {
            id,
            device_id: None,
            device_name: None,
            source_ip: "10.0.0.1".into(),
            received_at: "2026-08-15T12:00:00Z".into(),
            device_time: None,
            facility: None,
            severity: Some(6),
            severity_label: Some("informação".into()),
            hostname: None,
            app_name: None,
            pid: None,
            topics: Vec::new(),
            message: "linha".into(),
        }
    }

    #[test]
    fn sem_assinante_o_lote_nao_e_publicado() {
        let bus = LogBus::create();
        assert!(!bus.has_subscribers());
        // Não deve entrar em pânico nem custar nada.
        bus.publish_batch([entrada(1), entrada(2)]);
    }

    #[tokio::test]
    async fn o_assinante_recebe_o_que_foi_publicado() {
        let bus = LogBus::create();
        let mut receiver = bus.subscribe();
        assert!(bus.has_subscribers());

        bus.publish_batch([entrada(1), entrada(2)]);

        assert_eq!(receiver.recv().await.expect("primeira").id, 1);
        assert_eq!(receiver.recv().await.expect("segunda").id, 2);
    }

    #[tokio::test]
    async fn o_anel_do_log_nao_e_o_do_dominio() {
        // Regressão da decisão: encher o anel do log tem de atrasar **só** o
        // log. Aqui o assinante lento perde o começo e continua vivo.
        let bus = LogBus::create();
        let mut receiver = bus.subscribe();
        for id in 0..(CAPACITY as i64 + 10) {
            bus.publish(entrada(id));
        }
        match receiver.recv().await {
            Err(broadcast::error::RecvError::Lagged(perdidas)) => {
                assert!(perdidas >= 10, "perdeu {perdidas}");
            }
            outro => panic!("esperava Lagged, veio {outro:?}"),
        }
        // E segue entregando depois do atraso.
        assert!(receiver.recv().await.is_ok());
    }
}
