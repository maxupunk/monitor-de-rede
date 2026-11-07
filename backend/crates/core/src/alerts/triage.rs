//! O campo do alerta com a opinião do Decisor (a triagem do assistente). O
//! assistente grava; a apresentação do alerta, no núcleo, lê.

use serde_json::Value;

/// Campo de `alert_events.data` com a opinião do Decisor.
pub const TRIAGE_FIELD: &str = "deciderTriage";
/// O mesmo campo em alertas gravados antes do renome para Decisor.
const LEGACY_TRIAGE_FIELD: &str = "layaTriage";

/// Alerta gravado antes do renome traz a opinião em `layaTriage`: quem lê
/// recebe em [`TRIAGE_FIELD`], sem migrar o JSON de cada alerta no banco.
pub fn upgrade_legacy_triage(data: &mut Value) {
    let Some(object) = data.as_object_mut() else {
        return;
    };
    if let Some(legacy) = object.remove(LEGACY_TRIAGE_FIELD) {
        object.entry(TRIAGE_FIELD).or_insert(legacy);
    }
}
