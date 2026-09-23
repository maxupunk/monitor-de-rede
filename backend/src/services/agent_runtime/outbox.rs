//! Buffer offline do agente: eventos que não puderam ser entregues.
//!
//! Rollups de minuto e resultados de monitor produzidos com o canal fora do
//! ar esperam aqui e saem na próxima conexão. Persistido em arquivo para
//! sobreviver a um reinício do agente, com teto de itens — descartando os
//! mais antigos — para não encher o disco do servidor monitorado.

use std::{collections::VecDeque, path::PathBuf, sync::Mutex};

use crate::services::agents::protocol::AgentEvent;

pub struct Outbox {
    path: Option<PathBuf>,
    max_items: usize,
    items: Mutex<VecDeque<AgentEvent>>,
}

impl Outbox {
    /// Em memória, sem arquivo (testes).
    #[must_use]
    pub fn in_memory(max_items: usize) -> Self {
        Self {
            path: None,
            max_items: max_items.max(1),
            items: Mutex::new(VecDeque::new()),
        }
    }

    /// Abre o arquivo existente; conteúdo ilegível começa vazio.
    #[must_use]
    pub fn open(path: PathBuf, max_items: usize) -> Self {
        let items = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<VecDeque<AgentEvent>>(&text).ok())
            .unwrap_or_default();
        let outbox = Self {
            path: Some(path),
            max_items: max_items.max(1),
            items: Mutex::new(items),
        };
        outbox.trim_and_persist(&mut outbox.lock());
        outbox
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<AgentEvent>> {
        self.items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn trim_and_persist(&self, items: &mut VecDeque<AgentEvent>) {
        while items.len() > self.max_items {
            items.pop_front();
        }
        let Some(path) = &self.path else {
            return;
        };
        let Ok(text) = serde_json::to_string(&*items) else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let temporary = path.with_extension("tmp");
        if std::fs::write(&temporary, text).is_ok() {
            let _ = std::fs::rename(temporary, path);
        }
    }

    pub fn push(&self, event: AgentEvent) {
        let mut items = self.lock();
        items.push_back(event);
        self.trim_and_persist(&mut items);
    }

    /// Tira tudo para envio.
    #[must_use]
    pub fn drain(&self) -> Vec<AgentEvent> {
        let mut items = self.lock();
        let drained: Vec<AgentEvent> = items.drain(..).collect();
        self.trim_and_persist(&mut items);
        drained
    }

    /// Devolve à frente da fila o que não chegou a sair.
    pub fn restore(&self, events: Vec<AgentEvent>) {
        let mut items = self.lock();
        for event in events.into_iter().rev() {
            items.push_front(event);
        }
        self.trim_and_persist(&mut items);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
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

    #[test]
    fn teto_descarta_os_mais_antigos() {
        let outbox = Outbox::in_memory(2);
        for n in 1..=3 {
            outbox.push(rollup(n));
        }
        let drained = outbox.drain();
        assert_eq!(drained.iter().map(samples).collect::<Vec<_>>(), vec![2, 3]);
        assert!(outbox.is_empty());
    }

    #[test]
    fn restaurar_devolve_a_ordem_original() {
        let outbox = Outbox::in_memory(10);
        outbox.push(rollup(3));
        outbox.restore(vec![rollup(1), rollup(2)]);
        assert_eq!(
            outbox.drain().iter().map(samples).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn arquivo_sobrevive_ao_reinicio() {
        let path = std::env::temp_dir()
            .join(format!("nm-outbox-{}", uuid::Uuid::new_v4()))
            .join("outbox.json");
        Outbox::open(path.clone(), 10).push(rollup(7));
        let reopened = Outbox::open(path.clone(), 10);
        assert_eq!(reopened.len(), 1);
        assert_eq!(samples(&reopened.drain()[0]), 7);
        let _ = std::fs::remove_dir_all(path.parent().expect("pasta"));
    }
}
