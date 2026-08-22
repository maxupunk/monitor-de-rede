//! Contrato tipado do vocabulário de alertas.
//!
//! # O vão que este arquivo fecha
//!
//! As chaves de `condition.field` viviam em `services::alerts::fields` (Rust) e
//! os rótulos em português em `frontend/src/utils/alertPresentation.ts`
//! (TypeScript). Nada ligava os dois: renomear um campo de um lado apagava o
//! rótulo do outro **sem erro de compilação em nenhum dos dois** — como o
//! próprio comentário do `fields.rs` registrava. Acrescentar quatro campos de
//! saúde sem fechar isso dobraria a dívida.
//!
//! Agora as chaves atravessam a fronteira por `ts-rs` como um tipo união, e o
//! `alertPresentation.ts` é tipado contra ele: renomear no Rust quebra o
//! `typecheck` do frontend, que é onde o erro deve aparecer.
//!
//! O `serde(rename_all = "camelCase")` produz exatamente as constantes de
//! [`crate::services::alerts::fields`] — e um teste nesta mesma pasta garante
//! que as duas listas nunca divirjam.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Um `condition.field` válido.
///
/// A ordem é a de [`crate::services::alerts::fields::ALERT_FIELDS`], que é a
/// ordem em que os campos aparecem no seletor da tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum AlertField {
    // Resultado de monitor
    Status,
    LatencyMs,
    PacketLoss,
    StatusCode,
    DurationMs,
    ConnectTimeMs,
    ResolutionTimeMs,
    IfOperStatus,
    IfSpeed,
    SnmpUptime,
    InBps,
    OutBps,
    // Saúde do equipamento — de dispositivo, não do servidor
    CpuUsagePercent,
    MemoryUsedPercent,
    StorageUsedPercent,
    LoadAverage1m,
    // Interfaces coletadas via SNMP
    InterfaceName,
    InterfaceOperStatus,
    InterfaceStatusTransition,
    InterfaceSpeedBps,
    InterfaceSpeedTransition,
    InterfaceSpeedDropPercent,
    // Túneis WireGuard
    VpnPeerName,
    VpnPeerStatus,
    VpnStatusTransition,
    VpnSecondsSinceActivity,
    // Padrões no log
    LogPatternKey,
    LogMatchCount,
    LogWindowSeconds,
    LogSeverity,
    LogMessage,
    // Baseline móvel e anomalias estatísticas (§2.3.3)
    LatencyBaselineMs,
    LatencyStddevMs,
    LatencyDeviationPercent,
    LatencyZScore,
    LatencyUpperBandMs,
    PacketLossBaselinePercent,
    PacketLossStddevPercent,
    PacketLossDeviationPercent,
    PacketLossZScore,
    PacketLossUpperBandPercent,
    UptimeBaselinePercent,
    UptimeStddevPercent,
    UptimeDeviationPercent,
    UptimeZScore,
    SyslogVolumeBaseline,
    SyslogVolumeStddev,
    SyslogVolumeZScore,
    TrafficInZScore,
    TrafficOutZScore,
    // Fora da tela, mas avaliáveis
    Success,
    Type,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::alerts::fields::ALERT_FIELDS;

    /// Todas as variantes, na mesma ordem da declaração.
    ///
    /// Escrita à mão de propósito: é ela que obriga quem acrescenta uma
    /// variante a olhar para a lista do Rust — e o teste abaixo obriga as duas
    /// a coincidirem com a do frontend.
    const TODAS: [AlertField; 52] = [
        AlertField::Status,
        AlertField::LatencyMs,
        AlertField::PacketLoss,
        AlertField::StatusCode,
        AlertField::DurationMs,
        AlertField::ConnectTimeMs,
        AlertField::ResolutionTimeMs,
        AlertField::IfOperStatus,
        AlertField::IfSpeed,
        AlertField::SnmpUptime,
        AlertField::InBps,
        AlertField::OutBps,
        AlertField::CpuUsagePercent,
        AlertField::MemoryUsedPercent,
        AlertField::StorageUsedPercent,
        AlertField::LoadAverage1m,
        AlertField::InterfaceName,
        AlertField::InterfaceOperStatus,
        AlertField::InterfaceStatusTransition,
        AlertField::InterfaceSpeedBps,
        AlertField::InterfaceSpeedTransition,
        AlertField::InterfaceSpeedDropPercent,
        AlertField::VpnPeerName,
        AlertField::VpnPeerStatus,
        AlertField::VpnStatusTransition,
        AlertField::VpnSecondsSinceActivity,
        AlertField::LogPatternKey,
        AlertField::LogMatchCount,
        AlertField::LogWindowSeconds,
        AlertField::LogSeverity,
        AlertField::LogMessage,
        AlertField::LatencyBaselineMs,
        AlertField::LatencyStddevMs,
        AlertField::LatencyDeviationPercent,
        AlertField::LatencyZScore,
        AlertField::LatencyUpperBandMs,
        AlertField::PacketLossBaselinePercent,
        AlertField::PacketLossStddevPercent,
        AlertField::PacketLossDeviationPercent,
        AlertField::PacketLossZScore,
        AlertField::PacketLossUpperBandPercent,
        AlertField::UptimeBaselinePercent,
        AlertField::UptimeStddevPercent,
        AlertField::UptimeDeviationPercent,
        AlertField::UptimeZScore,
        AlertField::SyslogVolumeBaseline,
        AlertField::SyslogVolumeStddev,
        AlertField::SyslogVolumeZScore,
        AlertField::TrafficInZScore,
        AlertField::TrafficOutZScore,
        AlertField::Success,
        AlertField::Type,
    ];

    fn chave(campo: AlertField) -> String {
        serde_json::to_value(campo)
            .expect("serializa")
            .as_str()
            .expect("string")
            .to_string()
    }

    /// O teste que fecha o vão.
    ///
    /// Se alguém renomear uma constante em `fields.rs` sem mexer aqui, este
    /// teste falha — e como o binding é gerado a partir do enum, o
    /// `alertPresentation.ts` deixa de compilar no mesmo movimento.
    #[test]
    fn o_enum_exportado_e_exatamente_o_vocabulario_do_motor() {
        let do_enum: Vec<String> = TODAS.iter().copied().map(chave).collect();
        let do_motor: Vec<String> = ALERT_FIELDS
            .iter()
            .map(|campo| (*campo).to_string())
            .collect();
        assert_eq!(
            do_enum, do_motor,
            "o vocabulário do Rust e o tipo exportado divergiram — o frontend perderia o rótulo em silêncio"
        );
    }

    #[test]
    fn os_campos_de_saude_atravessam_a_fronteira() {
        for (campo, esperado) in [
            (AlertField::CpuUsagePercent, "cpuUsagePercent"),
            (AlertField::MemoryUsedPercent, "memoryUsedPercent"),
            (AlertField::StorageUsedPercent, "storageUsedPercent"),
            (AlertField::LoadAverage1m, "loadAverage1m"),
        ] {
            assert_eq!(chave(campo), esperado);
        }
    }
}
