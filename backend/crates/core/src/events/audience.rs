//! Quem recebe cada evento do SSE (ADR 019).
//!
//! O barramento é global; cada conexão filtra o que entrega: evento de área
//! só a quem tem acesso a ela e evento `user:*` só ao próprio usuário. O
//! acesso acompanha a sessão — quando chega o `user:access_updated` deste
//! usuário, o filtro passa a valer o novo acesso sem reconectar.

use crate::{
    areas::AreaPolicies,
    events::DomainEvent,
    roles::{AreaAccess, ACCESS_UPDATED_EVENT, USER_EVENT_PREFIX},
};

pub struct EventAudience {
    user_id: Option<i64>,
    access: AreaAccess,
    policies: AreaPolicies,
}

impl EventAudience {
    #[must_use]
    pub const fn new(user_id: Option<i64>, access: AreaAccess, policies: AreaPolicies) -> Self {
        Self {
            user_id,
            access,
            policies,
        }
    }

    /// O evento pode ir a esta conexão? Atualiza o acesso quando o evento é a
    /// mudança de acesso deste usuário.
    pub fn admits(&mut self, event: &DomainEvent) -> bool {
        if event.event_type.starts_with(USER_EVENT_PREFIX) {
            let own = self.user_id.is_some()
                && event
                    .payload
                    .get("userId")
                    .and_then(serde_json::Value::as_i64)
                    == self.user_id;
            if own && event.event_type == ACCESS_UPDATED_EVENT {
                if let Some(access) = event
                    .payload
                    .get("role")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|role| {
                        AreaAccess::from_stored(role, event.payload.get("areaRoles")).ok()
                    })
                {
                    self.access = access;
                }
            }
            return own;
        }
        let area = self.policies.area_of_event(&event.event_type);
        self.access.role_in(area).is_some()
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use sea_orm::DatabaseConnection;
    use serde_json::json;

    use super::*;
    use crate::{
        areas::{Area, AreaPolicy, Consequence, Evidence},
        roles::Role,
        shared::errors::AppResult,
    };

    struct Docker;

    #[async_trait]
    impl AreaPolicy for Docker {
        fn area(&self) -> Area {
            Area::Servers
        }
        async fn evidence(&self, _db: &DatabaseConnection) -> AppResult<Evidence> {
            unreachable!()
        }
        async fn consequences(&self, _db: &DatabaseConnection) -> AppResult<Vec<Consequence>> {
            unreachable!()
        }
        fn event_prefixes(&self) -> &'static [&'static str] {
            &["docker:"]
        }
    }

    fn evento(tipo: &str, payload: serde_json::Value) -> DomainEvent {
        DomainEvent {
            event_type: tipo.into(),
            payload,
            occurred_at: String::new(),
        }
    }

    fn plateia(areas: &[(Area, Role)]) -> EventAudience {
        EventAudience::new(
            Some(7),
            AreaAccess::new(Role::Operator, areas.iter().copied().collect()),
            AreaPolicies::new(vec![Box::new(Docker)]),
        )
    }

    #[test]
    fn evento_de_area_so_chega_a_quem_tem_acesso() {
        let docker = evento("docker:snapshot", json!({}));
        assert!(!plateia(&[]).admits(&docker));
        assert!(plateia(&[(Area::Servers, Role::Viewer)]).admits(&docker));
        assert!(
            plateia(&[]).admits(&evento("monitor:result", json!({}))),
            "núcleo chega"
        );
    }

    #[test]
    fn evento_de_usuario_so_chega_a_ele_e_atualiza_o_acesso() {
        let mut minha = plateia(&[]);
        let outro = evento(
            ACCESS_UPDATED_EVENT,
            json!({ "userId": 8, "role": "viewer" }),
        );
        assert!(!minha.admits(&outro));
        let meu = evento(
            ACCESS_UPDATED_EVENT,
            json!({ "userId": 7, "role": "operator", "areaRoles": { "servers": "viewer" } }),
        );
        assert!(minha.admits(&meu));
        assert!(
            minha.admits(&evento("docker:snapshot", json!({}))),
            "o acesso novo vale na mesma conexão"
        );
    }
}
