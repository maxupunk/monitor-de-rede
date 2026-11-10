//! O registry pelo barramento SSE (AGENTS §9): `registry:changed` avisa que
//! o catálogo mudou (imagem nova, análise concluída, coleta) — a tela aberta
//! recarrega a visão uma vez, sem polling.

use loco_rs::app::AppContext;
use serde_json::json;

use netmonitor_core::events::EventBus;

pub const CHANGED_EVENT: &str = "registry:changed";

pub fn publish_changed(ctx: &AppContext) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        bus.publish_ephemeral(CHANGED_EVENT, json!({}));
    }
}
