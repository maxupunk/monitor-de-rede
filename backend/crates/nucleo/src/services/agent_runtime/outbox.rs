//! Buffer offline do agente: eventos que não puderam ser entregues.
//!
//! Rollups de minuto e resultados de monitor produzidos com o canal fora do
//! ar esperam aqui e saem na próxima conexão. Persistido em arquivo para
//! sobreviver a um reinício do agente, com teto de itens — descartando os
//! mais antigos — para não encher o disco do servidor monitorado.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::services::agents::protocol::AgentEvent;

/// Um evento à espera, com a identidade que o confirma ([`Outbox::ack`]).
struct Entry {
    seq: u64,
    event: AgentEvent,
}

struct Queue {
    items: VecDeque<Entry>,
    next_seq: u64,
    /// Sobe a cada alteração; o disco guarda a versão em `OutboxState::io`.
    version: u64,
}

impl Queue {
    fn next_entry(&mut self, event: AgentEvent) -> Entry {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        Entry { seq, event }
    }
}

struct OutboxState {
    path: Option<PathBuf>,
    max_items: usize,
    queue: Mutex<Queue>,
    /// Serializa a gravação e guarda a versão que já está no disco: quem
    /// chega com uma versão já gravada (por uma gravação concorrente que viu
    /// a sua alteração) não grava de novo.
    io: Mutex<u64>,
}

pub struct Outbox {
    state: Arc<OutboxState>,
}

impl OutboxState {
    fn lock_queue(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn trim(&self, queue: &mut Queue) -> bool {
        let mut trimmed = false;
        while queue.items.len() > self.max_items {
            queue.items.pop_front();
            trimmed = true;
        }
        trimmed
    }

    /// Altera a fila; devolve a versão que precisa chegar ao disco, se mudou.
    fn mutate<T>(&self, change: impl FnOnce(&mut Queue) -> (T, bool)) -> (T, Option<u64>) {
        let mut queue = self.lock_queue();
        let (value, changed) = change(&mut queue);
        if !changed {
            return (value, None);
        }
        queue.version = queue.version.wrapping_add(1);
        (value, Some(queue.version))
    }

    /// Grava ao menos até a versão `target` (bloqueante).
    fn persist_upto(&self, path: &Path, target: u64) {
        let mut persisted = self
            .io
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *persisted >= target {
            return;
        }
        let (version, snapshot) = {
            let queue = self.lock_queue();
            let snapshot: Vec<AgentEvent> = queue
                .items
                .iter()
                .map(|entry| entry.event.clone())
                .collect();
            (queue.version, snapshot)
        };
        let Ok(text) = serde_json::to_string(&snapshot) else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let temporary = path.with_extension("tmp");
        if std::fs::write(&temporary, text).is_ok() && std::fs::rename(&temporary, path).is_ok() {
            *persisted = version;
        }
    }
}

impl Outbox {
    fn with_items(path: Option<PathBuf>, max_items: usize, events: VecDeque<AgentEvent>) -> Self {
        let mut queue = Queue {
            items: VecDeque::with_capacity(events.len()),
            next_seq: 0,
            version: 0,
        };
        for event in events {
            let entry = queue.next_entry(event);
            queue.items.push_back(entry);
        }
        Self {
            state: Arc::new(OutboxState {
                path,
                max_items: max_items.max(1),
                queue: Mutex::new(queue),
                io: Mutex::new(0),
            }),
        }
    }

    /// Em memória, sem arquivo (testes).
    #[must_use]
    pub fn in_memory(max_items: usize) -> Self {
        Self::with_items(None, max_items, VecDeque::new())
    }

    /// Abre o arquivo existente; conteúdo ilegível começa vazio.
    #[must_use]
    pub fn open(path: PathBuf, max_items: usize) -> Self {
        let events = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<VecDeque<AgentEvent>>(&text).ok())
            .unwrap_or_default();
        let outbox = Self::with_items(Some(path.clone()), max_items, events);
        let ((), target) = outbox.state.mutate(|queue| ((), outbox.state.trim(queue)));
        if let Some(target) = target {
            outbox.state.persist_upto(&path, target);
        }
        outbox
    }

    /// Grava até `target` e espera a gravação terminar.
    async fn persist(&self, target: Option<u64>) {
        let (Some(path), Some(target)) = (self.state.path.clone(), target) else {
            return;
        };
        let state = self.state.clone();
        if tokio::task::spawn_blocking(move || state.persist_upto(&path, target))
            .await
            .is_err()
        {
            tracing::warn!("falha ao gravar o buffer offline do agente");
        }
    }

    /// Grava até `target` sem esperar: perder uma confirmação só reenvia o
    /// evento na próxima conexão, nunca o perde.
    fn persist_in_background(&self, target: Option<u64>) {
        let (Some(path), Some(target)) = (self.state.path.clone(), target) else {
            return;
        };
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let state = self.state.clone();
            handle.spawn_blocking(move || state.persist_upto(&path, target));
        } else {
            self.state.persist_upto(&path, target);
        }
    }

    /// Enfileira e só retorna com o evento no disco: um reinício logo depois
    /// não o perde. Gravações concorrentes coalescem numa só.
    pub async fn push(&self, event: AgentEvent) {
        let ((), target) = self.state.mutate(|queue| {
            let entry = queue.next_entry(event);
            queue.items.push_back(entry);
            self.state.trim(queue);
            ((), true)
        });
        self.persist(target).await;
    }

    /// Garante no disco o estado atual (encerramento do agente).
    pub async fn flush(&self) {
        let version = self.state.lock_queue().version;
        self.persist(Some(version)).await;
    }

    /// Espia até `max` eventos da frente sem removê-los nem tocar no disco,
    /// cada um com a sequência que o confirma em [`Self::ack`].
    #[must_use]
    pub fn peek_batch(&self, max: usize) -> Vec<(u64, AgentEvent)> {
        if max == 0 {
            return Vec::new();
        }
        self.state
            .lock_queue()
            .items
            .iter()
            .take(max)
            .map(|entry| (entry.seq, entry.event.clone()))
            .collect()
    }

    /// Remove os eventos já aceitos pela fila de saída **pela identidade**: um
    /// `push` concorrente que descartou o mais antigo pelo teto não faz a
    /// confirmação apagar um evento que não saiu.
    pub fn ack(&self, seqs: &[u64]) {
        if seqs.is_empty() {
            return;
        }
        let ((), target) = self.state.mutate(|queue| {
            let before = queue.items.len();
            queue.items.retain(|entry| !seqs.contains(&entry.seq));
            ((), queue.items.len() != before)
        });
        self.persist_in_background(target);
    }

    /// Tira tudo para envio. Sem itens, não toca no disco.
    #[must_use]
    pub fn drain(&self) -> Vec<AgentEvent> {
        let (drained, target) = self.state.mutate(|queue| {
            let drained: Vec<AgentEvent> = queue.items.drain(..).map(|entry| entry.event).collect();
            let changed = !drained.is_empty();
            (drained, changed)
        });
        self.persist_in_background(target);
        drained
    }

    /// Devolve à frente da fila o que não chegou a sair.
    pub fn restore(&self, events: Vec<AgentEvent>) {
        if events.is_empty() {
            return;
        }
        let ((), target) = self.state.mutate(|queue| {
            for event in events.into_iter().rev() {
                let entry = queue.next_entry(event);
                queue.items.push_front(entry);
            }
            self.state.trim(queue);
            ((), true)
        });
        self.persist_in_background(target);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.state.lock_queue().items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.state.lock_queue().items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::services::telemetry::rollup::MetricsRollup;

    fn rollup(samples: u32) -> AgentEvent {
        AgentEvent::MetricsRollup {
            rollup: Box::new(MetricsRollup {
                bucket_at: Utc::now(),
                samples,
                host: None,
                containers: Vec::new(),
            }),
        }
    }

    fn samples(event: &AgentEvent) -> u32 {
        match event {
            AgentEvent::MetricsRollup { rollup } => rollup.samples,
            _ => 0,
        }
    }

    fn temp_path(prefix: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("{prefix}-{}", uuid::Uuid::new_v4()))
            .join("outbox.json")
    }

    #[tokio::test]
    async fn teto_descarta_os_mais_antigos() {
        let outbox = Outbox::in_memory(2);
        for n in 1..=3 {
            outbox.push(rollup(n)).await;
        }
        let drained = outbox.drain();
        assert_eq!(drained.iter().map(samples).collect::<Vec<_>>(), vec![2, 3]);
        assert!(outbox.is_empty());
    }

    #[tokio::test]
    async fn restaurar_devolve_a_ordem_original() {
        let outbox = Outbox::in_memory(10);
        outbox.push(rollup(3)).await;
        outbox.restore(vec![rollup(1), rollup(2)]);
        assert_eq!(
            outbox.drain().iter().map(samples).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[tokio::test]
    async fn arquivo_sobrevive_ao_reinicio() {
        let path = temp_path("nm-outbox");
        Outbox::open(path.clone(), 10).push(rollup(7)).await;
        let reopened = Outbox::open(path.clone(), 10);
        assert_eq!(reopened.len(), 1);
        assert_eq!(samples(&reopened.drain()[0]), 7);
        let _ = std::fs::remove_dir_all(path.parent().expect("pasta"));
    }

    #[tokio::test]
    async fn push_so_retorna_com_o_evento_no_disco() {
        let path = temp_path("nm-outbox-durable");
        let outbox = Outbox::open(path.clone(), 10);
        outbox.push(rollup(1)).await;
        outbox.push(rollup(2)).await;
        // Sem esperar nada em segundo plano: o arquivo já tem os dois.
        let text = std::fs::read_to_string(&path).expect("arquivo gravado");
        let saved: Vec<AgentEvent> = serde_json::from_str(&text).expect("json");
        assert_eq!(saved.iter().map(samples).collect::<Vec<_>>(), vec![1, 2]);
        let _ = std::fs::remove_dir_all(path.parent().expect("pasta"));
    }

    #[tokio::test]
    async fn flush_grava_o_estado_atual() {
        let path = temp_path("nm-outbox-flush");
        let outbox = Outbox::open(path.clone(), 10);
        outbox.push(rollup(1)).await;
        outbox.push(rollup(2)).await;
        let sent: Vec<u64> = outbox
            .peek_batch(1)
            .into_iter()
            .map(|(seq, _)| seq)
            .collect();
        outbox.ack(&sent);
        outbox.flush().await;
        let text = std::fs::read_to_string(&path).expect("arquivo gravado");
        let saved: Vec<AgentEvent> = serde_json::from_str(&text).expect("json");
        assert_eq!(saved.iter().map(samples).collect::<Vec<_>>(), vec![2]);
        let _ = std::fs::remove_dir_all(path.parent().expect("pasta"));
    }

    #[test]
    fn drain_vazio_nao_toca_no_disco() {
        let path = temp_path("nm-outbox-empty");
        let outbox = Outbox::open(path.clone(), 10);
        assert!(outbox.drain().is_empty());
        assert!(!path.exists(), "drain vazio não deve gravar arquivo");
    }

    #[tokio::test]
    async fn peek_e_ack_mantem_eventos_ate_confirmacao() {
        let outbox = Outbox::in_memory(10);
        for n in 1..=3 {
            outbox.push(rollup(n)).await;
        }
        let batch = outbox.peek_batch(2);
        assert_eq!(
            batch
                .iter()
                .map(|(_, event)| samples(event))
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(outbox.len(), 3, "peek não remove");
        outbox.ack(&[batch[0].0]);
        assert_eq!(
            outbox.drain().iter().map(samples).collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[tokio::test]
    async fn ack_pela_identidade_nao_apaga_evento_nao_enviado() {
        let outbox = Outbox::in_memory(3);
        for n in 1..=3 {
            outbox.push(rollup(n)).await;
        }
        let batch = outbox.peek_batch(2);
        // Entre o peek e o ack, o teto descarta o mais antigo (1).
        outbox.push(rollup(4)).await;
        let sent: Vec<u64> = batch.iter().map(|(seq, _)| *seq).collect();
        outbox.ack(&sent);
        // Por contagem, o ack teria apagado 2 e 3 (o 3 nunca saiu).
        assert_eq!(
            outbox.drain().iter().map(samples).collect::<Vec<_>>(),
            vec![3, 4]
        );
    }
}
