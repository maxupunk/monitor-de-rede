//! Tempo real do teste de enlace — só SSE (AGENTS §9, ADR 015 §5).
//!
//! * `link_test:updated` — o `LinkTest` inteiro (sem amostras) a cada
//!   mudança. Enquanto o teste roda, é o **snapshot** do barramento: quem abre
//!   a tela depois recebe o estado atual logo ao conectar, sem HTTP. No fim, o
//!   snapshot é esquecido e o estado final sai como evento comum.
//! * `link_test:sample` — efêmero, `{ testId, legSeq, flow, bps, tMs, atMs }`.
//! * `link_test:health` — efêmero, `{ testId, legSeq, deviceId, atMs, cpu,
//!   memory }`: CPU e memória de uma ponta durante a medição.
//! * `link_test_schedule:updated` — `{ id }`; a tela relê a lista.

use serde_json::json;

use super::{
    health::LinkTestHealthEvent,
    model::{LinkTest, LinkTestSampleEvent, LinkTestScheduleEvent},
};
use crate::services::events::EventBus;

pub const UPDATED_EVENT: &str = "link_test:updated";
pub const SAMPLE_EVENT: &str = "link_test:sample";
pub const HEALTH_EVENT: &str = "link_test:health";
pub const SCHEDULE_EVENT: &str = "link_test_schedule:updated";

/// Chave do snapshot de um teste no barramento.
#[must_use]
pub fn snapshot_key(test_id: i64) -> String {
    format!("link_test:{test_id}")
}

/// Publica o estado do teste: snapshot enquanto ativo, evento final depois.
pub fn publish_test(bus: Option<&EventBus>, test: &LinkTest) {
    let Some(bus) = bus else { return };
    let mut payload = test.clone();
    for leg in &mut payload.legs {
        leg.samples = None;
        leg.health = None;
    }
    let payload = json!(payload);
    let key = snapshot_key(test.id);
    if test.status.is_active() {
        bus.publish_snapshot(UPDATED_EVENT, key, payload);
    } else {
        bus.forget_snapshots(&key);
        bus.publish_ephemeral(UPDATED_EVENT, payload);
    }
}

/// Publica uma amostra ao vivo.
pub fn publish_sample(bus: Option<&EventBus>, sample: LinkTestSampleEvent) {
    if let Some(bus) = bus {
        bus.publish_ephemeral(SAMPLE_EVENT, json!(sample));
    }
}

/// Publica CPU/memória de uma ponta ao vivo.
pub fn publish_health(bus: Option<&EventBus>, event: LinkTestHealthEvent) {
    if let Some(bus) = bus {
        bus.publish_ephemeral(HEALTH_EVENT, json!(event));
    }
}

/// Avisa que a lista de agendamentos mudou.
pub fn publish_schedule(bus: Option<&EventBus>, id: i64) {
    if let Some(bus) = bus {
        bus.publish_ephemeral(SCHEDULE_EVENT, json!(LinkTestScheduleEvent { id }));
    }
}
