//! Coleta de saúde local do próprio NetMonitor.
//!
//! A saída deste módulo é um [`CheckResult`] comum — o mesmo tipo que o ping,
//! o TCP e o SNMP produzem. Daí para a frente nada aqui é especial: o
//! `process_result` grava `monitor_results`, grava as séries de dispositivo em
//! `metrics` e chama o motor de alertas, exatamente como faz para um roteador.
//!
//! # As séries, e por que estes nomes
//!
//! Os nomes vivem em [`series`] e pertencem à família que o SNMP **já** grava
//! (`cpu_usage`, `memory_usage`, `inBps`, `outBps`). É o que faz os widgets de
//! CPU e memória e o endpoint `/devices/{id}/metrics` aceitarem o servidor sem
//! uma linha de frontend nova. Nenhum nome exclusivo do servidor nasce aqui.

pub mod contracts;
pub mod parsers;
pub mod sources;

use chrono::Utc;

use crate::services::monitoring::contracts::{CheckMetric, CheckResult, MonitorStatus};

use contracts::{HealthSource, MeasureSource, Reading};

/// Os nomes de série, no vocabulário de `metrics.name` (§3.2).
///
/// Constantes, e não literais espalhados: é este módulo que decide o que é uma
/// série de dispositivo, e o `process_result` consulta a mesma lista.
pub mod series {
    pub const CPU_USAGE: &str = "cpu_usage";
    pub const MEMORY_USAGE: &str = "memory_usage";
    pub const MEMORY_USED_BYTES: &str = "memory_used_bytes";
    pub const MEMORY_TOTAL_BYTES: &str = "memory_total_bytes";
    pub const STORAGE_USAGE: &str = "storage_usage";
    pub const LOAD_AVERAGE_1M: &str = "load_average_1m";
    pub const PROCESS_MEMORY_BYTES: &str = "process_memory_bytes";
    pub const UPTIME_SECONDS: &str = "uptime_seconds";
    pub const IN_BPS: &str = "inBps";
    pub const OUT_BPS: &str = "outBps";
}

/// Coordena as fontes e normaliza o resultado.
///
/// Não sabe ler arquivo nem gravar no banco: recebe fontes prontas e devolve
/// um `CheckResult`. É o que permite testá-lo com dublês, sem `/proc` e sem
/// banco.
pub struct HealthCoordinator {
    sources: Vec<Box<dyn HealthSource>>,
}

impl HealthCoordinator {
    #[must_use]
    pub fn new(sources: Vec<Box<dyn HealthSource>>) -> Self {
        Self { sources }
    }

    /// As fontes de produção, na ordem em que a precedência as consulta.
    #[must_use]
    pub fn with_default_sources() -> Self {
        Self::new(vec![
            Box::new(sources::HostSourceLinux::new(sources::ProcRoot::default())),
            Box::new(sources::CgroupSourceLinux::default()),
            Box::new(sources::ProcessSourceLinux::new(
                sources::ProcRoot::default(),
            )),
            Box::new(sources::StorageSource::default()),
        ])
    }

    /// Executa uma coleta e devolve o resultado no formato comum.
    #[must_use]
    pub fn collect(&self) -> CheckResult {
        let started_at = Utc::now();
        let mut bruto = Reading::default();
        for source in &self.sources {
            bruto.absorb(source.read());
        }
        let finished_at = Utc::now();
        finalize(bruto, started_at, finished_at)
    }
}

/// Resolve precedência, monta as métricas e descreve o que faltou.
///
/// Separado de [`HealthCoordinator::collect`] porque é aqui que mora a única
/// decisão não trivial — qual fonte ganha quando duas respondem a mesma
/// pergunta — e ela merece teste próprio, sem relógio nem arquivo.
fn finalize(
    bruto: Reading,
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
) -> CheckResult {
    // Precedência: dentro de um container com limite, o cgroup responde a
    // pergunta que o operador está fazendo ("quanto falta para o OOM killer"),
    // e o host responde outra. Uma série, um número, uma origem declarada.
    let mut escolhidas: Vec<contracts::Measure> = Vec::new();
    for medida in bruto.measures {
        match escolhidas
            .iter_mut()
            .find(|atual| atual.name == medida.name)
        {
            Some(atual) => {
                if precedencia(medida.source) > precedencia(atual.source) {
                    *atual = medida;
                }
            }
            None => escolhidas.push(medida),
        }
    }

    let mut origens = serde_json::Map::new();
    let metrics: Vec<CheckMetric> = escolhidas
        .iter()
        .map(|medida| {
            origens.insert(
                medida.name.to_string(),
                serde_json::Value::String(medida.source.as_str().to_string()),
            );
            CheckMetric {
                name: medida.name.to_string(),
                value: medida.value,
                unit: medida.unit.to_string(),
            }
        })
        .collect();

    // Indisponibilidade de uma série que outra fonte cobriu não é
    // indisponibilidade: o cgroup pode falhar sem que a memória do host falte.
    let indisponiveis: Vec<&contracts::Unavailable> = bruto
        .unavailable
        .iter()
        .filter(|faltante| !escolhidas.iter().any(|m| m.name == faltante.name))
        .collect();

    let unavailable: serde_json::Map<String, serde_json::Value> = indisponiveis
        .iter()
        .map(|faltante| {
            (
                faltante.name.to_string(),
                serde_json::Value::String(faltante.reason.clone()),
            )
        })
        .collect();

    // O status é sobre a **coleta**, não sobre a saúde: julgar "CPU alta é
    // down" aqui roubaria do motor de alertas a decisão que é dele. Sem
    // nenhuma medida a coleta não funcionou; com parte delas, funcionou
    // parcialmente.
    let (status, message) = if metrics.is_empty() {
        (
            MonitorStatus::Unknown,
            Some("Nenhuma métrica de saúde pôde ser coletada neste sistema".to_string()),
        )
    } else if unavailable.is_empty() {
        (
            MonitorStatus::Up,
            Some(format!("{} métricas coletadas", metrics.len())),
        )
    } else {
        (
            MonitorStatus::Up,
            Some(format!(
                "{} métricas coletadas; {} indisponíveis neste sistema",
                metrics.len(),
                unavailable.len()
            )),
        )
    };

    let mut data = serde_json::Map::new();
    // Os campos avaliáveis por regra entram no dataset da Fase 3; aqui ficam
    // as informações de diagnóstico que a Visão Geral mostra.
    data.insert("sources".to_string(), serde_json::Value::Object(origens));
    data.insert(
        "unavailable".to_string(),
        serde_json::Value::Object(unavailable),
    );

    CheckResult {
        success: status == MonitorStatus::Up,
        status,
        started_at,
        finished_at,
        duration_ms: (finished_at - started_at).num_milliseconds().max(0),
        message,
        metrics,
        data: serde_json::Value::Object(data),
    }
}

/// Quanto uma origem vale quando duas respondem a mesma série.
const fn precedencia(source: MeasureSource) -> u8 {
    match source {
        MeasureSource::Host => 0,
        MeasureSource::Process => 1,
        // O limite do container é o teto real: é ele que decide o OOM.
        MeasureSource::Cgroup => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::{contracts::*, *};

    /// Fonte de mentira, para testar o coordenador sem tocar em `/proc`.
    struct Dublê(&'static str, Reading);

    impl HealthSource for Dublê {
        fn name(&self) -> &'static str {
            self.0
        }
        fn read(&self) -> Reading {
            self.1.clone()
        }
    }

    fn coleta(fontes: Vec<Box<dyn HealthSource>>) -> CheckResult {
        HealthCoordinator::new(fontes).collect()
    }

    fn metrica<'a>(resultado: &'a CheckResult, nome: &str) -> Option<&'a CheckMetric> {
        resultado.metrics.iter().find(|m| m.name == nome)
    }

    #[test]
    fn o_limite_do_cgroup_vence_a_memoria_do_host() {
        let resultado = coleta(vec![
            Box::new(Dublê(
                "host",
                Reading::default().measure(Measure::new(
                    series::MEMORY_USAGE,
                    20.0,
                    "percent",
                    MeasureSource::Host,
                )),
            )),
            Box::new(Dublê(
                "cgroup",
                Reading::default().measure(Measure::new(
                    series::MEMORY_USAGE,
                    91.0,
                    "percent",
                    MeasureSource::Cgroup,
                )),
            )),
        ]);

        let memoria = metrica(&resultado, series::MEMORY_USAGE).expect("memória");
        assert_eq!(
            memoria.value, 91.0,
            "dentro de um container quem decide o OOM é o cgroup"
        );
        assert_eq!(resultado.data["sources"][series::MEMORY_USAGE], "cgroup");
        assert_eq!(
            resultado.metrics.len(),
            1,
            "a mesma série não pode aparecer duas vezes"
        );
    }

    #[test]
    fn a_ordem_das_fontes_nao_muda_a_precedencia() {
        let host = || {
            Box::new(Dublê(
                "host",
                Reading::default().measure(Measure::new(
                    series::MEMORY_USAGE,
                    20.0,
                    "percent",
                    MeasureSource::Host,
                )),
            )) as Box<dyn HealthSource>
        };
        let cgroup = || {
            Box::new(Dublê(
                "cgroup",
                Reading::default().measure(Measure::new(
                    series::MEMORY_USAGE,
                    91.0,
                    "percent",
                    MeasureSource::Cgroup,
                )),
            )) as Box<dyn HealthSource>
        };
        assert_eq!(
            metrica(&coleta(vec![host(), cgroup()]), series::MEMORY_USAGE)
                .unwrap()
                .value,
            metrica(&coleta(vec![cgroup(), host()]), series::MEMORY_USAGE)
                .unwrap()
                .value
        );
    }

    #[test]
    fn metrica_indisponivel_e_declarada_com_o_motivo_e_nao_vira_zero() {
        let resultado = coleta(vec![Box::new(Dublê(
            "host",
            Reading::default()
                .measure(Measure::new(
                    series::CPU_USAGE,
                    12.0,
                    "percent",
                    MeasureSource::Host,
                ))
                .missing(series::STORAGE_USAGE, "sistema de arquivos não consultável"),
        ))]);

        assert!(metrica(&resultado, series::STORAGE_USAGE).is_none());
        assert_eq!(
            resultado.data["unavailable"][series::STORAGE_USAGE],
            "sistema de arquivos não consultável"
        );
        assert_eq!(resultado.status, MonitorStatus::Up, "a coleta funcionou");
        assert!(resultado.message.unwrap().contains("indisponíveis"));
    }

    #[test]
    fn falta_coberta_por_outra_fonte_nao_e_falta() {
        let resultado = coleta(vec![
            Box::new(Dublê(
                "cgroup",
                Reading::default().missing(series::MEMORY_USAGE, "sem cgroup"),
            )),
            Box::new(Dublê(
                "host",
                Reading::default().measure(Measure::new(
                    series::MEMORY_USAGE,
                    33.0,
                    "percent",
                    MeasureSource::Host,
                )),
            )),
        ]);
        assert_eq!(
            metrica(&resultado, series::MEMORY_USAGE).unwrap().value,
            33.0
        );
        assert!(
            resultado.data["unavailable"]
                .as_object()
                .unwrap()
                .is_empty(),
            "o host respondeu; o cgroup ter falhado não é problema do operador"
        );
    }

    #[test]
    fn sem_nenhuma_metrica_a_coleta_e_unknown_e_nao_up_com_zeros() {
        let resultado = coleta(vec![Box::new(Dublê(
            "host",
            Reading::default().missing(series::CPU_USAGE, "sem /proc"),
        ))]);
        assert_eq!(resultado.status, MonitorStatus::Unknown);
        assert!(!resultado.success);
        assert!(resultado.metrics.is_empty());
    }
}
