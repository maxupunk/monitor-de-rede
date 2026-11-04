//! As notas dos modelos de chat no benchmark do próprio NetMonitor (Laya-tuning).
//!
//! Avalia cada modelo local do Ollama em duas suítes complementares:
//! - **Suíte Oficial (`bench`, 26 cenários):** cobre todas as categorias de
//!   ferramentas, diagnóstico, gráficos, ações com confirmação e cortesia.
//! - **Cenários Reais NOC / ISP (`blind`, 45 cenários):** incidentes reais de
//!   fibra óptica GPON, Wi-Fi 802.11, BGP/OSPF/VLAN/NAT, pesquisa web de CVE/RFC,
//!   anti-alucinação e ambiguidade.
//!
//! O resultado vem de um snapshot embutido em tempo de compilação
//! (`chat_benchmark.json`, gerado pelo Laya-tuning via
//! `uv run laya-tuning chat netmonitor-benchmark`).

use std::{collections::BTreeMap, sync::LazyLock};

use serde::Deserialize;

use super::{local_models, ollama};
use crate::dtos::ai::{
    ChatBenchmarkDomain, ChatBenchmarkDomainScore, ChatBenchmarkReport, ChatBenchmarkRow,
    ChatBenchmarkSplitView,
};

/// Os 6 domínios operacionais do benchmark de chat, na ordem das colunas da página.
pub const DOMAIN_LABELS: &[(&str, &str)] = &[
    ("diagnostico", "Diagnóstico e causa raiz"),
    ("redes_equipamentos", "Redes, fibra, Wi-Fi e equipamentos"),
    ("monitoramento_alertas", "Monitores, alertas e logs"),
    ("operacao_infra", "Plataforma, Docker e auditoria"),
    ("conhecimento_web", "Documentação e pesquisa web"),
    ("seguranca_rigor", "Segurança e anti-alucinação"),
];

struct ChatModelMeta {
    model: &'static str,
    aliases: &'static [&'static str],
    label: &'static str,
    family: &'static str,
    quantization: Option<&'static str>,
    parameter_size: &'static str,
    download_mb: u32,
    recommended: bool,
    bundled: bool,
    netmonitor_trained: bool,
    fallback_description: &'static str,
}

const MEASURED_META: &[ChatModelMeta] = &[
    ChatModelMeta {
        model: "lfm3-netmonitor:latest",
        aliases: &[
            "lfm3-netmonitor",
            "lfm3-netmonitor:q8_0",
            "lfm3-netmonitor:q8_k",
            "netmonitor/lfm3-netmonitor:latest",
        ],
        label: "LFM3 NetMonitor (Q8_0 — Máxima precisão)",
        family: "Liquid LFM3 MoE (NetMonitor v3)",
        quantization: Some("Q8_0"),
        parameter_size: "2.6B MoE (472M ativos)",
        download_mb: 1_846,
        recommended: true,
        bundled: true,
        netmonitor_trained: true,
        fallback_description:
            "Especialista nas 61 ferramentas do NetMonitor em 8 bits (Q8_0): máxima fidelidade em diagnóstico, fibra GPON, Wi-Fi e segurança.",
    },
    ChatModelMeta {
        model: "lfm3-netmonitor:q5_k_m",
        aliases: &["netmonitor/lfm3-netmonitor:q5_k_m"],
        label: "LFM3 NetMonitor (Q5_K_M — Equilibrado)",
        family: "Liquid LFM3 MoE (NetMonitor v3)",
        quantization: Some("Q5_K_M"),
        parameter_size: "2.6B MoE (472M ativos)",
        download_mb: 1_259,
        recommended: false,
        bundled: true,
        netmonitor_trained: true,
        fallback_description:
            "Mesma especialização nas 61 ferramentas do NetMonitor com 32% menos memória (1,26 GB) e maior nota na suíte oficial.",
    },
    ChatModelMeta {
        model: "lfm3-netmonitor:q4_k_m",
        aliases: &["netmonitor/lfm3-netmonitor:q4_k_m"],
        label: "LFM3 NetMonitor (Q4_K_M — Ultraleve)",
        family: "Liquid LFM3 MoE (NetMonitor v3)",
        quantization: Some("Q4_K_M"),
        parameter_size: "2.6B MoE (472M ativos)",
        download_mb: 1_100,
        recommended: false,
        bundled: true,
        netmonitor_trained: true,
        fallback_description:
            "Versão compacta de 4 bits (1,10 GB) para servidores com pouca RAM/VRAM: a mais rápida em geração e líder nos cenários reais NOC/ISP.",
    },
    ChatModelMeta {
        model: "lfm3:latest",
        aliases: &["lfm3"],
        label: "LFM3 (Base sem ajuste)",
        family: "Liquid LFM3 MoE",
        quantization: Some("Q4_K_M"),
        parameter_size: "2.6B MoE (472M ativos)",
        download_mb: 1_600,
        recommended: false,
        bundled: false,
        netmonitor_trained: false,
        fallback_description:
            "Modelo base antes do fine-tuning do NetMonitor: rápido, mas falha no protocolo load_tools e em consultas de interfaces e inventário.",
    },
    ChatModelMeta {
        model: "llama3.2:3b",
        aliases: &["llama3.2"],
        label: "Llama 3.2 3B",
        family: "Meta Llama 3.2",
        quantization: Some("Q4_K_M"),
        parameter_size: "3B denso",
        download_mb: 2_000,
        recommended: false,
        bundled: false,
        netmonitor_trained: false,
        fallback_description:
            "Modelo denso de 3B parâmetros da Meta: perde pontos por chamar ferramentas em cumprimentos e executar ações sem pedir confirmação em casos ambíguos.",
    },
    ChatModelMeta {
        model: "qwen3:4b",
        aliases: &["qwen3"],
        label: "Qwen 3 4B",
        family: "Alibaba Qwen 3",
        quantization: Some("Q4_K_M"),
        parameter_size: "4B denso (raciocínio)",
        download_mb: 2_600,
        recommended: false,
        bundled: false,
        netmonitor_trained: false,
        fallback_description:
            "Modelo denso de 4B com cadeia de pensamento longa: estoura o tempo limite em chamadas multi-passo e consome o orçamento de tokens antes de responder.",
    },
];

fn find_meta(model: &str) -> Option<&'static ChatModelMeta> {
    MEASURED_META
        .iter()
        .find(|m| local_models::matches_model_or_alias(m.model, m.aliases, model))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    measured_on: String,
    hardware: String,
    num_ctx: u32,
    timeout_ms: u32,
    min_score: f64,
    min_safety: f64,
    min_tools: f64,
    bench_cases: u32,
    blind_cases: u32,
    models: Vec<ModelSnapshotRow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitResult {
    pub run: String,
    pub cases: u32,
    pub overall: f64,
    pub overall_low: f64,
    pub overall_high: f64,
    pub tools: f64,
    pub answer: f64,
    pub safety: f64,
    pub perfect: f64,
    pub errors: u32,
    pub turn_p50_ms: f64,
    pub turn_p95_ms: f64,
    pub tokens_per_s: f64,
    pub fit: bool,
    pub domains: BTreeMap<String, f64>,
}

impl SplitResult {
    fn view(&self) -> ChatBenchmarkSplitView {
        ChatBenchmarkSplitView {
            run: self.run.clone(),
            cases: self.cases,
            overall: self.overall,
            overall_low: self.overall_low,
            overall_high: self.overall_high,
            tools: self.tools,
            answer: self.answer,
            safety: self.safety,
            perfect: self.perfect,
            errors: self.errors,
            turn_p50_ms: self.turn_p50_ms,
            turn_p95_ms: self.turn_p95_ms,
            tokens_per_s: self.tokens_per_s,
            fit: self.fit,
            domains: DOMAIN_LABELS
                .iter()
                .filter_map(|(id, _)| {
                    self.domains.get(*id).map(|&score| ChatBenchmarkDomainScore {
                        domain: (*id).to_string(),
                        score,
                    })
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSnapshotRow {
    pub model: String,
    pub fit: bool,
    pub bench: SplitResult,
    #[serde(default)]
    pub blind: Option<SplitResult>,
}

impl ModelSnapshotRow {
    fn row(&self) -> ChatBenchmarkRow {
        let meta = find_meta(&self.model);
        let catalog_desc = ollama::RECOMMENDED_MODELS
            .iter()
            .find(|rec| rec.name.eq_ignore_ascii_case(&self.model))
            .map(|rec| rec.description.to_string());

        ChatBenchmarkRow {
            model: self.model.clone(),
            label: meta.map_or_else(|| self.model.clone(), |m| m.label.to_string()),
            family: meta.map_or_else(|| "Ollama".to_string(), |m| m.family.to_string()),
            description: catalog_desc
                .or_else(|| meta.map(|m| m.fallback_description.to_string())),
            quantization: meta.and_then(|m| m.quantization.map(str::to_string)),
            parameter_size: meta.map(|m| m.parameter_size.to_string()),
            download_mb: meta.map(|m| m.download_mb),
            recommended: meta.is_some_and(|m| m.recommended),
            bundled: meta.is_some_and(|m| m.bundled),
            netmonitor_trained: meta.is_some_and(|m| m.netmonitor_trained),
            fit: self.fit,
            bench: self.bench.view(),
            blind: self.blind.as_ref().map(SplitResult::view),
        }
    }
}

static SNAPSHOT: LazyLock<Option<Snapshot>> =
    LazyLock::new(|| serde_json::from_str(include_str!("chat_benchmark.json")).ok());

fn snapshot() -> Option<&'static Snapshot> {
    SNAPSHOT.as_ref()
}

/// O resultado de `model` no benchmark de chat (aceita apelidos como `lfm3-netmonitor`).
#[must_use]
pub fn result(model: &str) -> Option<&'static ModelSnapshotRow> {
    let canonical = find_meta(model).map_or(model, |m| m.model);
    snapshot()?
        .models
        .iter()
        .find(|row| row.model.eq_ignore_ascii_case(canonical))
}

/// O relatório completo para `GET /api/ai/chat/benchmark`.
#[must_use]
pub fn report() -> Option<ChatBenchmarkReport> {
    let snapshot = snapshot()?;
    let models: Vec<ChatBenchmarkRow> =
        snapshot.models.iter().map(ModelSnapshotRow::row).collect();
    Some(ChatBenchmarkReport {
        measured_on: snapshot.measured_on.clone(),
        hardware: snapshot.hardware.clone(),
        num_ctx: snapshot.num_ctx,
        timeout_ms: snapshot.timeout_ms,
        min_score: snapshot.min_score,
        min_safety: snapshot.min_safety,
        min_tools: snapshot.min_tools,
        bench_cases: snapshot.bench_cases,
        blind_cases: snapshot.blind_cases,
        domains: DOMAIN_LABELS
            .iter()
            .map(|(id, label)| ChatBenchmarkDomain {
                id: (*id).to_string(),
                label: (*label).to_string(),
            })
            .collect(),
        models,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_snapshot_embutido_abre_e_cobre_os_seis_dominios() {
        let snap = snapshot().expect("chat_benchmark.json inválido");
        assert!(snap.bench_cases > 0 && snap.blind_cases > 0);
        assert_eq!(snap.models.len(), 6);
        for row in &snap.models {
            assert!((0.0..=1.0).contains(&row.bench.overall), "{}", row.model);
            for (domain, _) in DOMAIN_LABELS {
                assert!(
                    row.bench.domains.contains_key(*domain),
                    "{} sem domínio {domain} no bench",
                    row.model
                );
            }
            if let Some(ref blind) = row.blind {
                assert!((0.0..=1.0).contains(&blind.overall), "{}", row.model);
                for (domain, _) in DOMAIN_LABELS {
                    assert!(
                        blind.domains.contains_key(*domain),
                        "{} sem domínio {domain} no blind",
                        row.model
                    );
                }
            }
        }
    }

    #[test]
    fn as_tres_quantizacoes_do_netmonitor_atendem_ao_portao() {
        let rep = report().expect("relatório de chat");
        assert_eq!(rep.models[0].model, "lfm3-netmonitor:latest");
        assert!(rep.models[0].recommended && rep.models[0].bundled);
        let aprovados: Vec<&str> = rep
            .models
            .iter()
            .filter(|m| m.fit)
            .map(|m| m.model.as_str())
            .collect();
        assert_eq!(
            aprovados,
            [
                "lfm3-netmonitor:latest",
                "lfm3-netmonitor:q4_k_m",
                "lfm3-netmonitor:q5_k_m"
            ]
        );
    }

    #[test]
    fn acha_pelo_nome_e_apelido() {
        let by_alias = result("lfm3-netmonitor").expect("pelo apelido");
        assert_eq!(by_alias.model, "lfm3-netmonitor:latest");
        assert!(result("modelo-inexistente").is_none());
    }
}

