//! Métricas do host lidas de `/proc` e `statvfs`.
//!
//! Os parsers são funções puras sobre o texto dos arquivos, testadas sem
//! Linux. O leitor guarda a amostra anterior de CPU e rede porque as duas
//! métricas são diferenças entre leituras.
//!
//! No modo container o agente enxerga o host por montagens somente leitura:
//! `HOST_PROC=/host/proc` e `HOST_ROOT=/host` (`-v /proc:/host/proc:ro`,
//! `-v /:/host:ro`).

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use super::rollup::{DiskUsage, HostSample};

/// Tempos agregados de CPU (`/proc/stat`, linha `cpu`), em jiffies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    pub total: u64,
    pub idle: u64,
}

#[must_use]
pub fn parse_cpu_times(stat: &str) -> Option<CpuTimes> {
    let line = stat.lines().find(|line| line.starts_with("cpu "))?;
    let values: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse().ok())
        .collect();
    if values.len() < 4 {
        return None;
    }
    // guest e guest_nice já estão contados em user/nice: somá-los de novo
    // inflaria o total.
    let total: u64 = values.iter().take(8).sum();
    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    Some(CpuTimes { total, idle })
}

/// Percentual de CPU ocupada entre duas leituras.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn cpu_percent(before: CpuTimes, after: CpuTimes) -> f64 {
    let total = after.total.saturating_sub(before.total);
    if total == 0 {
        return 0.0;
    }
    let idle = after.idle.saturating_sub(before.idle).min(total);
    ((total - idle) as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
}

/// `(usado, total)` em bytes. Usado = total − `MemAvailable`, que já desconta
/// cache recuperável — o mesmo critério dos widgets de RAM.
#[must_use]
pub fn parse_meminfo(meminfo: &str) -> Option<(u64, u64)> {
    let field = |name: &str| {
        meminfo
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
            .map(|kib| kib * 1024)
    };
    let total = field("MemTotal:")?;
    let available = field("MemAvailable:").or_else(|| field("MemFree:"))?;
    Some((total.saturating_sub(available), total))
}

#[must_use]
pub fn parse_loadavg(loadavg: &str) -> Option<f64> {
    loadavg.split_whitespace().next()?.parse().ok()
}

/// Interfaces virtuais do Docker e do kernel não são tráfego do host: o
/// mesmo pacote apareceria duas vezes (veth + bridge).
fn is_virtual_interface(name: &str) -> bool {
    name == "lo"
        || [
            "veth", "docker", "br-", "virbr", "cni", "flannel", "cali", "vxlan",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Soma `(rx, tx)` em bytes das interfaces reais (`/proc/net/dev`).
#[must_use]
pub fn parse_net_dev(net_dev: &str) -> (u64, u64) {
    net_dev
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| !is_virtual_interface(name.trim()))
        .filter_map(|(_, counters)| {
            let values: Vec<u64> = counters
                .split_whitespace()
                .filter_map(|value| value.parse().ok())
                .collect();
            Some((*values.first()?, *values.get(8)?))
        })
        .fold((0, 0), |(rx, tx), (r, t)| (rx + r, tx + t))
}

const REAL_FILESYSTEMS: [&str; 8] = [
    "ext2", "ext3", "ext4", "xfs", "btrfs", "zfs", "f2fs", "vfat",
];

/// Pontos de montagem de sistemas de arquivos reais, sem repetir dispositivo.
#[must_use]
pub fn parse_mounts(mounts: &str) -> Vec<String> {
    let mut seen_devices = Vec::new();
    let mut output = Vec::new();
    for line in mounts.lines() {
        let mut fields = line.split_whitespace();
        let (Some(device), Some(mount), Some(fs)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if !REAL_FILESYSTEMS.contains(&fs) || seen_devices.iter().any(|seen| seen == device) {
            continue;
        }
        // Montagens do próprio Docker (overlay de containers, volumes) já são
        // contadas no disco que as hospeda.
        if mount.starts_with("/var/lib/docker/") || mount.starts_with("/run/") {
            continue;
        }
        seen_devices.push(device.to_string());
        output.push(mount.replace("\\040", " "));
    }
    output
}

#[cfg(unix)]
fn disk_usage(path: &Path) -> Option<(u64, u64)> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: `statvfs` só escreve na struct zerada que recebe e lê o caminho
    // terminado em NUL que o `CString` garante.
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stats) } != 0 {
        return None;
    }
    // Os campos mudam de largura entre arquiteturas; `as u64` normaliza.
    #[allow(clippy::unnecessary_cast)]
    let (block, blocks, free) = (
        stats.f_frsize as u64,
        stats.f_blocks as u64,
        stats.f_bfree as u64,
    );
    // Mesmo critério do `df`: usado = total − livre.
    Some(((blocks - free.min(blocks)) * block, blocks * block))
}

#[cfg(not(unix))]
fn disk_usage(_path: &Path) -> Option<(u64, u64)> {
    None
}

/// Leitor com estado: guarda a leitura anterior de CPU e de rede.
pub struct HostMetricsReader {
    proc_root: PathBuf,
    host_root: PathBuf,
    previous_cpu: Option<CpuTimes>,
    previous_net: Option<(Instant, u64, u64)>,
}

impl HostMetricsReader {
    #[must_use]
    pub fn new(proc_root: impl Into<PathBuf>, host_root: impl Into<PathBuf>) -> Self {
        Self {
            proc_root: proc_root.into(),
            host_root: host_root.into(),
            previous_cpu: None,
            previous_net: None,
        }
    }

    /// `HOST_PROC` / `HOST_ROOT`, com os caminhos do próprio sistema como padrão.
    #[must_use]
    pub fn from_env() -> Self {
        Self::new(
            std::env::var("HOST_PROC").unwrap_or_else(|_| "/proc".into()),
            std::env::var("HOST_ROOT").unwrap_or_else(|_| "/".into()),
        )
    }

    fn read(&self, relative: &str) -> Option<String> {
        std::fs::read_to_string(self.proc_root.join(relative)).ok()
    }

    /// Uma amostra. `None` fora do Linux ou sem `/proc` legível. A primeira
    /// leitura devolve CPU e rede zeradas, porque ainda não há diferença.
    #[allow(clippy::cast_precision_loss)]
    pub fn sample(&mut self) -> Option<HostSample> {
        let cpu = parse_cpu_times(&self.read("stat")?)?;
        let (memory_used_bytes, memory_total_bytes) = parse_meminfo(&self.read("meminfo")?)?;
        let load1 = self
            .read("loadavg")
            .as_deref()
            .and_then(parse_loadavg)
            .unwrap_or(0.0);
        let (rx, tx) = self
            .read("net/dev")
            .as_deref()
            .map(parse_net_dev)
            .unwrap_or_default();
        let now = Instant::now();

        let cpu_percent = self
            .previous_cpu
            .replace(cpu)
            .map_or(0.0, |before| cpu_percent(before, cpu));
        let (net_rx_bps, net_tx_bps) = self.previous_net.replace((now, rx, tx)).map_or(
            (0.0, 0.0),
            |(at, prev_rx, prev_tx)| {
                let seconds = now.duration_since(at).as_secs_f64();
                if seconds <= 0.0 {
                    return (0.0, 0.0);
                }
                (
                    rx.saturating_sub(prev_rx) as f64 / seconds,
                    tx.saturating_sub(prev_tx) as f64 / seconds,
                )
            },
        );

        let disks = self
            .read("mounts")
            .as_deref()
            .map(parse_mounts)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|mount| {
                let relative = mount.trim_start_matches('/');
                let (used_bytes, total_bytes) = disk_usage(&self.host_root.join(relative))?;
                (total_bytes > 0).then_some(DiskUsage {
                    mount,
                    used_bytes,
                    total_bytes,
                })
            })
            .collect();

        Some(HostSample {
            cpu_percent,
            memory_used_bytes,
            memory_total_bytes,
            load1,
            net_rx_bps,
            net_tx_bps,
            disks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_e_diferenca_entre_leituras() {
        let before =
            parse_cpu_times("cpu  100 0 100 700 100 0 0 0 0 0\ncpu0 1 2 3 4").expect("cpu");
        let after = parse_cpu_times("cpu  200 0 200 1300 100 0 0 0 0 0").expect("cpu");
        assert_eq!(
            before,
            CpuTimes {
                total: 1_000,
                idle: 800
            }
        );
        assert!((cpu_percent(before, after) - 25.0).abs() < 1e-9);
        assert!((cpu_percent(after, after)).abs() < f64::EPSILON);
    }

    #[test]
    fn memoria_usada_desconta_o_disponivel() {
        let text = "MemTotal:       16000 kB\nMemFree:  1000 kB\nMemAvailable:   12000 kB\n";
        assert_eq!(parse_meminfo(text), Some((4_000 * 1024, 16_000 * 1024)));
        assert_eq!(
            parse_meminfo("MemTotal: 10 kB\nMemFree: 4 kB"),
            Some((6 * 1024, 10 * 1024))
        );
    }

    #[test]
    fn rede_ignora_interfaces_virtuais() {
        let text = "Inter-|   Receive\n face |bytes packets\n\
            lo: 500 1 0 0 0 0 0 0 500 1 0 0 0 0 0 0\n\
            eth0: 1000 10 0 0 0 0 0 0 2000 20 0 0 0 0 0 0\n\
            veth12ab: 999 1 0 0 0 0 0 0 999 1 0 0 0 0 0 0\n\
            wg0: 10 1 0 0 0 0 0 0 20 1 0 0 0 0 0 0\n";
        assert_eq!(parse_net_dev(text), (1_010, 2_020));
    }

    #[test]
    fn montagens_reais_sem_repetir_dispositivo() {
        let text = "/dev/sda1 / ext4 rw 0 0\n\
            proc /proc proc rw 0 0\n\
            overlay /var/lib/docker/overlay2/x/merged overlay rw 0 0\n\
            /dev/sdb1 /srv/dados xfs rw 0 0\n\
            /dev/sda1 /var/lib/docker/volumes ext4 rw 0 0\n\
            /dev/sdc1 /mnt/meu\\040disco ext4 rw 0 0\n";
        assert_eq!(
            parse_mounts(text),
            vec!["/", "/srv/dados", "/mnt/meu disco"]
        );
    }

    #[test]
    fn load_average_do_primeiro_campo() {
        assert_eq!(parse_loadavg("0.52 0.40 0.30 1/200 999"), Some(0.52));
    }
}
