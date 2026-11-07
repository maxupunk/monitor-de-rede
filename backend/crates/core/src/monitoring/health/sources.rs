//! As fontes concretas de saúde local.
//!
//! Cada uma implementa [`HealthSource`] e sabe ler **um** assunto. Toda a
//! lógica de interpretação vive em [`super::parsers`], testada com fixtures;
//! aqui só se abre arquivo e se traduz ausência em indisponibilidade.
//!
//! # Portabilidade
//!
//! O alvo de produção é Linux em container. Em qualquer outro sistema os
//! arquivos simplesmente não existem, e o resultado é "indisponível" com o
//! motivo — nunca um zero inventado. É por isso que não há `#[cfg(target_os)]`
//! espalhado: a ausência do arquivo já é o caminho correto.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use super::{
    contracts::{HealthSource, Measure, MeasureSource, Reading},
    parsers::{self, CpuTimes, NetTotals},
    series,
};

/// Raiz do `procfs`. Injetável para que os testes leiam uma fixture em disco
/// em vez do `/proc` da máquina que roda a suíte.
#[derive(Debug, Clone)]
pub struct ProcRoot(PathBuf);

impl Default for ProcRoot {
    fn default() -> Self {
        Self(PathBuf::from("/proc"))
    }
}

impl ProcRoot {
    #[must_use]
    pub fn new(raiz: impl Into<PathBuf>) -> Self {
        Self(raiz.into())
    }

    fn read(&self, relativo: &str) -> Option<String> {
        std::fs::read_to_string(self.0.join(relativo)).ok()
    }
}

/// Uma leitura anterior guardada para calcular deltas.
///
/// CPU e tráfego são contadores acumulados: só fazem sentido comparados com a
/// amostra anterior. O estado é do processo, não do banco — reiniciar o
/// serviço custa uma amostra, e é mais barato que uma tabela.
#[derive(Debug, Default)]
struct Previous {
    cpu: Option<CpuTimes>,
    net: Option<(NetTotals, chrono::DateTime<chrono::Utc>)>,
}

/// CPU, memória, carga, uptime e tráfego do host.
pub struct HostSourceLinux {
    proc: ProcRoot,
    previous: Mutex<Previous>,
    now: fn() -> chrono::DateTime<chrono::Utc>,
}

impl HostSourceLinux {
    #[must_use]
    pub fn new(proc: ProcRoot) -> Self {
        Self {
            proc,
            previous: Mutex::new(Previous::default()),
            now: chrono::Utc::now,
        }
    }

    /// Versão com relógio injetado, para testes de taxa determinísticos.
    #[must_use]
    pub fn with_clock(proc: ProcRoot, now: fn() -> chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            proc,
            previous: Mutex::new(Previous::default()),
            now,
        }
    }
}

impl HealthSource for HostSourceLinux {
    fn name(&self) -> &'static str {
        "host"
    }

    #[allow(clippy::cast_precision_loss)]
    fn read(&self) -> Reading {
        let mut reading = Reading::default();
        // Um `Mutex` envenenado por um panic em outra thread não pode calar a
        // coleta inteira: recuperamos o estado e seguimos.
        let mut previous = self
            .previous
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // --- CPU: delta entre duas leituras ---
        match self
            .proc
            .read("stat")
            .as_deref()
            .and_then(parsers::parse_proc_stat)
        {
            Some(atual) => {
                match previous
                    .cpu
                    .and_then(|antes| parsers::cpu_usage_percent(antes, atual))
                {
                    Some(uso) => {
                        reading = reading.measure(Measure::new(
                            series::CPU_USAGE,
                            uso,
                            "percent",
                            MeasureSource::Host,
                        ))
                    }
                    None => {
                        reading = reading.missing(
                            series::CPU_USAGE,
                            "primeira amostra desde o início do processo: o uso de CPU é a diferença entre duas leituras",
                        );
                    }
                }
                previous.cpu = Some(atual);
            }
            None => {
                reading = reading.missing(series::CPU_USAGE, "/proc/stat indisponível ou ilegível");
            }
        }

        // --- Memória do host ---
        match self
            .proc
            .read("meminfo")
            .as_deref()
            .and_then(parsers::parse_meminfo)
        {
            Some(info) => match info.used_percent() {
                Some(percentual) => {
                    reading = reading
                        .measure(Measure::new(
                            series::MEMORY_USAGE,
                            percentual,
                            "percent",
                            MeasureSource::Host,
                        ))
                        .measure(Measure::new(
                            series::MEMORY_USED_BYTES,
                            info.used_bytes() as f64,
                            "bytes",
                            MeasureSource::Host,
                        ))
                        .measure(Measure::new(
                            series::MEMORY_TOTAL_BYTES,
                            info.total_bytes as f64,
                            "bytes",
                            MeasureSource::Host,
                        ));
                }
                None => {
                    reading = reading.missing(
                        series::MEMORY_USAGE,
                        "/proc/meminfo sem MemTotal utilizável",
                    );
                }
            },
            None => {
                reading = reading.missing(
                    series::MEMORY_USAGE,
                    "/proc/meminfo indisponível ou ilegível",
                );
            }
        }

        // --- Carga ---
        match self
            .proc
            .read("loadavg")
            .as_deref()
            .and_then(parsers::parse_loadavg_1m)
        {
            Some(carga) => {
                reading = reading.measure(Measure::new(
                    series::LOAD_AVERAGE_1M,
                    carga,
                    "processes",
                    MeasureSource::Host,
                ));
            }
            None => {
                reading = reading.missing(
                    series::LOAD_AVERAGE_1M,
                    "/proc/loadavg indisponível ou ilegível",
                );
            }
        }

        // --- Uptime ---
        match self
            .proc
            .read("uptime")
            .as_deref()
            .and_then(parsers::parse_uptime_seconds)
        {
            Some(segundos) => {
                reading = reading.measure(Measure::new(
                    series::UPTIME_SECONDS,
                    segundos,
                    "seconds",
                    MeasureSource::Host,
                ));
            }
            None => {
                reading = reading.missing(
                    series::UPTIME_SECONDS,
                    "/proc/uptime indisponível ou ilegível",
                );
            }
        }

        // --- Tráfego agregado: também um delta ---
        let agora = (self.now)();
        match self
            .proc
            .read("net/dev")
            .as_deref()
            .and_then(parsers::parse_net_dev)
        {
            Some(atual) => {
                match previous.net {
                    Some((antes, quando)) => {
                        #[allow(clippy::cast_precision_loss)]
                        let segundos = (agora - quando).num_milliseconds() as f64 / 1_000.0;
                        let entrada =
                            parsers::rate_per_second(antes.rx_bytes, atual.rx_bytes, segundos);
                        let saida =
                            parsers::rate_per_second(antes.tx_bytes, atual.tx_bytes, segundos);
                        match (entrada, saida) {
                            (Some(entrada), Some(saida)) => {
                                reading = reading
                                    .measure(Measure::new(
                                        series::IN_BPS,
                                        entrada,
                                        "bps",
                                        MeasureSource::Host,
                                    ))
                                    .measure(Measure::new(
                                        series::OUT_BPS,
                                        saida,
                                        "bps",
                                        MeasureSource::Host,
                                    ));
                            }
                            _ => {
                                reading = reading.missing(
                                    series::IN_BPS,
                                    "contador de rede reiniciado ou intervalo nulo entre amostras",
                                );
                            }
                        }
                    }
                    None => {
                        reading = reading.missing(
                            series::IN_BPS,
                            "primeira amostra desde o início do processo: o tráfego é a diferença entre duas leituras",
                        );
                    }
                }
                previous.net = Some((atual, agora));
            }
            None => {
                reading = reading.missing(
                    series::IN_BPS,
                    "/proc/net/dev indisponível ou sem interface além do loopback",
                );
            }
        }

        reading
    }
}

/// Memória do **próprio processo**.
pub struct ProcessSourceLinux {
    proc: ProcRoot,
}

impl ProcessSourceLinux {
    #[must_use]
    pub fn new(proc: ProcRoot) -> Self {
        Self { proc }
    }
}

impl HealthSource for ProcessSourceLinux {
    fn name(&self) -> &'static str {
        "process"
    }

    fn read(&self) -> Reading {
        match self
            .proc
            .read("self/status")
            .as_deref()
            .and_then(parsers::parse_process_rss_bytes)
        {
            #[allow(clippy::cast_precision_loss)]
            Some(bytes) => Reading::default().measure(Measure::new(
                series::PROCESS_MEMORY_BYTES,
                bytes as f64,
                "bytes",
                MeasureSource::Process,
            )),
            None => Reading::default().missing(
                series::PROCESS_MEMORY_BYTES,
                "/proc/self/status sem VmRSS: indisponível neste sistema",
            ),
        }
    }
}

/// Memória vista de **dentro do container**, quando há limite de cgroup.
///
/// Só publica algo quando existe limite: sem limite, o número correto é o do
/// host, e duas fontes respondendo a mesma pergunta com valores diferentes é
/// pior que uma só. A precedência é resolvida no coordenador.
pub struct CgroupSourceLinux {
    v2: PathBuf,
    v1: PathBuf,
}

impl Default for CgroupSourceLinux {
    fn default() -> Self {
        Self::new("/sys/fs/cgroup", "/sys/fs/cgroup/memory")
    }
}

impl CgroupSourceLinux {
    #[must_use]
    pub fn new(v2: impl Into<PathBuf>, v1: impl Into<PathBuf>) -> Self {
        Self {
            v2: v2.into(),
            v1: v1.into(),
        }
    }

    fn ler(base: &Path, arquivo: &str) -> Option<String> {
        std::fs::read_to_string(base.join(arquivo)).ok()
    }

    /// Bytes usados e limite do cgroup, se houver limite.
    fn memory_usage(&self) -> Option<(u64, u64)> {
        // v2 primeiro: é o padrão nas distribuições atuais.
        let v2 = Self::ler(&self.v2, "memory.max")
            .as_deref()
            .and_then(parsers::parse_cgroup_limit)
            .zip(
                Self::ler(&self.v2, "memory.current")
                    .as_deref()
                    .and_then(parsers::parse_cgroup_usage),
            )
            .map(|(limite, usado)| {
                let cache = Self::ler(&self.v2, "memory.stat")
                    .as_deref()
                    .and_then(parsers::parse_cgroup_v2_inactive_file)
                    .unwrap_or(0);
                (limite, usado.saturating_sub(cache))
            });

        let v1 = || {
            Self::ler(&self.v1, "memory.limit_in_bytes")
                .as_deref()
                .and_then(parsers::parse_cgroup_limit)
                .zip(
                    Self::ler(&self.v1, "memory.usage_in_bytes")
                        .as_deref()
                        .and_then(parsers::parse_cgroup_usage),
                )
                .map(|(limite, usado)| {
                    let cache = Self::ler(&self.v1, "memory.stat")
                        .as_deref()
                        .and_then(parsers::parse_cgroup_v1_total_inactive_file)
                        .unwrap_or(0);
                    (limite, usado.saturating_sub(cache))
                })
        };

        v2.or_else(v1).filter(|(limite, _)| *limite > 0)
    }
}

impl HealthSource for CgroupSourceLinux {
    fn name(&self) -> &'static str {
        "cgroup"
    }

    #[allow(clippy::cast_precision_loss)]
    fn read(&self) -> Reading {
        match self.memory_usage() {
            Some((limite, usado)) => {
                #[allow(clippy::cast_precision_loss)]
                let percentual = (usado as f64 / limite as f64) * 100.0;
                Reading::default()
                    .measure(Measure::new(
                        series::MEMORY_USAGE,
                        percentual,
                        "percent",
                        MeasureSource::Cgroup,
                    ))
                    .measure(Measure::new(
                        series::MEMORY_USED_BYTES,
                        usado as f64,
                        "bytes",
                        MeasureSource::Cgroup,
                    ))
                    .measure(Measure::new(
                        series::MEMORY_TOTAL_BYTES,
                        limite as f64,
                        "bytes",
                        MeasureSource::Cgroup,
                    ))
            }
            // Não é indisponibilidade: é ausência de limite, e nesse caso a
            // resposta certa é a do host. Registrar "indisponível" aqui faria
            // a tela mostrar um aviso para uma instalação perfeitamente sadia.
            None => Reading::default(),
        }
    }
}

/// Uso do sistema de arquivos que hospeda os dados.
pub struct StorageSource {
    caminho: PathBuf,
}

impl Default for StorageSource {
    fn default() -> Self {
        // O diretório de trabalho é onde o SQLite e os arquivos do serviço
        // vivem — é esse o volume cujo enchimento derruba o produto.
        Self {
            caminho: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

impl StorageSource {
    #[must_use]
    pub fn new(caminho: impl Into<PathBuf>) -> Self {
        Self {
            caminho: caminho.into(),
        }
    }
}

impl HealthSource for StorageSource {
    fn name(&self) -> &'static str {
        "storage"
    }

    fn read(&self) -> Reading {
        match statvfs_used_percent(&self.caminho) {
            Some(percentual) => Reading::default().measure(Measure::new(
                series::STORAGE_USAGE,
                percentual,
                "percent",
                MeasureSource::Host,
            )),
            None => Reading::default().missing(
                series::STORAGE_USAGE,
                "não foi possível consultar o sistema de arquivos neste sistema operacional",
            ),
        }
    }
}

/// Percentual usado do volume, pela via do sistema operacional.
///
/// O cálculo usa `blocks - bfree` sobre `blocks`, e não `bavail`: o espaço
/// reservado ao root está ocupado do ponto de vista de quem quer gravar, mas
/// contá-lo como indisponível faria o percentual passar de 100%.
#[cfg(unix)]
fn statvfs_used_percent(caminho: &Path) -> Option<f64> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = caminho.as_os_str().as_bytes();
    let c_path = std::ffi::CString::new(bytes).ok()?;
    // SAFETY: `c_path` é um ponteiro válido, terminado em nul, vivo durante a
    // chamada; `stat` é escrito pelo kernel e só é lido após sucesso.
    let stat = unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c_path.as_ptr(), &raw mut stat) != 0 {
            return None;
        }
        stat
    };
    let total = u64::from(stat.f_blocks);
    let livre = u64::from(stat.f_bfree);
    if total == 0 || livre > total {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    Some(((total - livre) as f64 / total as f64) * 100.0)
}

#[cfg(not(unix))]
fn statvfs_used_percent(_caminho: &Path) -> Option<f64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta uma árvore de fixtures em disco e devolve a raiz.
    fn fixtures(arquivos: &[(&str, &str)]) -> tempdir::Fixture {
        tempdir::Fixture::new(arquivos)
    }

    /// Diretório temporário mínimo — o suficiente para escrever fixtures de
    /// `/proc` sem trazer uma dependência nova para a árvore.
    mod tempdir {
        use std::path::{Path, PathBuf};

        pub struct Fixture(PathBuf);

        impl Fixture {
            pub fn new(arquivos: &[(&str, &str)]) -> Self {
                // Um contador global, e não o id da thread: dois `Fixture`
                // vivos no mesmo teste precisam de diretórios distintos, e
                // reaproveitar o caminho faria o segundo apagar o primeiro.
                static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let raiz = std::env::temp_dir().join(format!(
                    "netmonitor-health-{}-{}",
                    std::process::id(),
                    SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                let _ = std::fs::remove_dir_all(&raiz);
                for (nome, conteudo) in arquivos {
                    let caminho = raiz.join(nome);
                    if let Some(pai) = caminho.parent() {
                        std::fs::create_dir_all(pai).expect("criar diretório da fixture");
                    }
                    std::fs::write(&caminho, conteudo).expect("escrever fixture");
                }
                Self(raiz)
            }

            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    const STAT_A: &str = "cpu  100 0 0 900 0 0 0 0 0 0\n";
    const STAT_B: &str = "cpu  200 0 0 1700 0 0 0 0 0 0\n";

    fn achou<'a>(reading: &'a Reading, nome: &str) -> Option<&'a Measure> {
        reading.measures.iter().find(|m| m.name == nome)
    }

    fn faltou(reading: &Reading, nome: &str) -> bool {
        reading.unavailable.iter().any(|u| u.name == nome)
    }

    #[test]
    fn a_primeira_amostra_de_cpu_e_indisponivel_e_a_segunda_mede() {
        let fx = fixtures(&[
            ("stat", STAT_A),
            ("meminfo", "MemTotal: 1000 kB\nMemAvailable: 250 kB\n"),
            ("loadavg", "1.5 1.0 0.5 1/2 3\n"),
            ("uptime", "3600.0 7200.0\n"),
        ]);
        let fonte = HostSourceLinux::new(ProcRoot::new(fx.path()));

        let primeira = fonte.read();
        assert!(
            faltou(&primeira, series::CPU_USAGE),
            "sem amostra anterior não existe uso de CPU — e zero seria mentira"
        );
        // O que não depende de delta já vem na primeira leitura.
        assert!((achou(&primeira, series::MEMORY_USAGE).unwrap().value - 75.0).abs() < 1e-9);
        assert_eq!(
            achou(&primeira, series::LOAD_AVERAGE_1M).unwrap().value,
            1.5
        );
        assert_eq!(
            achou(&primeira, series::UPTIME_SECONDS).unwrap().value,
            3600.0
        );

        std::fs::write(fx.path().join("stat"), STAT_B).unwrap();
        let segunda = fonte.read();
        // 900 jiffies passaram, 800 ociosos → 11,11%.
        let cpu = achou(&segunda, series::CPU_USAGE).expect("uso de CPU na segunda amostra");
        assert!((cpu.value - 100.0 / 9.0).abs() < 1e-6, "veio {}", cpu.value);
        assert_eq!(cpu.source, MeasureSource::Host);
    }

    #[test]
    fn arquivos_ausentes_viram_indisponibilidade_e_nunca_zero() {
        let fx = fixtures(&[("stat", STAT_A)]);
        let reading = HostSourceLinux::new(ProcRoot::new(fx.path())).read();
        for serie in [
            series::MEMORY_USAGE,
            series::LOAD_AVERAGE_1M,
            series::UPTIME_SECONDS,
        ] {
            assert!(faltou(&reading, serie), "{serie} deveria ser indisponível");
            assert!(
                achou(&reading, serie).is_none(),
                "{serie} não pode aparecer como medida"
            );
        }
    }

    #[test]
    fn os_cgroups_v2_e_v1_descontam_o_cache_de_arquivos() {
        let v2 = fixtures(&[
            ("memory.max", "1000\n"),
            ("memory.current", "800\n"),
            ("memory.stat", "anon 300\ninactive_file 300\n"),
        ]);
        let fonte = CgroupSourceLinux::new(v2.path(), v2.path().join("inexistente"));
        let reading = fonte.read();
        // (800 - 300) / 1000 = 50%.
        assert!((achou(&reading, series::MEMORY_USAGE).unwrap().value - 50.0).abs() < 1e-9);
        assert_eq!(
            achou(&reading, series::MEMORY_USAGE).unwrap().source,
            MeasureSource::Cgroup
        );

        let v1 = fixtures(&[
            ("memory.limit_in_bytes", "1000\n"),
            ("memory.usage_in_bytes", "800\n"),
            ("memory.stat", "cache 400\ntotal_inactive_file 300\n"),
        ]);
        let reading = CgroupSourceLinux::new(v1.path().join("inexistente"), v1.path()).read();
        assert!((achou(&reading, series::MEMORY_USAGE).unwrap().value - 50.0).abs() < 1e-9);
    }

    #[test]
    fn cgroup_sem_limite_nao_publica_nada_nem_reclama() {
        let fx = fixtures(&[("memory.max", "max\n"), ("memory.current", "800\n")]);
        let reading = CgroupSourceLinux::new(fx.path(), fx.path().join("v1")).read();
        assert!(
            reading.measures.is_empty() && reading.unavailable.is_empty(),
            "sem limite quem responde é o host; um aviso aqui alarmaria à toa"
        );
    }

    #[test]
    fn a_memoria_do_processo_sai_do_status_e_falta_com_motivo() {
        let com = fixtures(&[("self/status", "VmRSS:\t 2048 kB\n")]);
        let reading = ProcessSourceLinux::new(ProcRoot::new(com.path())).read();
        let medida = achou(&reading, series::PROCESS_MEMORY_BYTES).expect("rss");
        assert_eq!(medida.value, 2048.0 * 1024.0);
        assert_eq!(medida.source, MeasureSource::Process);

        let sem = fixtures(&[("self/status", "Name: backend\n")]);
        let reading = ProcessSourceLinux::new(ProcRoot::new(sem.path())).read();
        assert!(faltou(&reading, series::PROCESS_MEMORY_BYTES));
    }
}
