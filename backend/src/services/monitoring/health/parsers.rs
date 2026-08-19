//! Parsers puros dos arquivos do Linux lidos pela coleta de saúde.
//!
//! Tudo aqui recebe `&str` e devolve dado — nada abre arquivo. É o que permite
//! testar `/proc/stat`, `/proc/meminfo`, cgroup v1 e cgroup v2 com fixtures,
//! inclusive os casos parciais (campo ausente, `max`, unidade estranha), sem
//! depender do sistema em que a suíte roda.

/// Tempos acumulados de CPU de `/proc/stat`, em jiffies.
///
/// Guardamos só o par que interessa: o total e a parcela ociosa. O uso é
/// sempre um **delta** entre duas leituras — um valor absoluto de jiffies não
/// significa nada para quem olha um gráfico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    pub total: u64,
    pub idle: u64,
}

/// Lê a linha agregada `cpu` de `/proc/stat`.
///
/// `idle` soma `idle` + `iowait`: processo bloqueado em disco não está usando
/// CPU, e contá-lo como uso faria todo backup noturno parecer pico de carga.
#[must_use]
pub fn parse_proc_stat(conteudo: &str) -> Option<CpuTimes> {
    let linha = conteudo
        .lines()
        .find(|linha| linha.starts_with("cpu ") || *linha == "cpu")?;
    let campos: Vec<u64> = linha
        .split_whitespace()
        .skip(1)
        .filter_map(|campo| campo.parse::<u64>().ok())
        .collect();
    // user, nice, system, idle — abaixo disso a linha não é utilizável.
    if campos.len() < 4 {
        return None;
    }
    let idle = campos[3] + campos.get(4).copied().unwrap_or(0);
    Some(CpuTimes {
        total: campos.iter().sum(),
        idle,
    })
}

/// Percentual de uso entre duas leituras de [`parse_proc_stat`].
///
/// `None` quando o intervalo não avançou (duas leituras no mesmo jiffy) ou
/// quando os contadores retrocederam — o que acontece de fato depois de uma
/// suspensão ou de uma migração de container. Inventar um número ali produz
/// picos de 100% que nunca existiram.
#[must_use]
pub fn cpu_usage_percent(anterior: CpuTimes, atual: CpuTimes) -> Option<f64> {
    let total = atual.total.checked_sub(anterior.total)?;
    let idle = atual.idle.checked_sub(anterior.idle)?;
    if total == 0 || idle > total {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    Some(((total - idle) as f64 / total as f64) * 100.0)
}

/// Memória do host, em bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

impl MemoryInfo {
    #[must_use]
    pub const fn used_bytes(self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }

    /// Percentual usado. `None` quando o total é zero — divisão que só
    /// aconteceria com um arquivo corrompido, e cujo resultado seria `NaN`
    /// atravessando o motor de alertas.
    #[must_use]
    pub fn used_percent(self) -> Option<f64> {
        if self.total_bytes == 0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        Some((self.used_bytes() as f64 / self.total_bytes as f64) * 100.0)
    }
}

/// Lê `/proc/meminfo`.
///
/// Usa `MemAvailable`, e não `MemFree`: o kernel publica a primeira desde 3.14
/// justamente porque a segunda ignora cache recuperável e faz todo servidor
/// saudável parecer sem memória. Sem `MemAvailable`, cai para
/// `MemFree + Buffers + Cached`, que é a aproximação que o próprio kernel
/// documenta.
#[must_use]
pub fn parse_meminfo(conteudo: &str) -> Option<MemoryInfo> {
    let campo = |nome: &str| -> Option<u64> {
        conteudo
            .lines()
            .find(|linha| linha.starts_with(&format!("{nome}:")))
            .and_then(|linha| linha.split_whitespace().nth(1))
            .and_then(|valor| valor.parse::<u64>().ok())
            // `/proc/meminfo` publica em kB.
            .map(|kb| kb * 1024)
    };
    let total_bytes = campo("MemTotal")?;
    let available_bytes = match campo("MemAvailable") {
        Some(valor) => valor,
        None => campo("MemFree")? + campo("Buffers").unwrap_or(0) + campo("Cached").unwrap_or(0),
    };
    Some(MemoryInfo {
        total_bytes,
        available_bytes: available_bytes.min(total_bytes),
    })
}

/// Carga média de 1 minuto, de `/proc/loadavg`.
#[must_use]
pub fn parse_loadavg_1m(conteudo: &str) -> Option<f64> {
    conteudo.split_whitespace().next()?.parse::<f64>().ok()
}

/// Uptime do host em segundos, de `/proc/uptime`.
#[must_use]
pub fn parse_uptime_seconds(conteudo: &str) -> Option<f64> {
    let valor = conteudo.split_whitespace().next()?.parse::<f64>().ok()?;
    (valor.is_finite() && valor >= 0.0).then_some(valor)
}

/// Memória residente do próprio processo, de `/proc/self/status`.
///
/// `VmRSS` é o que o operador reconhece como "quanto o NetMonitor está
/// ocupando"; `VmSize` incluiria mapeamentos que nunca tocaram memória física.
#[must_use]
pub fn parse_process_rss_bytes(conteudo: &str) -> Option<u64> {
    conteudo
        .lines()
        .find(|linha| linha.starts_with("VmRSS:"))
        .and_then(|linha| linha.split_whitespace().nth(1))
        .and_then(|valor| valor.parse::<u64>().ok())
        .map(|kb| kb * 1024)
}

/// Tráfego agregado das interfaces do host, de `/proc/net/dev`.
///
/// Soma tudo menos o loopback: contar `lo` faria toda conversa interna do
/// processo aparecer como tráfego de rede.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NetTotals {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[must_use]
pub fn parse_net_dev(conteudo: &str) -> Option<NetTotals> {
    let mut totais = NetTotals::default();
    let mut viu_alguma = false;
    for linha in conteudo.lines().skip(2) {
        let Some((interface, resto)) = linha.split_once(':') else {
            continue;
        };
        let interface = interface.trim();
        if interface == "lo" || interface.is_empty() {
            continue;
        }
        let campos: Vec<u64> = resto
            .split_whitespace()
            .map(|campo| campo.parse::<u64>().unwrap_or(0))
            .collect();
        // rx_bytes é a coluna 0; tx_bytes é a 8.
        if campos.len() < 9 {
            continue;
        }
        totais.rx_bytes += campos[0];
        totais.tx_bytes += campos[8];
        viu_alguma = true;
    }
    viu_alguma.then_some(totais)
}

/// Taxa em bytes por segundo entre duas leituras acumuladas.
///
/// `None` quando o contador retrocedeu (reinício de interface) ou quando o
/// intervalo é não-positivo. Os contadores do kernel são de 64 bits e não dão
/// a volta em prazo relevante, então retrocesso é sempre reinício.
#[must_use]
pub fn rate_per_second(anterior: u64, atual: u64, segundos: f64) -> Option<f64> {
    if segundos <= 0.0 {
        return None;
    }
    let delta = atual.checked_sub(anterior)?;
    #[allow(clippy::cast_precision_loss)]
    Some(delta as f64 / segundos)
}

/// Limite de memória de um cgroup, em bytes.
///
/// `None` significa "sem limite": tanto o `max` do v2 quanto o valor sentinela
/// gigantesco do v1 querem dizer isso, e tratá-los como número faria o
/// percentual de uso ser sempre 0%.
#[must_use]
pub fn parse_cgroup_limit(conteudo: &str) -> Option<u64> {
    let bruto = conteudo.trim();
    if bruto == "max" {
        return None;
    }
    let valor = bruto.parse::<u64>().ok()?;
    // O v1 usa `PAGE_COUNTER_MAX * PAGE_SIZE` como "ilimitado"; na prática
    // qualquer coisa acima de 2^60 não é um limite de memória de verdade.
    (valor < (1 << 60)).then_some(valor)
}

/// Uso corrente de memória de um cgroup, em bytes.
#[must_use]
pub fn parse_cgroup_usage(conteudo: &str) -> Option<u64> {
    conteudo.trim().parse::<u64>().ok()
}

/// Memória realmente em uso pelo cgroup, descontando o cache de arquivos.
///
/// `memory.current` do v2 inclui page cache, que o kernel devolve sob pressão.
/// Reportá-lo cru faz todo container que já leu disco parecer estar em 99%. O
/// campo `inactive_file` de `memory.stat` é o que se desconta — é a mesma
/// conta que o próprio OOM killer usa para decidir o que dá para recuperar.
#[must_use]
pub fn parse_cgroup_v2_inactive_file(conteudo: &str) -> Option<u64> {
    conteudo
        .lines()
        .find(|linha| linha.starts_with("inactive_file "))
        .and_then(|linha| linha.split_whitespace().nth(1))
        .and_then(|valor| valor.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROC_STAT: &str = "cpu  100 20 50 800 30 0 5 0 0 0\n\
                             cpu0 50 10 25 400 15 0 2 0 0 0\n\
                             intr 12345\n";

    #[test]
    fn le_a_linha_agregada_de_cpu_somando_iowait_ao_ocioso() {
        let tempos = parse_proc_stat(PROC_STAT).expect("linha cpu");
        assert_eq!(tempos.total, 100 + 20 + 50 + 800 + 30 + 5);
        assert_eq!(tempos.idle, 830, "iowait conta como ocioso");
    }

    #[test]
    fn arquivo_sem_linha_de_cpu_nao_vira_zero() {
        assert!(parse_proc_stat("intr 1\nctxt 2\n").is_none());
        assert!(parse_proc_stat("cpu 1 2\n").is_none(), "linha truncada");
    }

    #[test]
    fn o_uso_de_cpu_e_o_delta_entre_duas_leituras() {
        let antes = CpuTimes {
            total: 1_000,
            idle: 900,
        };
        let depois = CpuTimes {
            total: 1_200,
            idle: 1_050,
        };
        // 200 jiffies passaram, 150 ociosos → 25% de uso.
        let uso = cpu_usage_percent(antes, depois).expect("uso");
        assert!((uso - 25.0).abs() < 1e-9, "veio {uso}");
    }

    #[test]
    fn contador_que_retrocede_ou_nao_avanca_nao_produz_pico() {
        let base = CpuTimes {
            total: 1_000,
            idle: 900,
        };
        assert_eq!(cpu_usage_percent(base, base), None, "intervalo nulo");
        assert_eq!(
            cpu_usage_percent(
                base,
                CpuTimes {
                    total: 500,
                    idle: 400
                }
            ),
            None,
            "reinício de contador"
        );
    }

    #[test]
    fn a_memoria_disponivel_vem_de_mem_available_quando_existe() {
        let info = parse_meminfo(
            "MemTotal:       1000 kB\nMemFree:         100 kB\n\
             MemAvailable:    400 kB\nBuffers:          50 kB\nCached:  200 kB\n",
        )
        .expect("meminfo");
        assert_eq!(info.total_bytes, 1000 * 1024);
        assert_eq!(info.available_bytes, 400 * 1024);
        assert!((info.used_percent().unwrap() - 60.0).abs() < 1e-9);
    }

    #[test]
    fn sem_mem_available_cai_para_a_aproximacao_documentada_pelo_kernel() {
        let info = parse_meminfo(
            "MemTotal:       1000 kB\nMemFree:         100 kB\nBuffers: 50 kB\nCached: 200 kB\n",
        )
        .expect("meminfo antigo");
        assert_eq!(info.available_bytes, 350 * 1024);
    }

    #[test]
    fn meminfo_sem_total_e_indisponivel_e_nao_zero() {
        assert!(parse_meminfo("MemFree: 100 kB\n").is_none());
        assert_eq!(
            MemoryInfo {
                total_bytes: 0,
                available_bytes: 0
            }
            .used_percent(),
            None,
            "divisão por zero viraria NaN dentro do motor de alertas"
        );
    }

    #[test]
    fn carga_e_uptime_saem_da_primeira_coluna() {
        assert_eq!(parse_loadavg_1m("0.52 0.31 0.20 1/234 5678\n"), Some(0.52));
        assert_eq!(parse_uptime_seconds("12345.67 98765.43\n"), Some(12345.67));
        assert_eq!(parse_loadavg_1m(""), None);
        assert_eq!(parse_uptime_seconds("-1 0\n"), None);
    }

    #[test]
    fn a_memoria_do_processo_e_a_residente() {
        let status = "Name:\tbackend\nVmSize:\t 900000 kB\nVmRSS:\t  51200 kB\n";
        assert_eq!(parse_process_rss_bytes(status), Some(51_200 * 1024));
        assert_eq!(parse_process_rss_bytes("Name:\tbackend\n"), None);
    }

    #[test]
    fn o_trafego_agregado_ignora_o_loopback() {
        let net_dev = "Inter-|   Receive                    |  Transmit\n\
             face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets\n\
             \x20   lo: 5000 10 0 0 0 0 0 0 5000 10 0 0 0 0 0 0\n\
             \x20 eth0: 1000 10 0 0 0 0 0 0 2000 20 0 0 0 0 0 0\n\
             \x20 eth1:  500  5 0 0 0 0 0 0  700  7 0 0 0 0 0 0\n";
        let totais = parse_net_dev(net_dev).expect("net dev");
        assert_eq!(totais.rx_bytes, 1500, "lo não entra na conta");
        assert_eq!(totais.tx_bytes, 2700);
    }

    #[test]
    fn arquivo_de_rede_so_com_loopback_e_indisponivel() {
        let net_dev = "cabecalho\ncabecalho\n    lo: 1 1 0 0 0 0 0 0 1 1 0 0 0 0 0 0\n";
        assert_eq!(parse_net_dev(net_dev), None);
    }

    #[test]
    fn a_taxa_recusa_intervalo_nulo_e_contador_reiniciado() {
        assert_eq!(rate_per_second(100, 200, 10.0), Some(10.0));
        assert_eq!(rate_per_second(100, 200, 0.0), None);
        assert_eq!(rate_per_second(200, 100, 10.0), None);
    }

    #[test]
    fn max_e_o_sentinela_do_v1_significam_sem_limite() {
        assert_eq!(parse_cgroup_limit("max\n"), None);
        assert_eq!(parse_cgroup_limit("9223372036854771712\n"), None);
        assert_eq!(parse_cgroup_limit("536870912\n"), Some(536_870_912));
        assert_eq!(parse_cgroup_limit("nao-numero"), None);
    }

    #[test]
    fn o_uso_do_cgroup_desconta_o_cache_recuperavel() {
        let stat = "anon 100\nfile 900\ninactive_file 700\nslab 10\n";
        assert_eq!(parse_cgroup_v2_inactive_file(stat), Some(700));
        assert_eq!(parse_cgroup_v2_inactive_file("anon 100\n"), None);
        assert_eq!(parse_cgroup_usage(" 1024 \n"), Some(1024));
    }
}
