//! Agregação das amostras de host e containers em baldes de 1 minuto.
//!
//! O agente amostra a cada ~10 s, mas só o balde fechado atravessa o canal e
//! vai ao banco: é o que mantém 30 dias de histórico num SQLite sem inflar.
//! Gauges (CPU, memória, load) viram média e máximo; contadores cumulativos
//! (rede, disco) viram o delta do minuto, tolerando reinício do container.

use std::collections::BTreeMap;

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::docker::views::DockerMetricsResponse;

/// Uso de um ponto de montagem no instante da amostra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct DiskUsage {
    pub mount: String,
    #[ts(type = "number")]
    pub used_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostSample {
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub load1: f64,
    pub net_rx_bps: f64,
    pub net_tx_bps: f64,
    pub disks: Vec<DiskUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSample {
    pub name: String,
    pub project: Option<String>,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    /// Contadores cumulativos desde o início do container.
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
}

impl ContainerSample {
    /// Converte a leitura de métricas Docker já existente (mesma fórmula de
    /// CPU e memória do CLI, `docker/metrics.rs`).
    #[must_use]
    pub fn from_metrics(metrics: &DockerMetricsResponse) -> Vec<Self> {
        metrics
            .containers
            .iter()
            .map(|container| Self {
                name: container.container_name.clone(),
                project: container.project_name.clone(),
                cpu_percent: container.cpu.usage_percent,
                memory_bytes: container.memory.usage_bytes,
                net_rx_bytes: container.network.received_bytes,
                net_tx_bytes: container.network.transmitted_bytes,
                block_read_bytes: container.block_io.read_bytes,
                block_write_bytes: container.block_io.write_bytes,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRollup {
    pub cpu_avg: f64,
    pub cpu_max: f64,
    pub memory_used_avg: f64,
    pub memory_total: u64,
    pub load1: f64,
    pub net_rx_bps: f64,
    pub net_tx_bps: f64,
    pub disks: Vec<DiskUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerRollup {
    pub name: String,
    pub project: Option<String>,
    pub cpu_avg: f64,
    pub cpu_max: f64,
    pub memory_avg: f64,
    pub memory_max: u64,
    /// Bytes no minuto (delta dos contadores).
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
}

/// Um minuto fechado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsRollup {
    pub bucket_at: DateTime<Utc>,
    pub samples: u32,
    pub host: Option<HostRollup>,
    pub containers: Vec<ContainerRollup>,
}

/// Início do minuto que contém `at`.
#[must_use]
pub fn bucket_of(at: DateTime<Utc>) -> DateTime<Utc> {
    at.duration_trunc(TimeDelta::minutes(1)).unwrap_or(at)
}

#[derive(Default)]
struct HostBucket {
    count: u32,
    cpu_sum: f64,
    cpu_max: f64,
    memory_used_sum: f64,
    memory_total: u64,
    load1_sum: f64,
    net_rx_bps_sum: f64,
    net_tx_bps_sum: f64,
    disks: Vec<DiskUsage>,
}

impl HostBucket {
    #[allow(clippy::cast_precision_loss)]
    fn push(&mut self, sample: HostSample) {
        self.count = self.count.saturating_add(1);
        self.cpu_sum += finite(sample.cpu_percent);
        self.cpu_max = self.cpu_max.max(finite(sample.cpu_percent));
        self.memory_used_sum += sample.memory_used_bytes as f64;
        self.memory_total = sample.memory_total_bytes;
        self.load1_sum += finite(sample.load1);
        self.net_rx_bps_sum += finite(sample.net_rx_bps);
        self.net_tx_bps_sum += finite(sample.net_tx_bps);
        self.disks = sample.disks;
    }

    fn finish(self) -> Option<HostRollup> {
        if self.count == 0 {
            return None;
        }
        let denom = f64::from(self.count);
        Some(HostRollup {
            cpu_avg: self.cpu_sum / denom,
            cpu_max: self.cpu_max,
            memory_used_avg: self.memory_used_sum / denom,
            memory_total: self.memory_total,
            load1: self.load1_sum / denom,
            net_rx_bps: self.net_rx_bps_sum / denom,
            net_tx_bps: self.net_tx_bps_sum / denom,
            disks: self.disks,
        })
    }
}

#[derive(Clone, Copy)]
struct CounterBaseline {
    net_rx_bytes: u64,
    net_tx_bytes: u64,
    block_read_bytes: u64,
    block_write_bytes: u64,
}

impl CounterBaseline {
    const fn from_sample(sample: &ContainerSample) -> Self {
        Self {
            net_rx_bytes: sample.net_rx_bytes,
            net_tx_bytes: sample.net_tx_bytes,
            block_read_bytes: sample.block_read_bytes,
            block_write_bytes: sample.block_write_bytes,
        }
    }
}

fn step_delta(before: u64, after: u64) -> u64 {
    if after >= before {
        after - before
    } else {
        after
    }
}

#[derive(Default)]
struct ContainerBucket {
    project: Option<String>,
    count: u32,
    cpu_sum: f64,
    cpu_max: f64,
    memory_sum: f64,
    memory_max: u64,
    net_rx_bytes: u64,
    net_tx_bytes: u64,
    block_read_bytes: u64,
    block_write_bytes: u64,
}

impl ContainerBucket {
    #[allow(clippy::cast_precision_loss)]
    fn push(&mut self, sample: ContainerSample, prev: Option<CounterBaseline>) {
        self.project = sample.project;
        self.count = self.count.saturating_add(1);
        let cpu = finite(sample.cpu_percent);
        self.cpu_sum += cpu;
        self.cpu_max = self.cpu_max.max(cpu);
        self.memory_sum += sample.memory_bytes as f64;
        self.memory_max = self.memory_max.max(sample.memory_bytes);
        if let Some(prev) = prev {
            self.net_rx_bytes = self
                .net_rx_bytes
                .saturating_add(step_delta(prev.net_rx_bytes, sample.net_rx_bytes));
            self.net_tx_bytes = self
                .net_tx_bytes
                .saturating_add(step_delta(prev.net_tx_bytes, sample.net_tx_bytes));
            self.block_read_bytes = self
                .block_read_bytes
                .saturating_add(step_delta(prev.block_read_bytes, sample.block_read_bytes));
            self.block_write_bytes = self
                .block_write_bytes
                .saturating_add(step_delta(prev.block_write_bytes, sample.block_write_bytes));
        }
    }

    fn finish(self, name: String) -> Option<ContainerRollup> {
        if self.count == 0 {
            return None;
        }
        let denom = f64::from(self.count);
        Some(ContainerRollup {
            name,
            project: self.project,
            cpu_avg: self.cpu_sum / denom,
            cpu_max: self.cpu_max,
            memory_avg: self.memory_sum / denom,
            memory_max: self.memory_max,
            net_rx_bytes: self.net_rx_bytes,
            net_tx_bytes: self.net_tx_bytes,
            block_read_bytes: self.block_read_bytes,
            block_write_bytes: self.block_write_bytes,
        })
    }
}

fn finite(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

/// Recuo de relógio tolerado sem reiniciar a agregação (ajuste fino do NTP).
/// Acima disto o relógio foi corrigido de verdade (RTC adiantado, por
/// exemplo): ignorar as amostras deixaria o agente sem histórico até o
/// relógio novo alcançar o antigo.
const MAX_CLOCK_STEP_BACK: TimeDelta = TimeDelta::minutes(2);

#[derive(Default)]
pub struct RollupAccumulator {
    bucket: Option<DateTime<Utc>>,
    /// O último minuto fechado: os contadores só continuam valendo como base
    /// se o próximo balde for o minuto seguinte a ele.
    last_closed: Option<DateTime<Utc>>,
    host: HostBucket,
    containers: BTreeMap<String, ContainerBucket>,
    /// Última leitura de contadores cumulativos por container, preservada entre
    /// minutos para não perder o tráfego da fronteira entre baldes.
    baselines: BTreeMap<String, CounterBaseline>,
    samples: u32,
}

impl RollupAccumulator {
    /// Registra uma amostra. Devolve o minuto anterior quando esta amostra
    /// abre um novo.
    pub fn push(
        &mut self,
        at: DateTime<Utc>,
        host: Option<HostSample>,
        containers: Vec<ContainerSample>,
    ) -> Option<MetricsRollup> {
        let bucket = bucket_of(at);
        let closed = match self.bucket {
            // Amostra com relógio pouco atrasado (ajuste NTP) não fecha o
            // minuto atual nem abre balde retroativo duplicado.
            Some(current) if bucket < current && current - bucket <= MAX_CLOCK_STEP_BACK => {
                return None;
            }
            // Minuto seguinte, lacuna ou recuo grande do relógio: fecha o
            // corrente e recomeça neste.
            Some(current) if bucket != current => self.close(),
            _ => None,
        };
        if self.bucket.is_none() {
            // Depois de uma lacuna (ou de um recuo), o delta desde a última
            // leitura somaria minutos inteiros neste: pico falso.
            let follows = self
                .last_closed
                .is_some_and(|previous| bucket - previous == TimeDelta::minutes(1));
            if !follows {
                self.baselines.clear();
            }
        }
        self.bucket = Some(bucket);
        self.samples = self.samples.saturating_add(1);
        if let Some(host) = host {
            self.host.push(host);
        }
        for sample in containers {
            let next_baseline = CounterBaseline::from_sample(&sample);
            let prev = self.baselines.get(&sample.name).copied();
            if let Some(existing) = self.baselines.get_mut(&sample.name) {
                *existing = next_baseline;
            } else {
                self.baselines.insert(sample.name.clone(), next_baseline);
            }
            if let Some(entry) = self.containers.get_mut(&sample.name) {
                entry.push(sample, prev);
            } else {
                let name = sample.name.clone();
                let mut entry = ContainerBucket::default();
                entry.push(sample, prev);
                self.containers.insert(name, entry);
            }
        }
        closed
    }

    /// Fecha o minuto corrente (encerramento do processo, por exemplo).
    pub fn close(&mut self) -> Option<MetricsRollup> {
        let bucket = self.bucket.take()?;
        self.last_closed = Some(bucket);
        let host = std::mem::take(&mut self.host);
        let containers = std::mem::take(&mut self.containers);
        let samples = std::mem::take(&mut self.samples);
        if samples == 0 {
            return None;
        }
        // Mantém baselines apenas de containers vistos neste balde para evitar
        // acúmulo infinito de containers removidos.
        self.baselines
            .retain(|name, _| containers.contains_key(name));
        Some(MetricsRollup {
            bucket_at: bucket,
            samples,
            host: host.finish(),
            containers: containers
                .into_iter()
                .filter_map(|(name, bucket)| bucket.finish(name))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(minute: u32, second: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 22, 10, minute, second)
            .single()
            .expect("data")
    }

    fn host(cpu: f64, memory: u64) -> HostSample {
        HostSample {
            cpu_percent: cpu,
            memory_used_bytes: memory,
            memory_total_bytes: 1_000,
            load1: 1.0,
            net_rx_bps: 10.0,
            net_tx_bps: 20.0,
            disks: vec![DiskUsage {
                mount: "/".into(),
                used_bytes: 5,
                total_bytes: 10,
            }],
        }
    }

    fn web(cpu: f64, rx: u64) -> ContainerSample {
        ContainerSample {
            name: "web".into(),
            project: Some("portal".into()),
            cpu_percent: cpu,
            memory_bytes: 100,
            net_rx_bytes: rx,
            net_tx_bytes: 0,
            block_read_bytes: 0,
            block_write_bytes: 0,
        }
    }

    #[test]
    fn fecha_o_minuto_quando_chega_amostra_do_seguinte() {
        let mut acc = RollupAccumulator::default();
        assert!(acc
            .push(at(0, 5), Some(host(10.0, 200)), vec![web(1.0, 100)])
            .is_none());
        assert!(acc
            .push(at(0, 45), Some(host(30.0, 400)), vec![web(3.0, 400)])
            .is_none());
        let closed = acc
            .push(at(1, 2), Some(host(50.0, 600)), vec![web(5.0, 500)])
            .expect("minuto fechado");

        assert_eq!(closed.bucket_at, at(0, 0));
        assert_eq!(closed.samples, 2);
        let host = closed.host.expect("host");
        assert!((host.cpu_avg - 20.0).abs() < f64::EPSILON);
        assert!((host.cpu_max - 30.0).abs() < f64::EPSILON);
        assert!((host.memory_used_avg - 300.0).abs() < f64::EPSILON);
        assert_eq!(closed.containers[0].net_rx_bytes, 300);

        let rest = acc.close().expect("minuto corrente");
        assert_eq!(rest.bucket_at, at(1, 0));
        assert_eq!(rest.samples, 1);
        // Preserva o delta entre a última amostra do minuto 0 (400) e a primeira do minuto 1 (500).
        assert_eq!(rest.containers[0].net_rx_bytes, 100);
        assert!(acc.close().is_none());
    }

    #[test]
    fn contador_que_zera_conta_desde_o_reinicio() {
        let mut acc = RollupAccumulator::default();
        acc.push(at(0, 5), None, vec![web(0.0, 1_000)]);
        acc.push(at(0, 15), None, vec![web(0.0, 50)]);
        acc.push(at(0, 25), None, vec![web(0.0, 80)]);
        let closed = acc.close().expect("fechado");
        assert_eq!(closed.containers[0].net_rx_bytes, 80);
    }

    #[test]
    fn amostra_com_relogio_atrasado_e_ignorada_sem_fechar_balde() {
        let mut acc = RollupAccumulator::default();
        assert!(acc.push(at(2, 10), None, vec![web(1.0, 100)]).is_none());
        assert!(acc.push(at(1, 59), None, vec![web(9.0, 999)]).is_none());
        assert!(acc.push(at(2, 20), None, vec![web(3.0, 250)]).is_none());
        let closed = acc.close().expect("fechado");
        assert_eq!(closed.bucket_at, at(2, 0));
        assert_eq!(closed.samples, 2);
        assert_eq!(closed.containers[0].net_rx_bytes, 150);
    }

    #[test]
    fn recuo_grande_do_relogio_fecha_o_balde_e_recomeca() {
        let mut acc = RollupAccumulator::default();
        assert!(acc.push(at(30, 10), None, vec![web(1.0, 100)]).is_none());
        // NTP corrige um RTC 25 minutos adiantado.
        let closed = acc
            .push(at(5, 10), None, vec![web(2.0, 900)])
            .expect("o balde adiantado fecha");
        assert_eq!(closed.bucket_at, at(30, 0));
        assert!(acc.push(at(5, 20), None, vec![web(3.0, 950)]).is_none());
        let rest = acc.close().expect("as amostras novas não são descartadas");
        assert_eq!(rest.bucket_at, at(5, 0));
        assert_eq!(rest.samples, 2);
        // A base anterior ao recuo não vale: só o delta dentro do minuto novo.
        assert_eq!(rest.containers[0].net_rx_bytes, 50);
    }

    #[test]
    fn lacuna_de_minutos_descarta_a_base_dos_contadores() {
        let mut acc = RollupAccumulator::default();
        acc.push(at(0, 50), None, vec![web(0.0, 100)]);
        // Agente parado (ou Docker fora) por cinco minutos.
        let closed = acc
            .push(at(5, 10), None, vec![web(0.0, 10_000)])
            .expect("minuto 0 fechado");
        assert_eq!(closed.bucket_at, at(0, 0));
        acc.push(at(5, 20), None, vec![web(0.0, 10_040)]);
        let rest = acc.close().expect("minuto 5");
        assert_eq!(
            rest.containers[0].net_rx_bytes, 40,
            "sem o período parado num minuto só"
        );

        // Também quando o balde foi fechado por fora (encerramento).
        acc.push(at(9, 10), None, vec![web(0.0, 99_999)]);
        let after_close = acc.close().expect("minuto 9");
        assert_eq!(after_close.containers[0].net_rx_bytes, 0);
    }

    #[test]
    fn balde_trunca_no_minuto() {
        assert_eq!(bucket_of(at(7, 59)), at(7, 0));
    }
}
