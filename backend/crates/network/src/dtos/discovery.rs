//! Entrada da varredura da descoberta.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryScanInput {
    pub network_id: i64,
}
