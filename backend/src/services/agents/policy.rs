//! Política local do agente: o que este host aceita que a central faça.
//!
//! Configurada **no servidor remoto** (`AGENT_ALLOW`) e só anunciada à
//! central no `Hello`. A central usa a lista para não oferecer botões que
//! seriam recusados, mas quem decide é o agente, a cada pedido. Assim um
//! comprometimento da central não ganha `update`/`compose` num host que não
//! os liberou (ADR 011).

use std::{collections::BTreeSet, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum Permission {
    /// Inventário, inspeção, logs e métricas.
    Read,
    /// Start/stop/restart/remove, redes, volumes, imagens e limpeza de logs.
    Lifecycle,
    /// Pull de imagem e recriação de container.
    Update,
    /// Ações de projeto compose (executa o CLI no host).
    Compose,
    /// Monitores da central executados a partir deste site.
    Monitor,
    /// Varredura de rede a partir deste site.
    Discovery,
}

impl Permission {
    pub const ALL: [Self; 6] = [
        Self::Read,
        Self::Lifecycle,
        Self::Update,
        Self::Compose,
        Self::Monitor,
        Self::Discovery,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Lifecycle => "lifecycle",
            Self::Update => "update",
            Self::Compose => "compose",
            Self::Monitor => "monitor",
            Self::Discovery => "discovery",
        }
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Permission {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|permission| permission.as_str() == value.trim().to_ascii_lowercase())
            .ok_or_else(|| format!("permissão desconhecida: {value}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    allowed: BTreeSet<Permission>,
}

impl Default for Policy {
    /// Padrão conservador: ver e operar o ciclo de vida, medir e varrer. Pull,
    /// recriação e compose precisam ser liberados explicitamente no host.
    fn default() -> Self {
        Self::from_permissions([
            Permission::Read,
            Permission::Lifecycle,
            Permission::Monitor,
            Permission::Discovery,
        ])
    }
}

impl Policy {
    pub fn from_permissions(permissions: impl IntoIterator<Item = Permission>) -> Self {
        let mut allowed: BTreeSet<Permission> = permissions.into_iter().collect();
        // Operar sem enxergar não faz sentido: toda política inclui leitura.
        allowed.insert(Permission::Read);
        Self { allowed }
    }

    /// Lê a lista separada por vírgula (`read,lifecycle,update`). `all`
    /// libera tudo; vazio cai no padrão.
    ///
    /// # Errors
    ///
    /// Permissão desconhecida — erro de digitação não pode liberar nem negar
    /// nada em silêncio.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(Self::default());
        }
        if raw.eq_ignore_ascii_case("all") {
            return Ok(Self::from_permissions(Permission::ALL));
        }
        raw.split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(Permission::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map(Self::from_permissions)
    }

    #[must_use]
    pub fn allows(&self, permission: Permission) -> bool {
        self.allowed.contains(&permission)
    }

    #[must_use]
    pub fn permissions(&self) -> Vec<Permission> {
        self.allowed.iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padrao_nao_libera_update_nem_compose() {
        let policy = Policy::default();
        assert!(policy.allows(Permission::Read));
        assert!(policy.allows(Permission::Lifecycle));
        assert!(!policy.allows(Permission::Update));
        assert!(!policy.allows(Permission::Compose));
    }

    #[test]
    fn lista_explicita_sempre_inclui_leitura() {
        let policy = Policy::parse("update, compose").expect("política");
        assert_eq!(
            policy.permissions(),
            vec![Permission::Read, Permission::Update, Permission::Compose]
        );
        assert!(!policy.allows(Permission::Lifecycle));
    }

    #[test]
    fn all_libera_tudo_e_vazio_cai_no_padrao() {
        assert_eq!(Policy::parse("ALL").expect("all").permissions().len(), 6);
        assert_eq!(Policy::parse(" ").expect("vazio"), Policy::default());
    }

    #[test]
    fn permissao_desconhecida_e_erro() {
        assert!(Policy::parse("read,rooot").is_err());
    }
}
