pub mod alerts;
pub mod common;
pub mod devices;
pub mod docker;
pub mod logs;
pub mod monitors;
pub mod resources;
pub mod saas;
pub mod server_addresses;
pub mod users;

/// Lê um corpo de requisição **opcional** sem deixá-lo mascarar o erro real.
///
/// O extrator `Json` do axum rejeita com **400** um corpo vazio ou sem
/// `Content-Type: application/json`, e essa rejeição acontece *antes* do
/// handler. Nos endpoints em que todo campo tem default — o heartbeat do probe,
/// o silenciamento de alerta — isso trocaria o 401/404 correto por um
/// "requisição malformada" que não diz nada a quem chamou. Aqui o corpo chega
/// como texto (`body: String`), um payload ausente ou ilegível vira o `Default`
/// do DTO, e a autenticação ou a busca do recurso voltam a ser a primeira coisa
/// a falhar.
///
/// Só vale para DTOs cujos campos são todos opcionais. Onde há campo
/// obrigatório, use `Json<T>` e deixe o 400 acontecer.
pub fn optional_body<T: serde::de::DeserializeOwned + Default>(body: &str) -> T {
    if body.trim().is_empty() {
        return T::default();
    }
    serde_json::from_str(body).unwrap_or_default()
}

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
