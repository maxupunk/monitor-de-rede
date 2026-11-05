//! O que o motor de alertas compartilha com o resto da central: os
//! contratos (`contracts`), o feed SSE (`feed`), a chave da classificação do
//! episódio e o peso topológico de cada tipo de equipamento. O motor em si
//! continua fora do núcleo.

pub mod contracts;
pub mod feed;

/// Chave de `alert_events.data` com a classificação do episódio (camelCase,
/// como `lastProblemAt`).
pub const PROBLEM_KIND: &str = "problemKind";

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
