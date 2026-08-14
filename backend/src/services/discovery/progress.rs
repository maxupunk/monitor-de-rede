//! Ponte entre os scanners e a sessão de varredura ao vivo.
//!
//! Sem ela, cada fase só publicava progresso ao começar e ao terminar: a barra
//! da tela ficava parada em 0% durante toda a varredura de portas e os hosts
//! achados no ping só apareciam depois que o sweep inteiro terminava. O canal
//! mantém os scanners ignorantes de `AppContext` e da sessão — eles só relatam
//! o que já sabem, e quem traduz isso em estado é o [`super::service`].

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::merger::DiscoveredHost;

/// Teto de atualizações de progresso por fase.
///
/// Uma faixa /22 tem 1024 endereços; emitir uma atualização por endereço
/// enche o canal de broadcast da sessão e faz o SSE do navegador perder
/// mensagens. Uma barra que anda de 2% em 2% já é "tempo real" para quem olha.
const PROGRESS_STEPS: usize = 50;

#[derive(Debug)]
pub enum ScanEvent {
    Progress {
        phase: &'static str,
        current: usize,
        total: usize,
    },
    Hosts(Vec<DiscoveredHost>),
}

/// Emissor dos eventos. Clonável e não-bloqueante: relatar progresso nunca
/// pode segurar o scanner esperando quem consome a tela.
#[derive(Clone, Default)]
pub struct ScanReporter {
    sender: Option<UnboundedSender<ScanEvent>>,
}

impl ScanReporter {
    #[must_use]
    pub fn channel() -> (Self, UnboundedReceiver<ScanEvent>) {
        let (sender, receiver) = unbounded_channel();
        (
            Self {
                sender: Some(sender),
            },
            receiver,
        )
    }

    /// Repórter mudo, para chamar os scanners fora de uma sessão (testes).
    #[must_use]
    pub fn silent() -> Self {
        Self::default()
    }

    /// Avanço dentro da fase — publicado só nos marcos de [`emits_at`].
    pub fn progress(&self, phase: &'static str, current: usize, total: usize) {
        if emits_at(current, total) {
            self.phase(phase, current, total);
        }
    }

    /// Troca de fase, sempre publicada: é ela que muda o rótulo na tela, e
    /// perder essa mensagem deixaria o operador lendo "Ping (ICMP)" enquanto o
    /// servidor já está varrendo portas.
    pub fn phase(&self, phase: &'static str, current: usize, total: usize) {
        self.send(ScanEvent::Progress {
            phase,
            current,
            total,
        });
    }

    pub fn hosts(&self, hosts: &[DiscoveredHost]) {
        self.send(ScanEvent::Hosts(hosts.to_vec()));
    }

    fn send(&self, event: ScanEvent) {
        if let Some(sender) = &self.sender {
            // Receptor fechado significa varredura encerrada: o scanner que
            // ainda estiver drenando não deve falhar por causa disso.
            let _ = sender.send(event);
        }
    }
}

/// Marcos em que vale publicar: o começo, o fim e um passo a cada 1/50 do total.
#[must_use]
pub fn emits_at(current: usize, total: usize) -> bool {
    if total == 0 || current >= total {
        return true;
    }
    current.is_multiple_of((total / PROGRESS_STEPS).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limita_atualizacoes_por_fase_sem_perder_inicio_e_fim() {
        let emitted = (0..=1_024)
            .filter(|current| emits_at(*current, 1_024))
            .count();
        // ~50 marcos, mais o arredondamento do passo e o quadro final.
        assert!(emitted <= PROGRESS_STEPS + 5, "{emitted} atualizações");
        assert!(emits_at(0, 1_024));
        assert!(emits_at(1_024, 1_024));
    }

    /// Fase curta não pode virar barra de dois quadros: com 8 hosts, cada host
    /// é 12,5% do progresso e todos precisam aparecer.
    #[test]
    fn fase_curta_emite_todos_os_passos() {
        assert!((0..=8).all(|current| emits_at(current, 8)));
    }

    /// O marco existe para poupar mensagens de avanço, nunca para engolir a
    /// troca de fase — é ela que renomeia o que a tela está mostrando.
    #[tokio::test]
    async fn troca_de_fase_passa_por_fora_do_marco() {
        let (reporter, mut events) = ScanReporter::channel();
        reporter.progress("icmp", 7, 254);
        reporter.phase("ports", 7, 254);
        drop(reporter);

        let mut published = Vec::new();
        while let Some(event) = events.recv().await {
            if let ScanEvent::Progress { phase, .. } = event {
                published.push(phase);
            }
        }
        assert_eq!(published, ["ports"]);
    }

    #[tokio::test]
    async fn reporter_mudo_nao_falha_sem_receptor() {
        ScanReporter::silent().progress("icmp", 1, 10);
        ScanReporter::silent().hosts(&[]);
    }
}
