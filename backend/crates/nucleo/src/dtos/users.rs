use std::collections::BTreeMap;

use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserInput {
    #[validate(length(min = 2, message = "O nome precisa ter ao menos 2 caracteres."))]
    pub name: String,
    #[validate(email(message = "Informe um e-mail válido."))]
    pub email: String,
    #[validate(custom(function = "crate::models::users::validate_password"))]
    pub password: String,
    pub role: String,
    pub active: Option<bool>,
    /// Papel por área (ADR 019): `{ "servers": "operator" }`. Área ausente é
    /// sem acesso; o admin vale em todas.
    #[serde(default)]
    pub area_roles: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserInput {
    #[validate(length(min = 2, message = "O nome precisa ter ao menos 2 caracteres."))]
    pub name: String,
    #[validate(email(message = "Informe um e-mail válido."))]
    pub email: String,
    pub password: Option<String>,
    pub role: String,
    pub active: bool,
    #[serde(default)]
    pub area_roles: BTreeMap<String, String>,
}
