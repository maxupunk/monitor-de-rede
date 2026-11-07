use std::collections::BTreeMap;

use netmonitor_core::{areas::Area, roles::AreaAccess};
use serde::Serialize;

use crate::models::_entities::users;

/// Os papéis por área de um usuário, como a tela os lê (ADR 019).
#[must_use]
pub fn area_roles_of(user: &users::Model) -> BTreeMap<Area, String> {
    AreaAccess::of_user(user)
        .map(|access| {
            access
                .area_roles()
                .into_iter()
                .map(|(area, role)| (area, role.to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDetailResponse {
    pub id: i64,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub area_roles: BTreeMap<Area, String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<users::Model> for UserDetailResponse {
    fn from(user: users::Model) -> Self {
        let area_roles = area_roles_of(&user);
        Self {
            id: user.id,
            email: user.email,
            full_name: user.name,
            area_roles,
            role: user.role,
            active: user.active,
            created_at: user.created_at.to_rfc3339(),
            updated_at: user.updated_at.to_rfc3339(),
        }
    }
}
