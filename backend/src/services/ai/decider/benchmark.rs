//! O resultado de cada modelo num teste público e independente.
//!
//! Fonte: "Qual modelo de decisão (classificador de IA tipo Jev) é melhor?",
//! akitaonrails.com, 10/10/2026 — 23 modelos, 1.024 classificações por
//! modelo em tarefas operacionais e semânticas, **em inglês e em português**.
//! Escolher no chute acertaria ~31%. Os locais rodaram num Ryzen AI Max+ 395
//! (GPU integrada); os de nuvem, pela internet.
//!
//! É o critério da recomendação e da ordem da lista na tela: o número que o
//! operador precisa (acerta? e em português?) medido do mesmo jeito para
//! todos. O acerto que cada autor publica (`typed-decisions`) vem de testes
//! diferentes e fica só como referência quando não há este.

use crate::dtos::ai::DeciderBenchmarkView;

/// Acerto mínimo para um modelo ser recomendável: abaixo disso, perto do
/// chute (~31%).
pub const USEFUL_PERMILLE: u16 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Benchmark {
    /// Acerto geral (inglês e português), em milésimos.
    pub accuracy_permille: u16,
    /// Acerto só em português (forma compacta), em milésimos.
    pub portuguese_permille: u16,
    /// Latência medida no teste; `None` quando não foi medida.
    pub latency_ms: Option<u32>,
}

impl Benchmark {
    pub const fn new(accuracy: u16, portuguese: u16, latency_ms: u32) -> Self {
        Self {
            accuracy_permille: accuracy,
            portuguese_permille: portuguese,
            latency_ms: Some(latency_ms),
        }
    }

    /// Para quem o teste mediu só o acerto (rodou fora da rede do teste).
    pub const fn accuracy_only(accuracy: u16, portuguese: u16) -> Self {
        Self {
            accuracy_permille: accuracy,
            portuguese_permille: portuguese,
            latency_ms: None,
        }
    }

    #[must_use]
    pub const fn is_useful(&self) -> bool {
        self.accuracy_permille >= USEFUL_PERMILLE
    }

    #[must_use]
    pub fn view(&self) -> DeciderBenchmarkView {
        DeciderBenchmarkView {
            accuracy: f64::from(self.accuracy_permille) / 1_000.0,
            portuguese: f64::from(self.portuguese_permille) / 1_000.0,
            latency_ms: self.latency_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn util_e_o_que_fica_bem_acima_do_chute() {
        assert!(Benchmark::new(631, 619, 86).is_useful(), "Kev 0.8B");
        assert!(
            !Benchmark::new(369, 381, 19).is_useful(),
            "Laya multilingual"
        );
        let view = Benchmark::new(905, 893, 205).view();
        assert!((view.accuracy - 0.905).abs() < f64::EPSILON);
        assert_eq!(view.latency_ms, Some(205));
    }
}
