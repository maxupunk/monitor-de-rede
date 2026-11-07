//! O menu de cada usuário (Fase 16): itens fixados no topo e itens ocultos.
//!
//! Fica no servidor para valer em qualquer navegador e no celular; a mudança
//! chega às outras abas pelo SSE (`user:menu_updated`, só ao próprio usuário).
//! A chave de cada item é o caminho dele (`/docker`, `/topology`): o menu é
//! montado pelo frontend a partir das áreas, e o caminho é o que não muda.

use sea_orm::{ActiveModelTrait, ConnectionTrait, IntoActiveModel, Set};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::_entities::users,
    services::shared::errors::{AppError, AppResult},
};

/// Evento do SSE quando o menu de um usuário muda.
pub const MENU_UPDATED_EVENT: &str = "user:menu_updated";

/// Mais que isso não é personalizar, é refazer o menu.
pub const MAX_ITEMS: usize = 60;

const MAX_PATH_LEN: usize = 200;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct MenuPrefs {
    /// Caminhos fixados no topo do menu, na ordem em que aparecem.
    #[serde(default)]
    pub pinned: Vec<String>,
    /// Caminhos que não aparecem no menu.
    #[serde(default)]
    pub hidden: Vec<String>,
}

impl MenuPrefs {
    /// O menu gravado no usuário; o que não for válido é ignorado.
    #[must_use]
    pub fn of_user(user: &users::Model) -> Self {
        user.menu_prefs
            .clone()
            .and_then(|value| serde_json::from_value::<Self>(value).ok())
            .and_then(|prefs| prefs.normalized().ok())
            .unwrap_or_default()
    }

    /// Confere e arruma: caminhos de rota (`/…`), sem repetição, no máximo
    /// [`MAX_ITEMS`] de cada; item fixado não fica oculto.
    ///
    /// # Errors
    ///
    /// Caminho inválido ou lista longa demais.
    pub fn normalized(self) -> AppResult<Self> {
        fn clean(items: Vec<String>, what: &str) -> AppResult<Vec<String>> {
            let mut seen = Vec::with_capacity(items.len());
            for item in items {
                let item = item.trim().to_owned();
                if !item.starts_with('/') || item.len() > MAX_PATH_LEN {
                    return Err(AppError::validation(format!(
                        "Item de menu inválido em {what}: \"{item}\"."
                    )));
                }
                if !seen.contains(&item) {
                    seen.push(item);
                }
            }
            if seen.len() > MAX_ITEMS {
                return Err(AppError::validation(format!(
                    "No máximo {MAX_ITEMS} itens em {what}."
                )));
            }
            Ok(seen)
        }
        let pinned = clean(self.pinned, "fixados")?;
        let hidden = clean(self.hidden, "ocultos")?
            .into_iter()
            .filter(|item| !pinned.contains(item))
            .collect();
        Ok(Self { pinned, hidden })
    }
}

/// Grava o menu do usuário.
///
/// # Errors
///
/// Menu inválido ou falha do banco.
pub async fn save<C: ConnectionTrait>(
    db: &C,
    user: users::Model,
    prefs: MenuPrefs,
) -> AppResult<MenuPrefs> {
    let prefs = prefs.normalized()?;
    let mut active = user.into_active_model();
    active.menu_prefs = Set(Some(serde_json::to_value(&prefs).map_err(|error| {
        AppError::Internal(anyhow::anyhow!("menu não serializa: {error}"))
    })?));
    active.update(db).await?;
    Ok(prefs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu(pinned: &[&str], hidden: &[&str]) -> MenuPrefs {
        MenuPrefs {
            pinned: pinned.iter().map(|item| (*item).to_owned()).collect(),
            hidden: hidden.iter().map(|item| (*item).to_owned()).collect(),
        }
    }

    #[test]
    fn fixado_nao_fica_oculto_e_repeticao_sai() {
        let arrumado = menu(&["/docker", "/docker", "/topology"], &["/docker", "/logs"])
            .normalized()
            .unwrap();
        assert_eq!(arrumado, menu(&["/docker", "/topology"], &["/logs"]));
    }

    #[test]
    fn caminho_que_nao_e_rota_e_recusado() {
        assert!(menu(&["docker"], &[]).normalized().is_err());
        let longa: Vec<String> = (0..=MAX_ITEMS).map(|i| format!("/x{i}")).collect();
        assert!(MenuPrefs {
            pinned: longa,
            hidden: Vec::new()
        }
        .normalized()
        .is_err());
    }
}
