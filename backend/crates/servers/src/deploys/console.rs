//! O console ao vivo de uma execução longa (deploy, compilação): as linhas
//! vão ao SSE em lotes e ao banco de tempos em tempos, e a etapa atual é
//! gravada e publicada.
//!
//! O ritmo e o buffer são um só para tudo; o que muda de uma execução para
//! outra é o destino ([`ConsoleSink`]).

use std::time::{Duration, Instant};

use async_trait::async_trait;
use loco_rs::app::AppContext;

use super::runs::LogBuffer;

const PUBLISH_EVERY: Duration = Duration::from_millis(300);
const SAVE_EVERY: Duration = Duration::from_secs(2);

/// Para onde vai o console de uma execução.
#[async_trait]
pub trait ConsoleSink: Send + Sync {
    type Stage: Send;

    /// Publica linhas novas no SSE.
    fn publish(&self, ctx: &AppContext, lines: Vec<String>);

    /// Grava o log inteiro.
    async fn save(&self, ctx: &AppContext, text: &str);

    /// Grava e publica a etapa atual.
    async fn stage(&self, ctx: &AppContext, stage: Self::Stage);
}

pub struct Console<S: ConsoleSink> {
    ctx: AppContext,
    sink: S,
    buffer: LogBuffer,
    unsent: Vec<String>,
    last_publish: Instant,
    last_save: Instant,
    /// Etapa travada (o rollback não deixa a etapa voltar).
    pub pinned: bool,
}

impl<S: ConsoleSink> Console<S> {
    pub fn new(ctx: &AppContext, sink: S, saved: Option<&str>) -> Self {
        Self {
            ctx: ctx.clone(),
            sink,
            buffer: LogBuffer::from_saved(saved),
            unsent: Vec::new(),
            last_publish: Instant::now(),
            last_save: Instant::now(),
            pinned: false,
        }
    }

    pub async fn line(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.buffer.push(&text);
        self.unsent.push(text);
        if self.last_publish.elapsed() >= PUBLISH_EVERY {
            self.publish();
        }
        if self.last_save.elapsed() >= SAVE_EVERY {
            self.save().await;
        }
    }

    fn publish(&mut self) {
        self.sink
            .publish(&self.ctx, std::mem::take(&mut self.unsent));
        self.last_publish = Instant::now();
    }

    async fn save(&mut self) {
        self.sink.save(&self.ctx, &self.buffer.text()).await;
        self.last_save = Instant::now();
    }

    pub async fn stage(&mut self, stage: S::Stage) {
        if self.pinned {
            return;
        }
        self.publish();
        self.sink.stage(&self.ctx, stage).await;
    }

    pub async fn flush(&mut self) {
        self.publish();
        self.save().await;
    }
}

/// Um tamanho para o console, em base 1024 (`3.0 GiB`).
#[must_use]
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamanho_legivel() {
        assert_eq!(human_bytes(512), "512.0 B");
        assert_eq!(human_bytes(1536), "1.5 KiB");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }
}
