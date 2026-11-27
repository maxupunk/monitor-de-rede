//! O que o motor de alertas compartilha com o resto da central: os
//! contratos (`contracts`), o feed SSE (`feed`), a chave da classificação do
//! episódio e o peso topológico de cada tipo de equipamento. O motor em si
//! continua fora do núcleo.

pub mod contracts;
pub mod datasets;
pub mod feed;
pub mod fields;
pub mod triage;

/// Chave de `alert_events.data` com a classificação do episódio (camelCase,
/// como `lastProblemAt`).
pub const PROBLEM_KIND: &str = "problemKind";

/// Chave de `alert_events.data` com a origem do alerta quando ele não tem
/// equipamento nem monitor ("Registry · vulnerabilidades") — o cartão em
/// `/alerts` a mostra no lugar de "Geral".
pub const SOURCE_LABEL: &str = "sourceLabel";

/// Chave de `alert_events.data` com como o alerta fecha sozinho, quando o
/// produtor sabe dizer — o cartão a mostra no lugar do aviso genérico de
/// alerta de evento.
pub const CLOSES_WHEN: &str = "closesWhen";

/// Peso de prioridade topológica por tipo de equipamento.
#[must_use]
pub fn role_weight(device_type: &str) -> i32 {
    match device_type.to_lowercase().as_str() {
        "gateway" => 100,
        "router" => 90,
        "firewall" => 85,
        "switch" => 80,
        "access_point" | "ap" => 60,
        "vpn_peer" | "vpn" => 50,
        "server" => 40,
        _ => 20,
    }
}
