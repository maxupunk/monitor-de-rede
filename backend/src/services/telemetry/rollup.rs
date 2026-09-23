//! Agregação das amostras de host e containers em baldes de 1 minuto.
//!
//! O agente amostra a cada ~10 s, mas só o balde fechado atravessa o canal e
//! vai ao banco: é o que mantém 30 dias de histórico num SQLite sem inflar.
//! Gauges (CPU, memória, load) viram média e máximo; contadores cumulativos
//! (rede, disco) viram o delta do minuto, tolerando reinício do container.

use std::collections::BTreeMap;

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::views::docker::DockerMetricsResponse;

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
pub struct RollupAccumulator {
    bucket: Option<DateTime<Utc>>,
    hosts: Vec<HostSample>,
    containers: BTreeMap<String, Vec<ContainerSample>>,
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
            Some(current) if current != bucket => self.close(),
            _ => None,
        };
        self.bucket = Some(bucket);
        self.samples += 1;
        if let Some(host) = host {
            self.hosts.push(host);
        }
        for sample in containers {
            self.containers
                .entry(sample.name.clone())
                .or_default()
                .push(sample);
        }
        closed
    }

    /// Fecha o minuto corrente (encerramento do processo, por exemplo).
    pub fn close(&mut self) -> Option<MetricsRollup> {
        let bucket = self.bucket.take()?;
        let hosts = std::mem::take(&mut self.hosts);
        let containers = std::mem::take(&mut self.containers);
        let samples = std::mem::take(&mut self.samples);
        if samples == 0 {
            return None;
        }
        Some(MetricsRollup {
            bucket_at: bucket,
            samples,
            host: host_rollup(&hosts),
            containers: containers
                .into_values()
                .filter_map(|samples| container_rollup(&samples))
                .collect(),
        })
    }
}

#[allow(clippy::cast_precision_loss)]
fn average(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0_u32), |(sum, count), value| (sum + value, count + 1));
    if count == 0 {
        0.0
    } else {
        sum / f64::from(count)
    }
}

fn maximum(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(0.0, f64::max)
}

#[allow(clippy::cast_precision_loss)]
fn host_rollup(samples: &[HostSample]) -> Option<HostRollup> {
    let last = samples.last()?;
    Some(HostRollup {
        cpu_avg: average(samples.iter().map(|s| s.cpu_percent)),
        cpu_max: maximum(samples.iter().map(|s| s.cpu_percent)),
        memory_used_avg: average(samples.iter().map(|s| s.memory_used_bytes as f64)),
        memory_total: last.memory_total_bytes,
        load1: average(samples.iter().map(|s| s.load1)),
        net_rx_bps: average(samples.iter().map(|s| s.net_rx_bps)),
        net_tx_bps: average(samples.iter().map(|s| s.net_tx_bps)),
        disks: last.disks.clone(),
    })
}

/// Delta de um contador cumulativo no minuto. Se o contador diminuiu, o
/// container reiniciou: o que foi contado desde então é o próprio valor.
fn counter_delta(samples: &[ContainerSample], read: impl Fn(&ContainerSample) -> u64) -> u64 {
    samples
        .windows(2)
        .map(|pair| {
            let (before, after) = (read(&pair[0]), read(&pair[1]));
            if after >= before {
                after - before
            } else {
                after
            }
        })
        .sum()
}

#[allow(clippy::cast_precision_loss)]
fn container_rollup(samples: &[ContainerSample]) -> Option<ContainerRollup> {
    let last = samples.last()?;
    Some(ContainerRollup {
        name: last.name.clone(),
        project: last.project.clone(),
        cpu_avg: average(samples.iter().map(|s| s.cpu_percent)),
        cpu_max: maximum(samples.iter().map(|s| s.cpu_percent)),
        memory_avg: average(samples.iter().map(|s| s.memory_bytes as f64)),
        memory_max: samples.iter().map(|s| s.memory_bytes).max().unwrap_or(0),
        net_rx_bytes: counter_delta(samples, |s| s.net_rx_bytes),
        net_tx_bytes: counter_delta(samples, |s| s.net_tx_bytes),
        block_read_bytes: counter_delta(samples, |s| s.block_read_bytes),
        block_write_bytes: counter_delta(samples, |s| s.block_write_bytes),
    })
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
        assert!(acc.close().is_none());
    }

    #[test]
    fn contador_que_zera_conta_desde_o_reinicio() {
        let samples = [web(0.0, 1_000), web(0.0, 50), web(0.0, 80)];
        assert_eq!(counter_delta(&samples, |s| s.net_rx_bytes), 80);
    }

    #[test]
    fn balde_trunca_no_minuto() {
        assert_eq!(bucket_of(at(7, 59)), at(7, 0));
    }
}
