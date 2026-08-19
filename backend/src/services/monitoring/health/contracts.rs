//! Contratos da coleta de saúde local.
//!
//! São propositalmente pequenos e independentes de persistência: cada fonte
//! sabe ler **um** assunto e devolve medidas normalizadas. Quem escolhe entre
//! fontes e monta o resultado é o coordenador, que por isso pode ser testado
//! sem tocar em `/proc` e sem banco.

use serde::{Deserialize, Serialize};

/// De onde veio o número.
///
/// A distinção não é decorativa: dentro de um container o limite de memória do
/// cgroup e a memória do host respondem perguntas diferentes, e mostrar uma
/// pela outra é como reportar o combustível do caminhão ao lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeasureSource {
    Host,
    Cgroup,
    Process,
}

impl MeasureSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Cgroup => "cgroup",
            Self::Process => "process",
        }
    }
}

/// Uma grandeza medida, com unidade explícita e origem declarada.
#[derive(Debug, Clone, PartialEq)]
pub struct Measure {
    /// Nome da série, no vocabulário de `metrics.name` (§3.2).
    pub name: &'static str,
    pub value: f64,
    pub unit: &'static str,
    pub source: MeasureSource,
}

impl Measure {
    #[must_use]
    pub const fn new(
        name: &'static str,
        value: f64,
        unit: &'static str,
        source: MeasureSource,
    ) -> Self {
        Self {
            name,
            value,
            unit,
            source,
        }
    }
}

/// Métrica que esta instalação não consegue medir, e por quê.
///
/// Existe para que o resultado diga "indisponível" em vez de inventar zero —
/// um `0%` de CPU é indistinguível de um servidor ocioso e mentiria para o
/// operador e para o motor de alertas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    pub name: &'static str,
    pub reason: String,
}

/// O que uma fonte devolve: o que conseguiu medir e o que não conseguiu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reading {
    pub measures: Vec<Measure>,
    pub unavailable: Vec<Unavailable>,
}

impl Reading {
    #[must_use]
    pub fn measure(mut self, measure: Measure) -> Self {
        self.measures.push(measure);
        self
    }

    #[must_use]
    pub fn missing(mut self, name: &'static str, reason: impl Into<String>) -> Self {
        self.unavailable.push(Unavailable {
            name,
            reason: reason.into(),
        });
        self
    }

    pub fn absorb(&mut self, other: Self) {
        self.measures.extend(other.measures);
        self.unavailable.extend(other.unavailable);
    }
}

/// Uma fonte de saúde. Uma por assunto — host, cgroup, processo,
/// armazenamento —, cada uma trocável em teste por um dublê.
pub trait HealthSource: Send + Sync {
    /// Nome da fonte, para diagnóstico.
    fn name(&self) -> &'static str;

    /// Lê o que sabe ler. Nunca falha: indisponibilidade é dado de domínio.
    fn read(&self) -> Reading;
}

/// Relógio injetável, para tornar determinístico o cálculo de uptime e de
/// deltas de CPU.
pub trait Clock: Send + Sync {
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
}

/// O relógio de verdade.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}
