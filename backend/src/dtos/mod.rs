pub mod agents;
pub mod ai;
pub mod alerts;
pub mod areas;
pub mod backup;
pub mod common;
pub mod devices;
pub mod diagnostics;
pub mod logs;
pub mod monitors;
pub mod plugins;
pub mod resources;
pub mod saas;
pub mod server_addresses;
pub mod storages;
pub mod users;
pub mod wifi;

/// O leitor de corpo opcional mora no núcleo, para as áreas usarem o mesmo.
pub use netmonitor_core::shared::body::optional_body;
/// Os contratos do SNMP são da área Rede (crate `network`).
pub use netmonitor_network::dtos::snmp;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtos::resources::SilenceInput;

    #[test]
    fn corpo_ausente_ou_ilegivel_cai_no_default() {
        assert!(optional_body::<SilenceInput>("").minutes.is_none());
        assert!(optional_body::<SilenceInput>("   ").minutes.is_none());
        assert!(optional_body::<SilenceInput>("{nao e json")
            .minutes
            .is_none());
        assert_eq!(
            optional_body::<SilenceInput>(r#"{"minutes":30}"#).minutes,
            Some(30)
        );
    }
}
