//! Testes ativos de rede: geram tráfego a partir do servidor e por isso só
//! entram no registro com `allow_active_tools`.

use std::{net::IpAddr, time::Duration};

use async_trait::async_trait;
use loco_rs::prelude::AppContext;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::{AiToolHandler, ToolArgs, ToolKind, ToolOutput};
use crate::services::{
    network_tools::{
        dns::latency::{measure_dns_lookup, DnsLookupOptions, DnsProtocol},
        icmp_probe::{probe_icmp, IcmpProbeOptions},
        playbook::{run_device_reachability_playbook, run_internet_health_playbook},
        tcp_probe::{probe_tcp, TcpProbeState},
        traceroute::{execute_traceroute, TracerouteOptions},
    },
    shared::errors::{AppError, AppResult},
};

const DEFAULT_PORTS: [u16; 8] = [80, 443, 22, 53, 8080, 3389, 445, 161];

/// IP literal ou resolvido por DNS; a falha de resolução volta como dado
/// para a IA, não como erro da ferramenta.
async fn resolve_target(target: &str) -> Result<IpAddr, ToolOutput> {
    if let Ok(ip) = target.parse() {
        return Ok(ip);
    }
    match tokio::net::lookup_host((target, 0)).await {
        Ok(mut addresses) => addresses.next().map(|address| address.ip()).ok_or_else(|| {
            ToolOutput::not_found(format!("Não foi possível resolver o hostname '{target}'"))
        }),
        Err(error) => Err(ToolOutput::not_found(format!(
            "Falha de resolução DNS: {error}"
        ))),
    }
}

fn target_schema(extra: Value) -> Value {
    let mut properties = json!({
        "target": { "type": "string", "description": "IP ou hostname (ex: '192.168.1.1', 'google.com')" }
    });
    if let (Some(base), Some(more)) = (properties.as_object_mut(), extra.as_object()) {
        base.extend(more.clone());
    }
    json!({ "type": "object", "properties": properties, "required": ["target"] })
}

pub struct Ping;

#[async_trait]
impl AiToolHandler for Ping {
    fn name(&self) -> &'static str {
        "ping_host"
    }

    fn description(&self) -> &'static str {
        "Ping ICMP agora: RTT mínimo/médio/máximo, jitter e perda de pacotes."
    }

    fn parameters(&self) -> Value {
        target_schema(json!({
            "count": { "type": "integer", "description": "Pacotes (1 a 5, padrão 3)" }
        }))
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Active
    }

    async fn execute(&self, _ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let target = args.required_text("target", "Alvo (target) não informado para o ping")?;
        let ip = match resolve_target(&target).await {
            Ok(ip) => ip,
            Err(output) => return Ok(output),
        };
        let count = args.integer_in("count", 3, 1, 5) as usize;
        let options = IcmpProbeOptions::new(count, Duration::from_millis(2000));
        let sample = probe_icmp(ip, &options, &CancellationToken::new()).await;

        Ok(ToolOutput::data(json!({
            "target": target,
            "ip": ip.to_string(),
            "packets_sent": count,
            "packet_loss_pct": sample.packet_loss_pct,
            "avg_rtt_ms": sample.avg_rtt_ms,
            "min_rtt_ms": sample.min_rtt_ms,
            "max_rtt_ms": sample.max_rtt_ms,
            "jitter_ms": sample.jitter_ms,
        })))
    }
}

pub struct Traceroute;

#[async_trait]
impl AiToolHandler for Traceroute {
    fn name(&self) -> &'static str {
        "traceroute"
    }

    fn description(&self) -> &'static str {
        "Traceroute até o destino: saltos com IP, hostname e latência."
    }

    fn parameters(&self) -> Value {
        target_schema(json!({
            "max_hops": { "type": "integer", "description": "Máximo de saltos (1 a 20, padrão 12)" }
        }))
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Active
    }

    async fn execute(&self, _ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let target =
            args.required_text("target", "Alvo (target) não informado para o traceroute")?;
        let ip = match resolve_target(&target).await {
            Ok(ip) => ip,
            Err(output) => return Ok(output),
        };
        let options = TracerouteOptions {
            max_hops: args.integer_in("max_hops", 12, 1, 20) as u8,
            timeout_ms: 1000,
            probes_per_hop: 1,
        };
        let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
        let hops = execute_traceroute(ip, options, sender, CancellationToken::new()).await;
        while receiver.recv().await.is_some() {}

        let hops: Vec<Value> = hops
            .into_iter()
            .map(|hop| {
                json!({
                    "hop": hop.hop,
                    "ip": hop.ip,
                    "hostname": hop.hostname,
                    "avg_rtt_ms": hop.avg_rtt_ms,
                    "status": hop.status,
                })
            })
            .collect();
        Ok(ToolOutput::data(json!({
            "target": target,
            "ip": ip.to_string(),
            "hops_count": hops.len(),
            "hops": hops,
        })))
    }
}

pub struct ScanPorts;

#[async_trait]
impl AiToolHandler for ScanPorts {
    fn name(&self) -> &'static str {
        "scan_ports"
    }

    fn description(&self) -> &'static str {
        "Testa conexão TCP em portas do alvo e diz quais estão abertas."
    }

    fn parameters(&self) -> Value {
        target_schema(json!({
            "ports": {
                "type": "array",
                "items": { "type": "integer" },
                "description": "Portas TCP (ex: [80, 443, 22]). Omitido: portas comuns."
            }
        }))
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Active
    }

    async fn execute(&self, _ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let target = args.required_text(
            "target",
            "Alvo (target) não informado para o scan de portas",
        )?;
        let ip = match resolve_target(&target).await {
            Ok(ip) => ip,
            Err(output) => return Ok(output),
        };
        let ports = args
            .ports("ports")
            .unwrap_or_else(|| DEFAULT_PORTS.to_vec());
        let timeout = Duration::from_millis(800);

        let mut open_ports = Vec::new();
        let mut closed_ports = Vec::new();
        for port in ports {
            let observation = probe_tcp(std::net::SocketAddr::new(ip, port), timeout).await;
            if observation.state == TcpProbeState::Open {
                open_ports.push(port);
            } else {
                closed_ports.push(port);
            }
        }
        Ok(ToolOutput::data(json!({
            "target": target,
            "ip": ip.to_string(),
            "open_ports": open_ports,
            "closed_ports": closed_ports,
        })))
    }
}

pub struct DnsLookup;

#[async_trait]
impl AiToolHandler for DnsLookup {
    fn name(&self) -> &'static str {
        "dns_lookup"
    }

    fn description(&self) -> &'static str {
        "Resolve um hostname (registro A) medindo o tempo de resposta do DNS."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "hostname": { "type": "string", "description": "Hostname a resolver (ex: 'google.com')" },
                "server": { "type": "string", "description": "Servidor DNS 'IP:porta' (padrão '1.1.1.1:53')" }
            },
            "required": ["hostname"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Active
    }

    async fn execute(&self, _ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let hostname =
            args.required_text("hostname", "Hostname não informado para a consulta DNS")?;
        let sample = measure_dns_lookup(DnsLookupOptions {
            hostname: hostname.clone(),
            server: args.text("server"),
            protocol: DnsProtocol::Udp,
            record_type: hickory_proto::rr::RecordType::A,
            doh_url: None,
            timeout_ms: 2000,
        })
        .await;
        Ok(ToolOutput::data(json!({
            "hostname": hostname,
            "success": sample.success,
            "lookup_time_ms": sample.lookup_time_ms,
            "answers": sample.answers,
            "error": sample.error,
        })))
    }
}

pub struct Playbook;

#[async_trait]
impl AiToolHandler for Playbook {
    fn name(&self) -> &'static str {
        "run_playbook"
    }

    fn description(&self) -> &'static str {
        "Checklist automatizado de diagnóstico: 'internet_health' (saída WAN) ou 'device_reachability' (alcance de um dispositivo, exige target IP)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "playbook_type": { "type": "string", "enum": ["internet_health", "device_reachability"] },
                "target": { "type": "string", "description": "IP do dispositivo (para 'device_reachability')" }
            },
            "required": ["playbook_type"]
        })
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Active
    }

    async fn execute(&self, _ctx: &AppContext, args: &ToolArgs) -> AppResult<ToolOutput> {
        let playbook = args
            .text("playbook_type")
            .unwrap_or_else(|| "internet_health".into());
        let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
        let cancel = CancellationToken::new();

        let summary = match playbook.as_str() {
            "internet_health" => {
                let summary = run_internet_health_playbook(sender, cancel).await;
                while receiver.recv().await.is_some() {}
                json!(summary)
            }
            "device_reachability" => {
                let target = args.required_text(
                    "target",
                    "Alvo (target) não informado para o playbook de dispositivo",
                )?;
                let ip: IpAddr = target.parse().map_err(|_| {
                    AppError::validation("Endereço IP inválido para o playbook de dispositivo")
                })?;
                let summary = run_device_reachability_playbook(ip, None, sender, cancel).await;
                while receiver.recv().await.is_some() {}
                json!(summary)
            }
            other => {
                return Err(AppError::validation(format!(
                    "Tipo de playbook desconhecido: {other}"
                )))
            }
        };
        Ok(ToolOutput::data(summary))
    }
}
