//! Classificação do problema de um alerta (Fase 2 do roadmap de monitoramento
//! inteligente, §3.2/§3.3).
//!
//! O alerta passa a dizer **o que** está acontecendo, não só "caiu". A
//! classificação é pura e vive num lugar só: recebe o campo da condição da
//! regra (quando uma regra disparou) e os campos presentes no dataset, e
//! devolve um [`ProblemKind`]. O valor vai gravado em `alert_events.data` sob
//! [`PROBLEM_KIND`] — de onde alimenta o feed SSE, a Central de Alertas e o
//! texto das notificações — e é reavaliado a cada transição, porque a recaída
//! pode ter causa diferente da queda original.

use serde_json::Value;

use super::{contracts::AlertDataset, fields};

/// Chave de `alert_events.data` com a classificação do episódio (camelCase,
/// como `lastProblemAt`).
pub const PROBLEM_KIND: &str = "problemKind";

/// O que o alerta está observando. A forma string é contrato extensível:
/// valores novos entram aqui e no rótulo, sem migração de schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    /// Indisponível — o fallback honesto quando nada mais específico se vê.
    Down,
    /// O host responde via TCP, mas não aceita ou não devolve ICMP.
    IcmpFiltered,
    /// Perda parcial de pacotes (ping intermitente).
    PacketLoss,
    /// Latência acima do tolerado.
    Latency,
    /// Falha ou degradação de resolução DNS.
    DnsFailure,
    /// Interface de rede caindo/oscilando (transição ou leitura SNMP).
    InterfaceFlap,
    /// Túnel VPN caindo ou instável.
    VpnInstability,
    /// Anomalia estatística (Z-Score ou desvio relevante em relação à baseline).
    StatisticalAnomaly,
}

impl ProblemKind {
    /// Forma persistida em `data.problemKind`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::IcmpFiltered => "icmp_filtered",
            Self::PacketLoss => "packet_loss",
            Self::Latency => "latency",
            Self::DnsFailure => "dns_failure",
            Self::InterfaceFlap => "interface_flap",
            Self::VpnInstability => "vpn_instability",
            Self::StatisticalAnomaly => "statistical_anomaly",
        }
    }

    /// Lê de volta o valor persistido; desconhecido vira `None` (contrato
    /// extensível: uma versão nova pode gravar um tipo que esta não conhece).
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "down" => Some(Self::Down),
            "icmp_filtered" => Some(Self::IcmpFiltered),
            "packet_loss" => Some(Self::PacketLoss),
            "latency" => Some(Self::Latency),
            "dns_failure" => Some(Self::DnsFailure),
            "interface_flap" => Some(Self::InterfaceFlap),
            "vpn_instability" => Some(Self::VpnInstability),
            "statistical_anomaly" | "anomaly" => Some(Self::StatisticalAnomaly),
            _ => None,
        }
    }

    /// Rótulo em pt-BR para as notificações ("Tipo: perda de pacotes").
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Down => "indisponível",
            Self::IcmpFiltered => "ICMP filtrado",
            Self::PacketLoss => "perda de pacotes",
            Self::Latency => "latência alta",
            Self::DnsFailure => "falha de DNS",
            Self::InterfaceFlap => "interface oscilando",
            Self::VpnInstability => "instabilidade VPN",
            Self::StatisticalAnomaly => "anomalia estatística",
        }
    }
}

/// Classifica o problema a partir do campo da condição da regra e do dataset.
///
/// O campo da condição manda quando existe: é ele que diz **o que a regra
/// vigia** — uma regra sobre `packetLoss` num monitor que também ficou down é
/// perda de pacotes, não indisponibilidade genérica. Sem condição (o caminho
/// do `warning`, que não bate regra alguma), manda o dataset: transições de
/// interface e de túnel se identificam pelos próprios campos, DNS pelo tipo
/// do monitor, e perda de pacotes pelo valor observado. O resto é `down`.
#[must_use]
pub fn classify(condition_field: Option<&str>, dataset: &AlertDataset) -> ProblemKind {
    if let Some(kind) = condition_field.and_then(kind_of_field) {
        return kind;
    }
    classify_by_dataset(dataset)
}

/// Valor observado da métrica que motivou o alerta, formatado para a
/// notificação ("23% de perda", "812 ms"). Só os tipos medidos pelos checkers
/// têm valor a mostrar — os demais já se descrevem na mensagem do alerta.
/// Agregações (p95, médias do episódio) ficam para o futuro de propósito.
#[must_use]
pub fn observed_value(kind: ProblemKind, dataset: &AlertDataset) -> Option<String> {
    let number = |key: &str| dataset.get(key).and_then(Value::as_f64);
    match kind {
        ProblemKind::PacketLoss => {
            number(fields::PACKET_LOSS).map(|value| format!("{value:.0}% de perda"))
        }
        ProblemKind::Latency => number(fields::LATENCY_MS).map(|value| format!("{value:.0} ms")),
        ProblemKind::StatisticalAnomaly => number(fields::LATENCY_Z_SCORE)
            .map(|z| format!("{z:+.1}σ latência"))
            .or_else(|| number(fields::PACKET_LOSS_Z_SCORE).map(|z| format!("{z:+.1}σ perda")))
            .or_else(|| number(fields::UPTIME_Z_SCORE).map(|z| format!("{z:+.1}σ queda uptime")))
            .or_else(|| {
                number(fields::SYSLOG_VOLUME_Z_SCORE).map(|z| format!("{z:+.1}σ volume logs"))
            })
            .or_else(|| number(fields::TRAFFIC_IN_Z_SCORE).map(|z| format!("{z:+.1}σ tráfego IN")))
            .or_else(|| {
                number(fields::TRAFFIC_OUT_Z_SCORE).map(|z| format!("{z:+.1}σ tráfego OUT"))
            })
            .or_else(|| {
                number(fields::LATENCY_DEVIATION_PERCENT)
                    .map(|dev| format!("{dev:+.0}% vs baseline"))
            })
            .or_else(|| {
                number(fields::PACKET_LOSS_DEVIATION_PERCENT)
                    .map(|dev| format!("{dev:+.0} p.p. vs baseline"))
            }),
        _ => None,
    }
}

/// Tipo + valor observado prontos para a notificação de disparo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemDetail {
    pub kind: ProblemKind,
    pub observed: Option<String>,
}

/// Monta o detalhe do problema a partir do dataset do disparo.
#[must_use]
pub fn detail(kind: ProblemKind, dataset: &AlertDataset) -> ProblemDetail {
    ProblemDetail {
        kind,
        observed: observed_value(kind, dataset),
    }
}

/// O que o campo vigiado pela condição diz sobre o problema.
fn kind_of_field(field: &str) -> Option<ProblemKind> {
    match field {
        fields::PACKET_LOSS => Some(ProblemKind::PacketLoss),
        fields::REACHABILITY_CAUSE => Some(ProblemKind::IcmpFiltered),
        fields::LATENCY_MS
        | fields::CONNECT_TIME_MS
        | fields::RESOLUTION_TIME_MS
        | fields::DURATION_MS => Some(ProblemKind::Latency),
        fields::LATENCY_Z_SCORE
        | fields::LATENCY_STDDEV_MS
        | fields::LATENCY_UPPER_BAND_MS
        | fields::LATENCY_DEVIATION_PERCENT
        | fields::PACKET_LOSS_Z_SCORE
        | fields::PACKET_LOSS_STDDEV_PERCENT
        | fields::PACKET_LOSS_UPPER_BAND_PERCENT
        | fields::PACKET_LOSS_DEVIATION_PERCENT
        | fields::UPTIME_Z_SCORE
        | fields::UPTIME_STDDEV_PERCENT
        | fields::UPTIME_DEVIATION_PERCENT
        | fields::SYSLOG_VOLUME_Z_SCORE
        | fields::SYSLOG_VOLUME_BASELINE
        | fields::SYSLOG_VOLUME_STDDEV
        | fields::TRAFFIC_IN_Z_SCORE
        | fields::TRAFFIC_OUT_Z_SCORE => Some(ProblemKind::StatisticalAnomaly),
        fields::INTERFACE_STATUS_TRANSITION
        | fields::INTERFACE_SPEED_TRANSITION
        | fields::INTERFACE_SPEED_BPS
        | fields::INTERFACE_OPER_STATUS
        | fields::INTERFACE_SPEED_DROP_PERCENT
        | fields::IF_OPER_STATUS
        | fields::IF_SPEED => Some(ProblemKind::InterfaceFlap),
        fields::VPN_STATUS_TRANSITION
        | fields::VPN_PEER_STATUS
        | fields::VPN_SECONDS_SINCE_ACTIVITY => Some(ProblemKind::VpnInstability),
        // `status`, `statusCode`, `snmpUptime`, tráfego e campos desconhecidos
        // não dizem nada por si: a decisão passa para o dataset.
        _ => None,
    }
}

fn classify_by_dataset(dataset: &AlertDataset) -> ProblemKind {
    if dataset
        .get(fields::REACHABILITY_CAUSE)
        .and_then(Value::as_str)
        == Some("icmp_filtered")
    {
        return ProblemKind::IcmpFiltered;
    }
    if dataset.contains_key(fields::INTERFACE_STATUS_TRANSITION)
        || dataset.contains_key(fields::INTERFACE_SPEED_TRANSITION)
    {
        return ProblemKind::InterfaceFlap;
    }
    if dataset.contains_key(fields::VPN_STATUS_TRANSITION)
        || dataset.contains_key(fields::VPN_PEER_STATUS)
    {
        return ProblemKind::VpnInstability;
    }
    if dataset.get("type").and_then(Value::as_str) == Some("dns") {
        return ProblemKind::DnsFailure;
    }
    if dataset
        .get(fields::LATENCY_Z_SCORE)
        .and_then(Value::as_f64)
        .is_some_and(|z| z >= 3.0)
        || dataset
            .get(fields::PACKET_LOSS_Z_SCORE)
            .and_then(Value::as_f64)
            .is_some_and(|z| z >= 3.0)
    {
        return ProblemKind::StatisticalAnomaly;
    }
    if dataset
        .get(fields::PACKET_LOSS)
        .and_then(Value::as_f64)
        .is_some_and(|loss| loss > 0.0)
    {
        return ProblemKind::PacketLoss;
    }
    ProblemKind::Down
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dataset(pairs: &[(&str, Value)]) -> AlertDataset {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect()
    }

    #[test]
    fn a_classificacao_faz_aida_e_volta_pela_string_persistida() {
        for kind in [
            ProblemKind::Down,
            ProblemKind::IcmpFiltered,
            ProblemKind::PacketLoss,
            ProblemKind::Latency,
            ProblemKind::DnsFailure,
            ProblemKind::InterfaceFlap,
            ProblemKind::VpnInstability,
        ] {
            assert_eq!(ProblemKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(ProblemKind::parse("flapping"), None);
    }

    #[test]
    fn o_campo_da_condicao_manda_quando_existe() {
        let ping_down = dataset(&[(fields::STATUS, json!("down")), ("type", json!("ping"))]);
        assert_eq!(
            classify(Some(fields::PACKET_LOSS), &ping_down),
            ProblemKind::PacketLoss
        );
        for field in [
            fields::LATENCY_MS,
            fields::CONNECT_TIME_MS,
            fields::RESOLUTION_TIME_MS,
            fields::DURATION_MS,
        ] {
            assert_eq!(classify(Some(field), &ping_down), ProblemKind::Latency);
        }
        assert_eq!(
            classify(Some(fields::INTERFACE_STATUS_TRANSITION), &ping_down),
            ProblemKind::InterfaceFlap
        );
        assert_eq!(
            classify(Some(fields::VPN_STATUS_TRANSITION), &ping_down),
            ProblemKind::VpnInstability
        );
    }

    #[test]
    fn condicao_de_status_generico_cai_para_o_dataset() {
        let dns = dataset(&[(fields::STATUS, json!("down")), ("type", json!("dns"))]);
        assert_eq!(
            classify(Some(fields::STATUS), &dns),
            ProblemKind::DnsFailure
        );

        let tcp = dataset(&[(fields::STATUS, json!("down")), ("type", json!("tcp"))]);
        assert_eq!(classify(Some(fields::STATUS), &tcp), ProblemKind::Down);
    }

    #[test]
    fn dataset_de_interface_e_vpn_se_identificam_pelos_proprios_campos() {
        let interface = dataset(&[(
            fields::INTERFACE_STATUS_TRANSITION,
            json!(fields::interface_status_transition::WENT_DOWN),
        )]);
        assert_eq!(classify(None, &interface), ProblemKind::InterfaceFlap);

        let vpn = dataset(&[(fields::VPN_PEER_STATUS, json!("disconnected"))]);
        assert_eq!(classify(None, &vpn), ProblemKind::VpnInstability);
    }

    #[test]
    fn warning_de_ping_com_perda_classifica_como_perda_de_pacotes() {
        // O caminho do `warning` não tem condição: manda o valor observado.
        let aviso = dataset(&[
            (fields::STATUS, json!("warning")),
            ("type", json!("ping")),
            (fields::PACKET_LOSS, json!(33.0)),
        ]);
        assert_eq!(classify(None, &aviso), ProblemKind::PacketLoss);
    }

    #[test]
    fn evidencia_tcp_classifica_icmp_filtrado_antes_da_perda() {
        let aviso = dataset(&[
            (fields::STATUS, json!("warning")),
            (fields::REACHABILITY_CAUSE, json!("icmp_filtered")),
            (fields::PACKET_LOSS, json!(100.0)),
        ]);
        assert_eq!(classify(None, &aviso), ProblemKind::IcmpFiltered);
        assert_eq!(
            classify(Some(fields::REACHABILITY_CAUSE), &aviso),
            ProblemKind::IcmpFiltered
        );
    }

    #[test]
    fn perda_zerada_nao_e_problema_de_perda() {
        let saudavel = dataset(&[("type", json!("ping")), (fields::PACKET_LOSS, json!(0.0))]);
        assert_eq!(classify(None, &saudavel), ProblemKind::Down);
    }

    #[test]
    fn valor_observado_so_existe_para_os_tipos_medidos() {
        let ping = dataset(&[
            (fields::PACKET_LOSS, json!(23.4)),
            (fields::LATENCY_MS, json!(812.2)),
        ]);
        assert_eq!(
            observed_value(ProblemKind::PacketLoss, &ping),
            Some("23% de perda".to_string())
        );
        assert_eq!(
            observed_value(ProblemKind::Latency, &ping),
            Some("812 ms".to_string())
        );
        assert_eq!(observed_value(ProblemKind::Down, &ping), None);
        assert_eq!(observed_value(ProblemKind::PacketLoss, &dataset(&[])), None);
    }

    #[test]
    fn rotulos_em_portugues_cobrem_todos_os_tipos() {
        assert_eq!(ProblemKind::Down.label(), "indisponível");
        assert_eq!(ProblemKind::IcmpFiltered.label(), "ICMP filtrado");
        assert_eq!(ProblemKind::PacketLoss.label(), "perda de pacotes");
        assert_eq!(ProblemKind::Latency.label(), "latência alta");
        assert_eq!(ProblemKind::DnsFailure.label(), "falha de DNS");
        assert_eq!(ProblemKind::InterfaceFlap.label(), "interface oscilando");
        assert_eq!(ProblemKind::VpnInstability.label(), "instabilidade VPN");
    }
}
