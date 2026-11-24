//! Eventos SSE do servidor MCP (AGENTS §9): `mcp:changed` avisa a tela aberta
//! quando configurações ou tokens mudam — atualização imediata sem polling.
//!
//! Nenhuma área opcional reivindica o prefixo `mcp:`, então `EventAudience`
//! entrega o evento a toda sessão autenticada no Núcleo (`Area::Core`).

use loco_rs::app::AppContext;
use serde_json::json;

use netmonitor_core::events::EventBus;

pub const CHANGED_EVENT: &str = "mcp:changed";

pub fn publish_changed(ctx: &AppContext) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        bus.publish_ephemeral(CHANGED_EVENT, json!({}));
    }
}
