//! CPU e memória das pontas **durante** a medição (ADR 015 §12).
//!
//! Vazão baixa com o processador no teto é outra conversa que vazão baixa com
//! o processador folgado — e a coleta periódica (minutos, ou a média de 1 min
//! do `hrProcessorLoad`) não enxerga um teste de 5 s. Aqui cada ponta é lida a
//! cada segundo enquanto a perna mede, no mesmo relógio das amostras de vazão:
//!
//! * equipamento com SNMP — os contadores brutos da UCD-SNMP-MIB
//!   (`ssCpuRaw*`, a diferença entre duas leituras dá o uso daquele segundo,
//!   softirq incluído) e a memória da mesma MIB, num `GET` só; sem a UCD
//!   (RouterOS), o `hrProcessorLoad` da HOST-RESOURCES;
//! * a central — o `/proc` do próprio processo ([`HostSourceLinux`]).
//!
//! Só leitura, sempre: nada aqui muda o equipamento.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::endpoint::{Endpoint, Role};
use crate::{
    models::devices,
    services::{
        monitoring::health::{
            contracts::HealthSource,
            series,
            sources::{HostSourceLinux, ProcRoot},
        },
        preferences,
        snmp::{
            client::{SnmpClient, SnmpValue},
            collectors,
            service::device_config_with,
        },
    },
};

/// Intervalo entre duas leituras de uma ponta.
pub const HEALTH_EVERY: Duration = Duration::from_secs(1);
/// Teto de uma leitura: atrasada, a de agora fica sem número.
const READ_TIMEOUT: Duration = Duration::from_millis(900);
/// Leituras guardadas por ponta numa perna (2 min a 1 por segundo).
pub const MAX_HEALTH_PER_DEVICE: usize = 120;

/// Uma leitura de uma ponta: uso de CPU e de memória (%), no relógio da
/// perna (`atMs`, o mesmo de [`super::model::LinkTestSample::at_ms`]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestHealthSample {
    #[ts(type = "number")]
    pub device_id: i64,
    #[ts(type = "number")]
    pub at_ms: u64,
    pub cpu: Option<f64>,
    pub memory: Option<f64>,
}

/// Evento efêmero `link_test:health`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct LinkTestHealthEvent {
    #[ts(type = "number")]
    pub test_id: i64,
    pub leg_seq: u32,
    #[serde(flatten)]
    #[ts(flatten)]
    pub sample: LinkTestHealthSample,
}

/// O que uma leitura trouxe.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HealthReading {
    pub cpu: Option<f64>,
    pub memory: Option<f64>,
}

impl HealthReading {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.cpu.is_none() && self.memory.is_none()
    }
}

/// Lê uma ponta. Guarda o que precisa entre leituras (contadores de CPU).
#[async_trait]
pub trait HealthReader: Send {
    /// Nunca falha: o que não deu para ler vem `None`.
    async fn read(&mut self) -> HealthReading;
}

/// Quem será lido: a central ou um equipamento (ponta ou no meio do caminho).
#[derive(Clone, Copy)]
pub enum HealthTarget<'a> {
    Central,
    Device(&'a devices::Model),
}

impl<'a> HealthTarget<'a> {
    /// A ponta de uma perna (o agente remoto não é lido).
    #[must_use]
    pub fn of_endpoint(endpoint: &'a Endpoint) -> Option<Self> {
        match &endpoint.role {
            Role::Central { .. } => Some(Self::Central),
            Role::Device { device, .. } => Some(Self::Device(device)),
            Role::Agent { .. } => None,
        }
    }
}

/// Quem sabe ler cada equipamento (inversão de dependência: o teste do
/// orquestrador usa um dublê, sem SNMP).
#[async_trait]
pub trait HealthReaders: Send + Sync {
    /// `None` quando não há como ler (sem SNMP, sem IP).
    async fn reader_for(&self, target: HealthTarget<'_>) -> Option<Box<dyn HealthReader>>;
}

// --- Contadores da UCD-SNMP-MIB ------------------------------------------------

const OID_CPU_USER: &str = "1.3.6.1.4.1.2021.11.50.0";
const OID_CPU_NICE: &str = "1.3.6.1.4.1.2021.11.51.0";
const OID_CPU_SYSTEM: &str = "1.3.6.1.4.1.2021.11.52.0";
const OID_CPU_IDLE: &str = "1.3.6.1.4.1.2021.11.53.0";
const OID_CPU_WAIT: &str = "1.3.6.1.4.1.2021.11.54.0";

const HEALTH_OIDS: [&str; 10] = [
    OID_CPU_USER,
    OID_CPU_NICE,
    OID_CPU_SYSTEM,
    OID_CPU_IDLE,
    OID_CPU_WAIT,
    collectors::OID_MEM_TOTAL_REAL,
    collectors::OID_MEM_AVAIL_REAL,
    collectors::OID_MEM_BUFFER,
    collectors::OID_MEM_CACHED,
    collectors::OID_MEM_SYS_AVAILABLE,
];

/// Contadores acumulados de CPU (em ticks): o ocupado e o total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTicks {
    pub busy: u64,
    pub total: u64,
}

fn number(values: &BTreeMap<String, Option<SnmpValue>>, oid: &str) -> Option<u64> {
    values
        .get(oid)
        .and_then(Option::as_ref)
        .and_then(SnmpValue::number)
}

/// Os ticks da UCD. No Linux, `ssCpuRawSystem` já soma interrupção e softirq
/// (é onde o tráfego pesa num roteador): somar `ssCpuRawInterrupt` e
/// `ssCpuRawSoftIRQ` de novo contaria duas vezes. `None` sem a MIB.
#[must_use]
pub fn ucd_ticks(values: &BTreeMap<String, Option<SnmpValue>>) -> Option<CpuTicks> {
    let user = number(values, OID_CPU_USER)?;
    let system = number(values, OID_CPU_SYSTEM)?;
    let idle = number(values, OID_CPU_IDLE)?;
    let nice = number(values, OID_CPU_NICE).unwrap_or(0);
    let wait = number(values, OID_CPU_WAIT).unwrap_or(0);
    let busy = user.saturating_add(nice).saturating_add(system);
    Some(CpuTicks {
        busy,
        total: busy.saturating_add(idle).saturating_add(wait),
    })
}

/// Uso de CPU (%) entre duas leituras. `None` sem tempo passado ou com
/// contador que voltou (reinício, estouro).
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn cpu_between(before: CpuTicks, after: CpuTicks) -> Option<f64> {
    let total = after.total.checked_sub(before.total)?;
    let busy = after.busy.checked_sub(before.busy)?;
    if total == 0 {
        return None;
    }
    Some((busy as f64 / total as f64 * 100.0).clamp(0.0, 100.0))
}

/// O CPU entre leituras de contador. O net-snmp guarda os `ssCpuRaw*` em
/// cache (~5 s): duas leituras no mesmo segundo trazem o mesmo número. A
/// medida vale da última vez que o contador andou até agora, e enquanto ele
/// não anda o último uso medido continua sendo o atual.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CpuWindow {
    base: Option<CpuTicks>,
    last: Option<f64>,
}

impl CpuWindow {
    /// Uma leitura nova; devolve o uso de CPU atual (ou `None` ainda sem dois
    /// contadores diferentes).
    pub fn advance(&mut self, now: CpuTicks) -> Option<f64> {
        match self.base {
            Some(base) if base.total == now.total => {}
            Some(base) => {
                // Contador que voltou (reinício): recomeça do zero.
                self.last = cpu_between(base, now);
                self.base = Some(now);
            }
            None => self.base = Some(now),
        }
        self.last
    }
}

/// Uma ponta lida por SNMP.
pub struct SnmpHealth {
    client: SnmpClient,
    cpu: CpuWindow,
    /// A UCD não respondeu: o CPU vem do `hrProcessorLoad`.
    host_resources: bool,
}

impl SnmpHealth {
    #[must_use]
    pub fn new(client: SnmpClient) -> Self {
        Self {
            client,
            cpu: CpuWindow::default(),
            host_resources: false,
        }
    }
}

#[async_trait]
impl HealthReader for SnmpHealth {
    async fn read(&mut self) -> HealthReading {
        let Ok(values) = self.client.get(&HEALTH_OIDS).await else {
            return HealthReading::default();
        };
        let memory = collectors::memory_info(
            number(&values, collectors::OID_MEM_TOTAL_REAL),
            number(&values, collectors::OID_MEM_SYS_AVAILABLE),
            number(&values, collectors::OID_MEM_AVAIL_REAL),
            number(&values, collectors::OID_MEM_BUFFER),
            number(&values, collectors::OID_MEM_CACHED),
        )
        .used_percent;
        let cpu = match ucd_ticks(&values) {
            Some(now) => self.cpu.advance(now),
            None => {
                self.host_resources = true;
                None
            }
        };
        let cpu = if self.host_resources {
            collectors::collect_cpu(&self.client)
                .await
                .ok()
                .and_then(|info| info.usage_percent)
        } else {
            cpu
        };
        HealthReading { cpu, memory }
    }
}

/// A própria central, pelo `/proc`.
pub struct LocalHealth(HostSourceLinux);

#[async_trait]
impl HealthReader for LocalHealth {
    async fn read(&mut self) -> HealthReading {
        let reading = self.0.read();
        let value = |name: &str| {
            reading
                .measures
                .iter()
                .find(|measure| measure.name == name)
                .map(|measure| measure.value)
        };
        HealthReading {
            cpu: value(series::CPU_USAGE),
            memory: value(series::MEMORY_USAGE),
        }
    }
}

/// O padrão: SNMP no equipamento que o tem ligado, `/proc` na central.
pub struct DefaultHealthReaders {
    db: DatabaseConnection,
}

impl DefaultHealthReaders {
    #[must_use]
    pub const fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl HealthReaders for DefaultHealthReaders {
    async fn reader_for(&self, target: HealthTarget<'_>) -> Option<Box<dyn HealthReader>> {
        match target {
            HealthTarget::Central => Some(Box::new(LocalHealth(HostSourceLinux::new(
                ProcRoot::default(),
            )))),
            HealthTarget::Device(device) => {
                let has_ip = device
                    .ip_address
                    .as_deref()
                    .is_some_and(|ip| ip.trim().parse::<std::net::IpAddr>().is_ok());
                if !device.snmp_enabled || !has_ip {
                    return None;
                }
                let community = preferences::load(&self.db).await.map_or_else(
                    |_| preferences::DEFAULT_SNMP_COMMUNITY.to_owned(),
                    |prefs| prefs.default_snmp_community,
                );
                let mut config = device_config_with(device, &community).ok()?;
                config.timeout_ms = u64::try_from(READ_TIMEOUT.as_millis()).unwrap_or(900);
                Some(Box::new(SnmpHealth::new(SnmpClient::new(config))))
            }
        }
    }
}

/// As pontas de uma perna sendo lidas a cada [`HEALTH_EVERY`]; parar é
/// cancelar e esperar o laço sair.
pub struct HealthSampler {
    stop: CancellationToken,
    task: Option<JoinHandle<()>>,
}

impl HealthSampler {
    /// Lê `readers` (id do equipamento → leitor) até [`Self::stop`]; cada
    /// leitura vai para `sink` com o instante dela no relógio da perna.
    pub fn start<F>(
        readers: Vec<(i64, Box<dyn HealthReader>)>,
        clock: F,
        sink: Arc<dyn Fn(LinkTestHealthSample) + Send + Sync>,
    ) -> Self
    where
        F: Fn() -> u64 + Send + Sync + 'static,
    {
        let stop = CancellationToken::new();
        if readers.is_empty() {
            return Self { stop, task: None };
        }
        let token = stop.clone();
        let task = tokio::spawn(async move {
            let mut readers = readers;
            let mut tick = tokio::time::interval(HEALTH_EVERY);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    () = token.cancelled() => break,
                    _ = tick.tick() => {}
                }
                let at_ms = clock();
                let reads = readers.iter_mut().map(|(device_id, reader)| {
                    let device_id = *device_id;
                    async move {
                        let reading = tokio::time::timeout(READ_TIMEOUT, reader.read())
                            .await
                            .unwrap_or_default();
                        (device_id, reading)
                    }
                });
                for (device_id, reading) in futures::future::join_all(reads).await {
                    if !reading.is_empty() {
                        sink(LinkTestHealthSample {
                            device_id,
                            at_ms,
                            cpu: reading.cpu,
                            memory: reading.memory,
                        });
                    }
                }
            }
        });
        Self {
            stop,
            task: Some(task),
        }
    }

    /// Para de ler e espera a última leitura em curso.
    pub async fn stop(mut self) {
        self.stop.cancel();
        if let Some(task) = self.task.take() {
            let abort = task.abort_handle();
            if tokio::time::timeout(READ_TIMEOUT * 2, task).await.is_err() {
                abort.abort();
            }
        }
    }
}

impl Drop for HealthSampler {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valores(pares: &[(&str, u64)]) -> BTreeMap<String, Option<SnmpValue>> {
        pares
            .iter()
            .map(|(oid, valor)| ((*oid).to_owned(), Some(SnmpValue::Number(*valor))))
            .collect()
    }

    #[test]
    fn ticks_da_ucd_sem_contar_softirq_duas_vezes() {
        // BORDA de verdade: system já soma interrupção e softirq.
        let ticks = ucd_ticks(&valores(&[
            (OID_CPU_USER, 73_471),
            (OID_CPU_NICE, 0),
            (OID_CPU_SYSTEM, 127_295),
            (OID_CPU_IDLE, 13_325_983),
            (OID_CPU_WAIT, 3_394),
            ("1.3.6.1.4.1.2021.11.61.0", 94_285),
        ]))
        .expect("UCD");
        assert_eq!(ticks.busy, 73_471 + 127_295);
        assert_eq!(ticks.total, 73_471 + 127_295 + 13_325_983 + 3_394);
    }

    #[test]
    fn sem_ucd_nao_ha_ticks() {
        // RouterOS: noSuchObject vira valor ausente.
        let mut sem = valores(&[]);
        sem.insert(OID_CPU_USER.into(), None);
        assert_eq!(ucd_ticks(&sem), None);
    }

    #[test]
    fn uso_de_cpu_e_a_diferenca_entre_duas_leituras() {
        let antes = CpuTicks {
            busy: 1_000,
            total: 10_000,
        };
        let depois = CpuTicks {
            busy: 1_075,
            total: 10_100,
        };
        assert_eq!(cpu_between(antes, depois), Some(75.0));
        // Sem tempo passado, ou contador que voltou (reinício): sem número.
        assert_eq!(cpu_between(antes, antes), None);
        assert_eq!(
            cpu_between(
                antes,
                CpuTicks {
                    busy: 10,
                    total: 100
                }
            ),
            None
        );
    }

    #[test]
    fn contador_em_cache_repete_o_ultimo_uso_ate_andar() {
        let ticks = |busy, total| CpuTicks { busy, total };
        let mut janela = CpuWindow::default();
        assert_eq!(
            janela.advance(ticks(100, 1_000)),
            None,
            "primeira leitura: base"
        );
        assert_eq!(
            janela.advance(ticks(100, 1_000)),
            None,
            "cache do agente: nada andou"
        );
        assert_eq!(janela.advance(ticks(190, 1_100)), Some(90.0));
        // O agente ainda não renovou: o uso medido continua valendo.
        assert_eq!(janela.advance(ticks(190, 1_100)), Some(90.0));
        assert_eq!(janela.advance(ticks(200, 1_200)), Some(10.0));
        // Reinício do equipamento: o contador volta e a medida recomeça.
        assert_eq!(janela.advance(ticks(5, 50)), None);
        assert_eq!(janela.advance(ticks(55, 150)), Some(50.0));
    }

    struct Fixo(HealthReading);

    #[async_trait]
    impl HealthReader for Fixo {
        async fn read(&mut self) -> HealthReading {
            self.0
        }
    }

    #[tokio::test]
    async fn amostrador_le_cada_ponta_e_para_quando_mandado() {
        let lidas = Arc::new(std::sync::Mutex::new(Vec::new()));
        let destino = Arc::clone(&lidas);
        let sampler = HealthSampler::start(
            vec![
                (
                    7,
                    Box::new(Fixo(HealthReading {
                        cpu: Some(42.0),
                        memory: Some(60.0),
                    })) as Box<dyn HealthReader>,
                ),
                (8, Box::new(Fixo(HealthReading::default()))),
            ],
            || 1_000,
            Arc::new(move |amostra| destino.lock().unwrap().push(amostra)),
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        sampler.stop().await;
        let lidas = lidas.lock().unwrap().clone();
        assert!(!lidas.is_empty());
        // Ponta sem nada lido não vira amostra vazia.
        assert!(lidas.iter().all(|amostra| amostra.device_id == 7));
        assert_eq!(
            lidas[0],
            LinkTestHealthSample {
                device_id: 7,
                at_ms: 1_000,
                cpu: Some(42.0),
                memory: Some(60.0),
            }
        );
    }
}
