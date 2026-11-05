//! O contrato das ferramentas mora no núcleo (`netmonitor_core::ai`): cada
//! área contribui as suas, e o catálogo é montado no ponto de montagem. Este
//! módulo só reúne, no caminho de sempre, o que o laço do agente usa.

pub use netmonitor_core::ai::{
    args::ToolArgs,
    lookup::{
        device_ip, device_matches, device_not_found, fill_device_ip, monitor_target,
        target_device_id,
    },
    risk::{self, AiToolRisk, Approval, RiskAssessment},
    tools::*,
};
