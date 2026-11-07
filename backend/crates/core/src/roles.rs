//! O papel de um usuário (`admin`, `operator`, `viewer`): quem autoriza na
//! API e nas ferramentas da IA.

use std::str::FromStr;

use crate::{
    areas::{Area, AreaSet},
    shared::errors::{AppError, AppResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Operator,
    Viewer,
}

impl Role {
    pub const ADMIN: &'static str = "admin";
    pub const OPERATOR: &'static str = "operator";
    pub const VIEWER: &'static str = "viewer";

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admin => Self::ADMIN,
            Self::Operator => Self::OPERATOR,
            Self::Viewer => Self::VIEWER,
        }
    }

    #[must_use]
    pub const fn can_write(self) -> bool {
        matches!(self, Self::Admin | Self::Operator)
    }

    #[must_use]
    pub const fn can_manage_users(self) -> bool {
        matches!(self, Self::Admin)
    }

    #[must_use]
    pub const fn can_manage_docker(self) -> bool {
        matches!(self, Self::Admin)
    }

    /// Plugins de dispositivo executam comandos no equipamento com a
    /// credencial dele — mesma régua do Docker.
    #[must_use]
    pub const fn can_manage_plugins(self) -> bool {
        matches!(self, Self::Admin)
    }
}

impl FromStr for Role {
    type Err = AppError;

    fn from_str(value: &str) -> AppResult<Self> {
        match value {
            Self::ADMIN => Ok(Self::Admin),
            Self::OPERATOR => Ok(Self::Operator),
            Self::VIEWER => Ok(Self::Viewer),
            _ => Err(AppError::validation(
                "Perfil inválido. Use admin, operator ou viewer.",
            )),
        }
    }
}

/// Evento do SSE quando o acesso de um usuário muda (`{ userId, role,
/// areaRoles }`). Eventos `user:*` só chegam à conexão do próprio usuário.
pub const ACCESS_UPDATED_EVENT: &str = "user:access_updated";

/// Prefixo dos eventos que pertencem a um usuário só.
pub const USER_EVENT_PREFIX: &str = "user:";

/// O papel de um usuário em cada área (ADR 019) — a regra única que o guard da
/// API, a IA, o SSE e as notificações consultam.
///
/// O administrador global é administrador em todas as áreas. Os demais seguem
/// o papel global no Núcleo e, em cada área opcional, o papel atribuído a ela:
/// `operator`, `viewer` ou, sem atribuição, **sem acesso**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaAccess {
    global: Role,
    areas: std::collections::BTreeMap<Area, Role>,
}

impl AreaAccess {
    #[must_use]
    pub fn new(global: Role, areas: std::collections::BTreeMap<Area, Role>) -> Self {
        Self { global, areas }
    }

    /// O acesso gravado no usuário (`role` + `area_roles`).
    ///
    /// # Errors
    ///
    /// Papel global inválido. Papel de área inválido é descartado (sem acesso).
    pub fn of_user(user: &crate::models::users::Model) -> AppResult<Self> {
        Self::from_stored(&user.role, user.area_roles.as_ref())
    }

    /// O acesso a partir do que fica gravado (ou viaja no evento
    /// [`ACCESS_UPDATED_EVENT`]): o papel global e o mapa de papéis por área.
    ///
    /// # Errors
    ///
    /// Papel global inválido. Papel de área inválido é descartado (sem acesso).
    pub fn from_stored(role: &str, area_roles: Option<&serde_json::Value>) -> AppResult<Self> {
        let global = Role::from_str(role)?;
        let areas = area_roles
            .and_then(serde_json::Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(id, role)| {
                        let area = Area::from_id(id).filter(|area| area.is_optional())?;
                        let role = role.as_str().and_then(|text| Role::from_str(text).ok())?;
                        (role != Role::Admin).then_some((area, role))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self::new(global, areas))
    }

    /// O acesso de um usuário pelo id. Sem usuário (ou sem achar), só o
    /// Núcleo e só leitura — nunca mais do que se sabe.
    pub async fn load<C: sea_orm::ConnectionTrait>(db: &C, user_id: Option<i64>) -> Self {
        use sea_orm::EntityTrait;
        let user = match user_id {
            Some(id) => crate::models::users::Entity::find_by_id(id)
                .one(db)
                .await
                .ok()
                .flatten(),
            None => None,
        };
        user.and_then(|user| Self::of_user(&user).ok())
            .unwrap_or_else(|| Self::new(Role::Viewer, std::collections::BTreeMap::new()))
    }

    #[must_use]
    pub const fn global(&self) -> Role {
        self.global
    }

    /// O papel na área; `None` é sem acesso.
    #[must_use]
    pub fn role_in(&self, area: Area) -> Option<Role> {
        match (self.global, area) {
            (Role::Admin, _) => Some(Role::Admin),
            (global, Area::Core) => Some(global),
            _ => self.areas.get(&area).copied(),
        }
    }

    /// As áreas que o usuário enxerga (o Núcleo sempre).
    #[must_use]
    pub fn visible(&self) -> AreaSet {
        AreaSet::of(
            Area::OPTIONAL
                .into_iter()
                .filter(|area| self.role_in(*area).is_some()),
        )
    }

    /// As áreas opcionais em que o usuário pode mudar algo. O Núcleo está em
    /// todo `AreaSet`: a escrita nele segue o papel global, que o guard da API
    /// já aplica (o leitor global nem abre o chat da IA, que é um `POST`).
    #[must_use]
    pub fn writable(&self) -> AreaSet {
        AreaSet::of(
            Area::OPTIONAL
                .into_iter()
                .filter(|area| self.role_in(*area).is_some_and(Role::can_write)),
        )
    }

    /// Os papéis atribuídos por área, como a API os devolve.
    #[must_use]
    pub fn area_roles(&self) -> std::collections::BTreeMap<Area, &'static str> {
        self.areas
            .iter()
            .map(|(area, role)| (*area, role.as_str()))
            .collect()
    }

    /// Lê e valida os papéis por área de um formulário: só áreas opcionais e só
    /// `operator`/`viewer` (não há admin por área).
    ///
    /// # Errors
    ///
    /// Área ou papel desconhecido.
    pub fn parse_area_roles(
        input: &std::collections::BTreeMap<String, String>,
    ) -> AppResult<std::collections::BTreeMap<Area, Role>> {
        input
            .iter()
            .map(|(id, role)| {
                let area = Area::from_id(id)
                    .filter(|area| area.is_optional())
                    .ok_or_else(|| AppError::validation(format!("Área \"{id}\" desconhecida.")))?;
                match Role::from_str(role)? {
                    Role::Admin => Err(AppError::validation(
                        "Não existe administrador por área: use operator ou viewer.",
                    )),
                    role => Ok((area, role)),
                }
            })
            .collect()
    }

    /// Os papéis por área no formato gravado em `users.area_roles`.
    #[must_use]
    pub fn to_json(areas: &std::collections::BTreeMap<Area, Role>) -> serde_json::Value {
        serde_json::Value::Object(
            areas
                .iter()
                .map(|(area, role)| (area.id().to_owned(), role.as_str().into()))
                .collect(),
        )
    }
}

#[cfg(test)]
mod area_access_tests {
    use std::collections::BTreeMap;

    use super::*;

    fn acesso(global: Role, areas: &[(Area, Role)]) -> AreaAccess {
        AreaAccess::new(global, areas.iter().copied().collect())
    }

    #[test]
    fn admin_global_vale_em_todas_as_areas() {
        let admin = acesso(Role::Admin, &[]);
        for area in Area::ALL {
            assert_eq!(admin.role_in(area), Some(Role::Admin));
        }
        assert!(admin.visible().contains(Area::Servers));
    }

    #[test]
    fn sem_papel_na_area_e_sem_acesso_e_o_nucleo_segue_o_global() {
        let operador_docker = acesso(Role::Viewer, &[(Area::Servers, Role::Operator)]);
        assert_eq!(operador_docker.role_in(Area::Core), Some(Role::Viewer));
        assert_eq!(operador_docker.role_in(Area::Servers), Some(Role::Operator));
        assert_eq!(operador_docker.role_in(Area::Network), None);
        assert!(!operador_docker.visible().contains(Area::Network));
        assert!(operador_docker.writable().contains(Area::Servers));
        assert!(!operador_docker.writable().contains(Area::Vpn));
    }

    #[test]
    fn formulario_recusa_admin_por_area_e_area_desconhecida() {
        let pedido = |pares: &[(&str, &str)]| {
            AreaAccess::parse_area_roles(
                &pares
                    .iter()
                    .map(|(a, r)| ((*a).to_owned(), (*r).to_owned()))
                    .collect::<BTreeMap<_, _>>(),
            )
        };
        assert!(pedido(&[("servers", "admin")]).is_err());
        assert!(pedido(&[("core", "viewer")]).is_err());
        assert!(pedido(&[("lua", "viewer")]).is_err());
        let ok = pedido(&[("vpn", "viewer")]).unwrap();
        assert_eq!(
            AreaAccess::to_json(&ok),
            serde_json::json!({ "vpn": "viewer" })
        );
    }
}
