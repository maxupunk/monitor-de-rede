//! Registro e execução de ferramentas para o Agent Harness da IA.

use std::{net::IpAddr, time::Duration};

use loco_rs::prelude::AppContext;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::{
    models::{
        _entities::alert_events::Column as AlertEventsColumn, alert_events, devices, monitors,
    },
    services::{
        ai::{
            drivers::traits::{AiTool, AiToolFunction},
            knowledge,
        },
        network_tools::{
            dns::latency::{measure_dns_lookup, DnsLookupOptions, DnsProtocol},
            icmp_probe::{probe_icmp, IcmpProbeOptions},
            playbook::{run_device_reachability_playbook, run_internet_health_playbook},
            tcp_probe::{probe_tcp, TcpProbeState},
            traceroute::{execute_traceroute, TracerouteOptions},
        },
        shared::errors::{AppError, AppResult},
    },
};

/// Retorna a lista de todas as ferramentas disponíveis para a IA no formato JSON Schema.
pub fn get_available_tools(allow_active: bool) -> Vec<AiTool> {
    let mut tools = vec![
        AiTool {
            r#type: "function".to_string(),
            function: AiToolFunction {
                name: "list_devices".to_string(),
                description: "Lista os dispositivos cadastrados no NetMonitor com status, endereço IP e fabricante.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "search": {
                            "type": "string",
                            "description": "Texto para filtrar pelo nome, IP ou fabricante do dispositivo"
                        },
                        "status": {
                            "type": "string",
                            "enum": ["up", "down", "warning", "unknown"],
                            "description": "Filtrar dispositivos por status operacional"
                        }
                    }
                }),
            },
        },
        AiTool {
            r#type: "function".to_string(),
            function: AiToolFunction {
                name: "get_device_detail".to_string(),
                description: "Obtém dados detalhados de um dispositivo específico, incluindo seus monitores configurados e status.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "identifier": {
                            "type": "string",
                            "description": "Nome ou endereço IP do dispositivo"
                        }
                    },
                    "required": ["identifier"]
                }),
            },
        },
        AiTool {
            r#type: "function".to_string(),
            function: AiToolFunction {
                name: "get_active_alerts".to_string(),
                description: "Obtém os alertas e incidentes recentes registrados no sistema com severidade e motivo.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Número máximo de alertas a retornar (padrão 10)"
                        },
                        "severity": {
                            "type": "string",
                            "enum": ["critical", "warning", "info"],
                            "description": "Filtrar por severidade do alerta"
                        }
                    }
                }),
            },
        },
        AiTool {
            r#type: "function".to_string(),
            function: AiToolFunction {
                name: "get_system_summary".to_string(),
                description: "Retorna um resumo geral da infraestrutura: total de dispositivos online/offline, monitores e alertas ativos.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        },
        AiTool {
            r#type: "function".to_string(),
            function: AiToolFunction {
                name: "search_system_docs".to_string(),
                description: "Consulta a documentação técnica interna e manuais de operação do NetMonitor sobre recursos como SNMP, VPN, Descoberta, Janelas de Manutenção e Alertas.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Termo ou dúvida sobre como funciona ou como configurar algo no NetMonitor"
                        }
                    },
                    "required": ["query"]
                }),
            },
        },
    ];

    if allow_active {
        tools.extend(vec![
            AiTool {
                r#type: "function".to_string(),
                function: AiToolFunction {
                    name: "ping_host".to_string(),
                    description: "Envia pacotes ICMP Echo Request (ping) nativos para um endereço IP ou hostname, medindo RTT médio, mínimo, máximo e porcentagem de perda de pacotes.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "target": {
                                "type": "string",
                                "description": "Endereço IP ou hostname a ser testado (ex: '192.168.1.1' ou 'google.com')"
                            },
                            "count": {
                                "type": "integer",
                                "description": "Quantidade de pacotes a enviar (1 a 5, padrão 3)"
                            }
                        },
                        "required": ["target"]
                    }),
                },
            },
            AiTool {
                r#type: "function".to_string(),
                function: AiToolFunction {
                    name: "traceroute".to_string(),
                    description: "Executa um rastreamento de rota (traceroute) até o destino, listando todos os saltos intermediários com seus IPs e latências individuais.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "target": {
                                "type": "string",
                                "description": "Endereço IP ou hostname do destino"
                            },
                            "max_hops": {
                                "type": "integer",
                                "description": "Número máximo de saltos (1 a 20, padrão 12)"
                            }
                        },
                        "required": ["target"]
                    }),
                },
            },
            AiTool {
                r#type: "function".to_string(),
                function: AiToolFunction {
                    name: "scan_ports".to_string(),
                    description: "Testa a conectividade TCP em portas específicas de um endereço IP para verificar se o serviço está aberto ou filtrado.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "target": {
                                "type": "string",
                                "description": "Endereço IP do alvo"
                            },
                            "ports": {
                                "type": "array",
                                "items": { "type": "integer" },
                                "description": "Lista de portas TCP a testar (ex: [80, 443, 22, 53, 3389]). Se omitido, testa as portas padrão mais comuns."
                            }
                        },
                        "required": ["target"]
                    }),
                },
            },
            AiTool {
                r#type: "function".to_string(),
                function: AiToolFunction {
                    name: "dns_lookup".to_string(),
                    description: "Executa consulta de resolução de nomes DNS para um hostname, medindo o tempo de resposta e retornando os endereços IP resolvidos.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "hostname": {
                                "type": "string",
                                "description": "Hostname a resolver (ex: 'google.com')"
                            },
                            "server": {
                                "type": "string",
                                "description": "Servidor DNS a consultar no formato 'IP:porta' (opcional, padrão '1.1.1.1:53')"
                            }
                        },
                        "required": ["hostname"]
                    }),
                },
            },
            AiTool {
                r#type: "function".to_string(),
                function: AiToolFunction {
                    name: "run_playbook".to_string(),
                    description: "Executa um checklist automatizado completo de diagnóstico de rede. Suporta 'internet_health' (diagnóstico WAN global) ou 'device_reachability' (diagnóstico aprofundado de um dispositivo).".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "playbook_type": {
                                "type": "string",
                                "enum": ["internet_health", "device_reachability"],
                                "description": "Tipo de playbook a executar"
                            },
                            "target": {
                                "type": "string",
                                "description": "Endereço IP do dispositivo (obrigatório para 'device_reachability')"
                            }
                        },
                        "required": ["playbook_type"]
                    }),
                },
            },
        ]);
    }

    tools
}

/// Executa a ferramenta solicitada e devolve o resultado serializado em JSON.
pub async fn execute_tool(
    ctx: &AppContext,
    name: &str,
    arguments_json: &str,
) -> AppResult<serde_json::Value> {
    let args: serde_json::Value =
        serde_json::from_str(arguments_json).unwrap_or_else(|_| json!({}));

    match name {
        "list_devices" => {
            let search = args.get("search").and_then(|v| v.as_str()).map(str::trim);
            let status = args.get("status").and_then(|v| v.as_str()).map(str::trim);

            let mut query = devices::Entity::find().limit(30);
            if let Some(s) = search {
                if !s.is_empty() {
                    query = query.filter(
                        Condition::any()
                            .add(devices::Column::Name.contains(s))
                            .add(devices::Column::IpAddress.contains(s)),
                    );
                }
            }
            if let Some(st) = status {
                if !st.is_empty() {
                    query = query.filter(devices::Column::Status.eq(st));
                }
            }

            let list = query.all(&ctx.db).await?;
            let result: Vec<serde_json::Value> = list
                .into_iter()
                .map(|d| {
                    json!({
                        "id": d.id,
                        "name": d.name,
                        "ip": d.ip_address,
                        "status": d.status,
                        "vendor": d.vendor,
                        "last_seen": d.last_seen_at
                    })
                })
                .collect();

            Ok(json!({ "total": result.len(), "devices": result }))
        }

        "get_device_detail" => {
            let identifier = args
                .get("identifier")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AppError::validation("Identificador do dispositivo não informado")
                })?;

            let dev = devices::Entity::find()
                .filter(
                    Condition::any()
                        .add(devices::Column::Name.eq(identifier))
                        .add(devices::Column::IpAddress.eq(identifier)),
                )
                .one(&ctx.db)
                .await?;

            if let Some(d) = dev {
                let dev_monitors = monitors::Entity::find()
                    .filter(monitors::Column::DeviceId.eq(d.id))
                    .all(&ctx.db)
                    .await?;

                let monitors_info: Vec<serde_json::Value> = dev_monitors
                    .into_iter()
                    .map(|m| {
                        json!({
                            "id": m.id,
                            "name": m.name,
                            "type": m.r#type,
                            "status": m.status,
                            "interval_seconds": m.interval_seconds,
                            "last_run_at": m.last_run_at,
                            "enabled": m.enabled
                        })
                    })
                    .collect();

                Ok(json!({
                    "id": d.id,
                    "name": d.name,
                    "ip": d.ip_address,
                    "status": d.status,
                    "vendor": d.vendor,
                    "operating_system": d.operating_system,
                    "created_at": d.created_at,
                    "last_seen": d.last_seen_at,
                    "monitors": monitors_info
                }))
            } else {
                Ok(json!({ "error": format!("Dispositivo '{}' não encontrado", identifier) }))
            }
        }

        "get_active_alerts" => {
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(10)
                .clamp(1, 50);
            let severity = args.get("severity").and_then(|v| v.as_str());

            let mut query = alert_events::Entity::find()
                .order_by_desc(AlertEventsColumn::CreatedAt)
                .limit(limit);

            if let Some(sev) = severity {
                query = query.filter(AlertEventsColumn::Severity.eq(sev));
            }

            let rows = query.all(&ctx.db).await?;
            let events: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|e| {
                    json!({
                        "id": e.id,
                        "device_id": e.device_id,
                        "monitor_id": e.monitor_id,
                        "severity": e.severity,
                        "status": e.status,
                        "message": e.message,
                        "created_at": e.created_at
                    })
                })
                .collect();

            Ok(json!({ "total": events.len(), "alerts": events }))
        }

        "get_system_summary" => {
            let all_devs = devices::Entity::find().all(&ctx.db).await?;
            let total_devices = all_devs.len();
            let up_devices = all_devs.iter().filter(|d| d.status == "up").count();
            let down_devices = all_devs.iter().filter(|d| d.status == "down").count();
            let warning_devices = all_devs.iter().filter(|d| d.status == "warning").count();

            let all_monitors = monitors::Entity::find().all(&ctx.db).await?;
            let total_monitors = all_monitors.len();

            let recent_critical = alert_events::Entity::find()
                .filter(AlertEventsColumn::Severity.eq("critical"))
                .limit(5)
                .all(&ctx.db)
                .await?
                .len();

            Ok(json!({
                "devices": {
                    "total": total_devices,
                    "up": up_devices,
                    "down": down_devices,
                    "warning": warning_devices
                },
                "monitors": {
                    "total": total_monitors
                },
                "active_critical_alerts": recent_critical
            }))
        }

        "search_system_docs" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let docs = knowledge::search_docs(query);
            let results: Vec<serde_json::Value> = docs
                .into_iter()
                .map(|d| {
                    json!({
                        "topic": d.title,
                        "content": d.content
                    })
                })
                .collect();

            Ok(json!({ "query": query, "results": results }))
        }

        "ping_host" => {
            let target_str = args
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::validation("Alvo (target) não informado para o ping"))?;

            let count = args
                .get("count")
                .and_then(|v| v.as_u64())
                .unwrap_or(3)
                .clamp(1, 5) as usize;

            let ip: IpAddr = if let Ok(parsed) = target_str.parse() {
                parsed
            } else {
                match tokio::net::lookup_host((target_str, 0)).await {
                    Ok(mut addrs) => addrs.next().map(|a| a.ip()).ok_or_else(|| {
                        AppError::validation(format!(
                            "Não foi possível resolver o hostname '{target_str}'"
                        ))
                    })?,
                    Err(e) => {
                        return Ok(json!({ "error": format!("Falha de resolução DNS: {e}") }))
                    }
                }
            };

            let options = IcmpProbeOptions::new(count, Duration::from_millis(2000));
            let cancel = CancellationToken::new();
            let sample = probe_icmp(ip, &options, &cancel).await;

            Ok(json!({
                "target": target_str,
                "ip": ip.to_string(),
                "packets_sent": count,
                "packet_loss_pct": sample.packet_loss_pct,
                "avg_rtt_ms": sample.avg_rtt_ms,
                "min_rtt_ms": sample.min_rtt_ms,
                "max_rtt_ms": sample.max_rtt_ms,
                "jitter_ms": sample.jitter_ms,
                "rtts": sample.rtts
            }))
        }

        "traceroute" => {
            let target_str = args.get("target").and_then(|v| v.as_str()).ok_or_else(|| {
                AppError::validation("Alvo (target) não informado para o traceroute")
            })?;

            let max_hops = args
                .get("max_hops")
                .and_then(|v| v.as_u64())
                .unwrap_or(12)
                .clamp(1, 20) as u8;

            let ip: IpAddr = if let Ok(parsed) = target_str.parse() {
                parsed
            } else {
                match tokio::net::lookup_host((target_str, 0)).await {
                    Ok(mut addrs) => addrs.next().map(|a| a.ip()).ok_or_else(|| {
                        AppError::validation(format!(
                            "Não foi possível resolver o hostname '{target_str}'"
                        ))
                    })?,
                    Err(e) => {
                        return Ok(json!({ "error": format!("Falha de resolução DNS: {e}") }))
                    }
                }
            };

            let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
            let cancel = CancellationToken::new();
            let options = TracerouteOptions {
                max_hops,
                timeout_ms: 1000,
                probes_per_hop: 1,
            };

            let hops = execute_traceroute(ip, options, sender, cancel).await;
            while receiver.recv().await.is_some() {}

            let hops_data: Vec<serde_json::Value> = hops
                .into_iter()
                .map(|h| {
                    json!({
                        "hop": h.hop,
                        "ip": h.ip,
                        "hostname": h.hostname,
                        "avg_rtt_ms": h.avg_rtt_ms,
                        "status": h.status
                    })
                })
                .collect();

            Ok(json!({
                "target": target_str,
                "ip": ip.to_string(),
                "hops_count": hops_data.len(),
                "hops": hops_data
            }))
        }

        "scan_ports" => {
            let target_str = args.get("target").and_then(|v| v.as_str()).ok_or_else(|| {
                AppError::validation("Alvo (target) não informado para o scan de portas")
            })?;

            let ip: IpAddr = if let Ok(parsed) = target_str.parse() {
                parsed
            } else {
                match tokio::net::lookup_host((target_str, 0)).await {
                    Ok(mut addrs) => addrs.next().map(|a| a.ip()).ok_or_else(|| {
                        AppError::validation(format!(
                            "Não foi possível resolver o hostname '{target_str}'"
                        ))
                    })?,
                    Err(e) => {
                        return Ok(json!({ "error": format!("Falha de resolução DNS: {e}") }))
                    }
                }
            };

            let default_ports = vec![80, 443, 22, 53, 8080, 3389, 445, 161];
            let ports: Vec<u16> = args
                .get("ports")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|p| p.as_u64().map(|n| n as u16))
                        .collect()
                })
                .unwrap_or(default_ports);

            let timeout = Duration::from_millis(800);
            let mut open_ports = Vec::new();
            let mut closed_ports = Vec::new();

            for port in ports {
                let addr = std::net::SocketAddr::new(ip, port);
                let obs = probe_tcp(addr, timeout).await;
                if obs.state == TcpProbeState::Open {
                    open_ports.push(port);
                } else {
                    closed_ports.push(port);
                }
            }

            Ok(json!({
                "target": target_str,
                "ip": ip.to_string(),
                "open_ports": open_ports,
                "closed_ports": closed_ports
            }))
        }

        "dns_lookup" => {
            let hostname = args
                .get("hostname")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    AppError::validation("Hostname não informado para a consulta DNS")
                })?;

            let server = args
                .get("server")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            let options = DnsLookupOptions {
                hostname: hostname.to_string(),
                server,
                protocol: DnsProtocol::Udp,
                record_type: hickory_proto::rr::RecordType::A,
                doh_url: None,
                timeout_ms: 2000,
            };

            let sample = measure_dns_lookup(options).await;
            Ok(json!({
                "hostname": hostname,
                "success": sample.success,
                "lookup_time_ms": sample.lookup_time_ms,
                "answers": sample.answers,
                "error": sample.error
            }))
        }

        "run_playbook" => {
            let p_type = args
                .get("playbook_type")
                .and_then(|v| v.as_str())
                .unwrap_or("internet_health");

            let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
            let cancel = CancellationToken::new();

            match p_type {
                "internet_health" => {
                    let summary = run_internet_health_playbook(sender, cancel).await;
                    while receiver.recv().await.is_some() {}
                    Ok(json!(summary))
                }
                "device_reachability" => {
                    let target_str =
                        args.get("target").and_then(|v| v.as_str()).ok_or_else(|| {
                            AppError::validation(
                                "Alvo (target) não informado para o playbook de dispositivo",
                            )
                        })?;

                    let ip: IpAddr = target_str.parse().map_err(|_| {
                        AppError::validation("Endereço IP inválido para o playbook de dispositivo")
                    })?;

                    let summary = run_device_reachability_playbook(ip, None, sender, cancel).await;
                    while receiver.recv().await.is_some() {}
                    Ok(json!(summary))
                }
                outro => Err(AppError::validation(format!(
                    "Tipo de playbook desconhecido: {outro}"
                ))),
            }
        }

        outro => Err(AppError::validation(format!(
            "Ferramenta desconhecida: {outro}"
        ))),
    }
}
