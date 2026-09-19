//! Orquestrador de Playbooks de Diagnóstico de Rede.
//!
//! Executa checklists automatizados de verificação para incidentes e anomalias,
//! emitindo eventos de progresso passo a passo e gerando uma síntese diagnóstica
//! conclusiva com recomendações acionáveis.

use std::{net::IpAddr, time::Duration};

use serde_json::json;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::{
    dns::latency::{measure_dns_lookup, DnsLookupOptions, DnsProtocol},
    icmp_probe::{probe_icmp, IcmpProbeOptions},
    speedtest::{self, SpeedTestEvent},
    tcp_probe::{probe_tcp, TcpProbeState},
    traceroute::{self, TracerouteOptions},
};
use crate::dtos::diagnostics::{PlaybookStepResult, PlaybookSummary};

#[derive(Debug, Clone)]
pub enum PlaybookEvent {
    Step(PlaybookStepResult),
    Summary(PlaybookSummary),
    Done,
}

/// Executa o Playbook de Saúde da Internet.
pub async fn run_internet_health_playbook(
    sender: mpsc::Sender<PlaybookEvent>,
    cancel: CancellationToken,
) -> PlaybookSummary {
    let mut steps = Vec::new();
    let mut recommendations = Vec::new();
    let mut overall_status = "success".to_string();

    // Passo 1: Resolução DNS
    let dns_step = run_dns_check(&cancel).await;
    if dns_step.status == "failed" {
        overall_status = "failed".into();
        recommendations
            .push("Verifique se os servidores DNS configurados estão acessíveis.".into());
        recommendations.push(
            "Experimente utilizar servidores DNS públicos confiáveis (1.1.1.1 ou 8.8.8.8).".into(),
        );
    } else if dns_step.status == "warning" && overall_status != "failed" {
        overall_status = "warning".into();
        recommendations.push("O tempo de resposta do DNS está elevado (>200ms). Considere configurar DNS com menor latência.".into());
    }
    steps.push(dns_step.clone());
    let _ = sender.send(PlaybookEvent::Step(dns_step)).await;

    if cancel.is_cancelled() {
        return build_cancelled_summary("internet_health", steps);
    }

    // Passo 2: Latência e Conectividade Externa (Ping)
    let ping_step = run_external_ping_check(&cancel).await;
    if ping_step.status == "failed" {
        overall_status = "failed".into();
        recommendations.push("Não há conectividade com a internet. Verifique o cabo de rede/fibra e o status do modem/ONT.".into());
        recommendations.push(
            "Reinicie o roteador principal da borda caso o link não retorne em instantes.".into(),
        );
    } else if ping_step.status == "warning" && overall_status != "failed" {
        overall_status = "warning".into();
        recommendations
            .push("Latência externa elevada ou perda intermitente de pacotes detectada.".into());
    }
    steps.push(ping_step.clone());
    let _ = sender.send(PlaybookEvent::Step(ping_step)).await;

    if cancel.is_cancelled() {
        return build_cancelled_summary("internet_health", steps);
    }

    // Passo 3: Rota até a Internet (Traceroute)
    let trace_step = run_internet_traceroute_check(&cancel).await;
    if trace_step.status == "warning" && overall_status != "failed" {
        overall_status = "warning".into();
        recommendations
            .push("Possível instabilidade em saltos intermediários da rota da operadora.".into());
    }
    steps.push(trace_step.clone());
    let _ = sender.send(PlaybookEvent::Step(trace_step)).await;

    if cancel.is_cancelled() {
        return build_cancelled_summary("internet_health", steps);
    }

    // Passo 4: Teste de Desempenho e Velocidade WAN
    let speed_step = run_speed_check(&cancel).await;
    if speed_step.status == "warning" && overall_status != "failed" {
        overall_status = "warning".into();
        recommendations
            .push("A taxa de transferência medida está abaixo do perfil ótimo da conexão.".into());
    }
    steps.push(speed_step.clone());
    let _ = sender.send(PlaybookEvent::Step(speed_step)).await;

    // Diagnóstico consolidado
    let diagnosis = match overall_status.as_str() {
        "failed" => "Falha crítica de conectividade com a internet detectada.".into(),
        "warning" => {
            "Conexão com a internet ativa, porém apresentando degradação de desempenho ou latência."
                .into()
        }
        _ => {
            "Conexão com a internet estável e operando dentro dos parâmetros ideais de desempenho."
                .into()
        }
    };

    if recommendations.is_empty() {
        recommendations.push("Nenhuma ação corretiva necessária no momento.".into());
    }

    let summary = PlaybookSummary {
        playbook_type: "internet_health".into(),
        target: Some("Internet / WAN".into()),
        status: overall_status,
        diagnosis,
        recommendations,
        steps,
    };

    let _ = sender.send(PlaybookEvent::Summary(summary.clone())).await;
    let _ = sender.send(PlaybookEvent::Done).await;
    summary
}

/// Executa o Playbook de Diagnóstico de Dispositivo.
pub async fn run_device_reachability_playbook(
    target_ip: IpAddr,
    device_name: Option<String>,
    sender: mpsc::Sender<PlaybookEvent>,
    cancel: CancellationToken,
) -> PlaybookSummary {
    let mut steps = Vec::new();
    let mut recommendations = Vec::new();
    let mut overall_status = "success".to_string();

    let target_label = device_name.unwrap_or_else(|| target_ip.to_string());

    // Passo 1: Sonda ICMP (Ping)
    let ping_step = run_device_icmp_check(target_ip, &cancel).await;
    let icmp_ok = ping_step.status == "success";
    steps.push(ping_step.clone());
    let _ = sender.send(PlaybookEvent::Step(ping_step)).await;

    if cancel.is_cancelled() {
        return build_cancelled_summary("device_reachability", steps);
    }

    // Passo 2: Sonda TCP em Portas de Serviço
    let tcp_step = run_device_tcp_check(target_ip, &cancel).await;
    let tcp_proves_alive = tcp_step.status == "success";
    steps.push(tcp_step.clone());
    let _ = sender.send(PlaybookEvent::Step(tcp_step)).await;

    if cancel.is_cancelled() {
        return build_cancelled_summary("device_reachability", steps);
    }

    // Passo 3: Rota até o dispositivo (Traceroute)
    let trace_step = run_device_traceroute_check(target_ip, &cancel).await;
    steps.push(trace_step.clone());
    let _ = sender.send(PlaybookEvent::Step(trace_step)).await;

    // Síntese diagnóstica baseada em evidências
    let diagnosis = if icmp_ok {
        "O dispositivo responde normalmente via ICMP e está acessível na rede.".into()
    } else if tcp_proves_alive {
        overall_status = "warning".into();
        recommendations.push("O dispositivo responde a conexões TCP, mas não responde ao ping ICMP. Verifique se as regras de firewall do equipamento estão bloqueando ICMP Echo Request.".into());
        "Dispositivo está operacional e respondendo na rede, porém com ICMP filtrado ou desativado."
            .into()
    } else {
        overall_status = "failed".into();
        recommendations.push("Verifique se o dispositivo está ligado e com o cabo de rede conectado ou sinal Wi-Fi adequado.".into());
        recommendations
            .push("Confirme se o endereço IP não mudou e se pertence à sub-rede correta.".into());
        recommendations
            .push("Verifique os switches e pontos de acesso intermediários na rota.".into());
        "Dispositivo inacessível: não respondeu a requisições ICMP nem a conexões TCP em portas comuns.".into()
    };

    if recommendations.is_empty() {
        recommendations.push("Nenhuma ação corretiva necessária.".into());
    }

    let summary = PlaybookSummary {
        playbook_type: "device_reachability".into(),
        target: Some(format!("{target_label} ({target_ip})")),
        status: overall_status,
        diagnosis,
        recommendations,
        steps,
    };

    let _ = sender.send(PlaybookEvent::Summary(summary.clone())).await;
    let _ = sender.send(PlaybookEvent::Done).await;
    summary
}

async fn run_dns_check(cancel: &CancellationToken) -> PlaybookStepResult {
    let hostname = "google.com";
    let options = DnsLookupOptions {
        hostname: hostname.into(),
        server: Some("1.1.1.1:53".into()),
        protocol: DnsProtocol::Udp,
        record_type: hickory_proto::rr::RecordType::A,
        doh_url: None,
        timeout_ms: 2_000,
    };

    let sample = measure_dns_lookup(options).await;
    if cancel.is_cancelled() {
        return cancelled_step(1, "Resolução DNS");
    }

    if sample.success {
        let latency = sample.lookup_time_ms.unwrap_or(0.0);
        let status = if latency > 300.0 {
            "warning"
        } else {
            "success"
        };
        PlaybookStepResult {
            step_index: 1,
            step_name: "Resolução DNS".into(),
            description: "Consulta de resolução de nomes para servidores públicos (1.1.1.1)".into(),
            status: status.into(),
            message: Some(format!(
                "DNS respondendo em {:.1}ms com {} registro(s) encontrado(s)",
                latency,
                sample.answers.len()
            )),
            data: Some(json!({
                "latencyMs": latency,
                "answers": sample.answers,
            })),
        }
    } else {
        PlaybookStepResult {
            step_index: 1,
            step_name: "Resolução DNS".into(),
            description: "Consulta de resolução de nomes para servidores públicos".into(),
            status: "failed".into(),
            message: Some(format!(
                "Falha ao resolver {hostname}: {}",
                sample.error.unwrap_or_else(|| "Sem resposta".into())
            )),
            data: None,
        }
    }
}

async fn run_external_ping_check(cancel: &CancellationToken) -> PlaybookStepResult {
    let target: IpAddr = "1.1.1.1".parse().unwrap();
    let options = IcmpProbeOptions::new(3, Duration::from_millis(2_000));
    let res = probe_icmp(target, &options, cancel).await;

    if cancel.is_cancelled() {
        return cancelled_step(2, "Latência e Perda Externa");
    }

    if let Some(avg_rtt) = res.avg_rtt_ms {
        let status = if avg_rtt > 120.0 {
            "warning"
        } else {
            "success"
        };
        PlaybookStepResult {
            step_index: 2,
            step_name: "Latência Externa (Ping)".into(),
            description: "Medição de latência ICMP contra o Cloudflare DNS (1.1.1.1)".into(),
            status: status.into(),
            message: Some(format!(
                "Conexão externa respondendo com latência média de {avg_rtt:.1}ms"
            )),
            data: Some(json!({
                "latencyMs": avg_rtt,
                "rtts": res.rtts,
                "packetLossPct": res.packet_loss_pct,
            })),
        }
    } else {
        PlaybookStepResult {
            step_index: 2,
            step_name: "Latência Externa (Ping)".into(),
            description: "Medição de latência ICMP contra o Cloudflare DNS (1.1.1.1)".into(),
            status: "failed".into(),
            message: Some("100% de perda de pacotes ao pingar endereço externo (1.1.1.1)".into()),
            data: None,
        }
    }
}

async fn run_internet_traceroute_check(cancel: &CancellationToken) -> PlaybookStepResult {
    let target: IpAddr = "1.1.1.1".parse().unwrap();
    let (tx, mut rx) = mpsc::channel(32);
    let options = TracerouteOptions {
        max_hops: 12,
        timeout_ms: 1_200,
        probes_per_hop: 1,
    };

    let hops = traceroute::execute_traceroute(target, options, tx, cancel.clone()).await;
    while rx.recv().await.is_some() {}

    if cancel.is_cancelled() {
        return cancelled_step(3, "Traçado de Rota (Traceroute)");
    }

    let reached = hops.iter().any(|h| h.status == "reached");
    let total_hops = hops.len();
    let timeouts = hops.iter().filter(|h| h.status == "timeout").count();

    let status = if reached && timeouts <= 3 {
        "success"
    } else {
        "warning"
    };

    PlaybookStepResult {
        step_index: 3,
        step_name: "Traçado de Rota (Traceroute)".into(),
        description: "Mapeamento dos saltos intermediários até o destino público".into(),
        status: status.into(),
        message: Some(format!(
            "Rota concluída em {total_hops} salto(s) ({} timeouts intermediários)",
            timeouts
        )),
        data: Some(json!({ "hopsCount": total_hops, "reached": reached })),
    }
}

async fn run_speed_check(cancel: &CancellationToken) -> PlaybookStepResult {
    let (tx, mut rx) = mpsc::channel(16);
    let task_cancel = cancel.clone();

    let speed_handle =
        tokio::spawn(async move { speedtest::execute_wan_speedtest(tx, task_cancel).await });

    let mut last_progress = None;
    while let Some(event) = rx.recv().await {
        if let SpeedTestEvent::Progress(prog) = event {
            last_progress = Some(prog);
        }
    }

    let result = speed_handle.await.ok().flatten();

    if cancel.is_cancelled() {
        return cancelled_step(4, "Teste de Velocidade WAN");
    }

    if let Some(res) = result {
        let status = if res.download_mbps < 5.0 || res.jitter_ms > 40.0 {
            "warning"
        } else {
            "success"
        };
        PlaybookStepResult {
            step_index: 4,
            step_name: "Teste de Velocidade (WAN)".into(),
            description: "Medição de Download, Upload, Ping e Jitter via Cloudflare".into(),
            status: status.into(),
            message: Some(format!(
                "Download: {:.1} Mbps | Upload: {:.1} Mbps | Ping: {:.1} ms | Jitter: {:.1} ms",
                res.download_mbps, res.upload_mbps, res.ping_ms, res.jitter_ms
            )),
            data: Some(json!(res)),
        }
    } else {
        PlaybookStepResult {
            step_index: 4,
            step_name: "Teste de Velocidade (WAN)".into(),
            description: "Medição de Download, Upload, Ping e Jitter via Cloudflare".into(),
            status: "warning".into(),
            message: Some("Teste de velocidade não pôde ser completado integralmente".into()),
            data: last_progress.map(|p| json!(p)),
        }
    }
}

async fn run_device_icmp_check(target: IpAddr, cancel: &CancellationToken) -> PlaybookStepResult {
    let options = IcmpProbeOptions::new(3, Duration::from_millis(1_500));
    let res = probe_icmp(target, &options, cancel).await;

    if cancel.is_cancelled() {
        return cancelled_step(1, "Sonda ICMP (Ping)");
    }

    if let Some(avg_rtt) = res.avg_rtt_ms {
        PlaybookStepResult {
            step_index: 1,
            step_name: "Sonda ICMP (Ping)".into(),
            description: format!("Envio de 3 pacotes Echo Request para {target}"),
            status: "success".into(),
            message: Some(format!(
                "Host respondendo normalmente (latência média: {avg_rtt:.1}ms)"
            )),
            data: Some(json!({
                "latencyMs": avg_rtt,
                "rtts": res.rtts,
                "packetLossPct": res.packet_loss_pct,
            })),
        }
    } else {
        PlaybookStepResult {
            step_index: 1,
            step_name: "Sonda ICMP (Ping)".into(),
            description: format!("Envio de 3 pacotes Echo Request para {target}"),
            status: "failed".into(),
            message: Some("100% de perda de pacotes ICMP".into()),
            data: None,
        }
    }
}

async fn run_device_tcp_check(target: IpAddr, cancel: &CancellationToken) -> PlaybookStepResult {
    let common_ports: &[u16] = &[80, 443, 22, 53, 8080, 3389, 445];
    let timeout = std::time::Duration::from_millis(800);
    let mut open_ports = Vec::new();

    for &port in common_ports {
        if cancel.is_cancelled() {
            return cancelled_step(2, "Sonda TCP de Portas");
        }
        let addr = std::net::SocketAddr::new(target, port);
        let obs = probe_tcp(addr, timeout).await;
        if obs.state == TcpProbeState::Open {
            open_ports.push(port);
        }
    }

    if !open_ports.is_empty() {
        PlaybookStepResult {
            step_index: 2,
            step_name: "Sonda TCP de Portas".into(),
            description: "Verificação de conectividade TCP em portas comuns de serviço".into(),
            status: "success".into(),
            message: Some(format!(
                "Host responde em conexões TCP nas portas: {:?}",
                open_ports
            )),
            data: Some(json!({ "openPorts": open_ports })),
        }
    } else {
        PlaybookStepResult {
            step_index: 2,
            step_name: "Sonda TCP de Portas".into(),
            description: "Verificação de conectividade TCP em portas comuns de serviço".into(),
            status: "failed".into(),
            message: Some("Nenhuma porta TCP respondeu à tentativa de conexão".into()),
            data: Some(json!({ "checkedPorts": common_ports })),
        }
    }
}

async fn run_device_traceroute_check(
    target: IpAddr,
    cancel: &CancellationToken,
) -> PlaybookStepResult {
    let (tx, mut rx) = mpsc::channel(32);
    let options = TracerouteOptions {
        max_hops: 15,
        timeout_ms: 1_200,
        probes_per_hop: 1,
    };

    let hops = traceroute::execute_traceroute(target, options, tx, cancel.clone()).await;
    while rx.recv().await.is_some() {}

    if cancel.is_cancelled() {
        return cancelled_step(3, "Traçado de Rota Local");
    }

    let reached = hops.iter().any(|h| h.status == "reached");
    let total_hops = hops.len();

    let status = if reached { "success" } else { "warning" };
    PlaybookStepResult {
        step_index: 3,
        step_name: "Traçado de Rota Local".into(),
        description: format!("Mapeamento dos nós de rede até {target}"),
        status: status.into(),
        message: Some(format!(
            "Traçado finalizado com {total_hops} salto(s) (destino alcançado: {})",
            if reached { "sim" } else { "não" }
        )),
        data: Some(json!({ "hopsCount": total_hops, "reached": reached })),
    }
}

fn cancelled_step(step_index: u8, step_name: &str) -> PlaybookStepResult {
    PlaybookStepResult {
        step_index,
        step_name: step_name.into(),
        description: "Execução cancelada pelo usuário".into(),
        status: "failed".into(),
        message: Some("Cancelado".into()),
        data: None,
    }
}

fn build_cancelled_summary(playbook_type: &str, steps: Vec<PlaybookStepResult>) -> PlaybookSummary {
    PlaybookSummary {
        playbook_type: playbook_type.into(),
        target: None,
        status: "failed".into(),
        diagnosis: "Execução do playbook cancelada pelo usuário.".into(),
        recommendations: vec![
            "Execute o diagnóstico novamente para obter uma análise completa.".into(),
        ],
        steps,
    }
}
