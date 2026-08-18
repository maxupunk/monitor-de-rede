//! Presenters do contrato de autenticação consumido pelo frontend.

use serde::{Deserialize, Serialize};

use crate::models::_entities::users;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

impl LoginResponse {
    #[must_use]
    pub fn new(user: &users::Model, token: &str) -> Self {
        Self {
            token: token.to_owned(),
            user: UserResponse::new(user),
        }
    }
}

/// Estado da instalação, consultado antes de decidir entre a tela de login e a
/// de cadastro inicial.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatusResponse {
    /// `true` enquanto o banco não tiver nenhum usuário.
    pub needs_setup: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: i64,
    pub email: String,
    pub full_name: String,
    pub role: String,
}

impl UserResponse {
    #[must_use]
    pub fn new(user: &users::Model) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            full_name: user.name.clone(),
            // O esquema atual possui um único perfil de operador.
            role: user.role.clone(),
        }
    }
}
