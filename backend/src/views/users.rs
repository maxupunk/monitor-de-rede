use serde::Serialize;

use crate::models::_entities::users;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDetailResponse {
    pub id: i64,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<users::Model> for UserDetailResponse {
    fn from(user: users::Model) -> Self {
        Self {
            id: user.id,
            email: user.email,
            full_name: user.name,
            role: user.role,
            active: user.active,
            created_at: user.created_at.to_rfc3339(),
            updated_at: user.updated_at.to_rfc3339(),
        }
    }
}
