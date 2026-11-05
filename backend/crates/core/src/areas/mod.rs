//! Áreas do produto (ADR 017).
//!
//! Uma instalação liga só o que usa: **Rede**, **Servidores** e **VPN** são
//! opcionais; o **Núcleo** nunca desliga. A área decide **relevância** — o que
//! a tela mostra e quais rotinas próprias rodam —, nunca permissão (quem
//! autoriza é o papel) e nunca apaga dado.
//!
//! Este módulo guarda o estado (`system_settings`, chave [`STORAGE_KEY`]),
//! valida as mudanças e as publica no SSE ([`EVENT`]). O que cada área sabe
//! sobre si mesma — a evidência de uso, as consequências de desligar e o que
//! impede o desligamento — vem de uma [`AreaPolicy`] implementada pela própria
//! área e registrada pelo ponto de montagem: o núcleo não conhece área nenhuma.

mod enabled;
mod policy;
mod state;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use enabled::EnabledAreas;
pub use policy::{monitors_keep_running, AreaPolicies, AreaPolicy, Blocker, Consequence, Evidence};
pub use state::{
    is_enabled, load, publish, save, snapshot, AreaImpact, AreaService, AreaState, AreasSnapshot,
    EVENT, STORAGE_KEY,
};

/// Uma área do produto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum Area {
    /// Sempre ligada: dashboard, dispositivos, monitores, alertas, IA…
    Core,
    /// Topologia, descoberta, enlaces, Wi-Fi, sondas e plugins de dispositivo.
    Network,
    /// Docker, servidores e backup de bancos.
    Servers,
    /// Servidor WireGuard e dispositivos VPN.
    Vpn,
}

impl Area {
    /// Todas, na ordem em que a tela as apresenta.
    pub const ALL: [Self; 4] = [Self::Core, Self::Network, Self::Servers, Self::Vpn];

    /// As que podem ser desligadas.
    pub const OPTIONAL: [Self; 3] = [Self::Network, Self::Servers, Self::Vpn];

    /// Identificador estável (o mesmo do JSON e da URL).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Network => "network",
            Self::Servers => "servers",
            Self::Vpn => "vpn",
        }
    }

    /// Nome na tela.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Core => "Núcleo",
            Self::Network => "Rede",
            Self::Servers => "Servidores e Docker",
            Self::Vpn => "VPN WireGuard",
        }
    }

    /// Uma frase sobre o que a área traz.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::Core => {
                "Dashboard, dispositivos, monitores, alertas, eventos, logs, Assistente IA e \
                 administração. Sempre ligado."
            }
            Self::Network => {
                "Topologia, descoberta, redes IP, testes de velocidade, sinais Wi-Fi, sondas e \
                 plugins de roteadores e switches."
            }
            Self::Servers => {
                "Containers, projetos compose, imagens, volumes e redes Docker dos servidores, e \
                 o backup dos bancos de dados."
            }
            Self::Vpn => {
                "Servidor WireGuard para alcançar redes remotas e os dispositivos do túnel."
            }
        }
    }

    /// Ícone (Material Design Icons).
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Core => "mdi-shield-network",
            Self::Network => "mdi-lan",
            Self::Servers => "mdi-docker",
            Self::Vpn => "mdi-shield-lock-outline",
        }
    }

    /// Pode ser desligada?
    #[must_use]
    pub const fn is_optional(self) -> bool {
        !matches!(self, Self::Core)
    }

    /// A área pelo identificador.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|area| area.id() == id)
    }
}

impl Area {
    const fn bit(self) -> u8 {
        match self {
            Self::Core => 1,
            Self::Network => 1 << 1,
            Self::Servers => 1 << 2,
            Self::Vpn => 1 << 3,
        }
    }
}

/// Conjunto de áreas ligadas, `Copy` e montável em contexto `const` — para
/// caber em políticas que são `Copy` (como a das ferramentas da IA). O núcleo
/// está sempre dentro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AreaSet(u8);

impl AreaSet {
    /// Todas ligadas — o padrão de quem ainda não consultou o estado.
    pub const ALL: Self = Self(0b1111);

    /// Só o que está ligado em `enabled`.
    #[must_use]
    pub fn from_enabled(enabled: &EnabledAreas) -> Self {
        Self(
            Area::ALL
                .into_iter()
                .filter(|area| enabled.is_enabled(*area))
                .fold(Area::Core.bit(), |bits, area| bits | area.bit()),
        )
    }

    #[must_use]
    pub const fn contains(self, area: Area) -> bool {
        matches!(area, Area::Core) || self.0 & area.bit() != 0
    }

    /// O mesmo conjunto sem a área (o núcleo não sai).
    #[must_use]
    pub const fn without(self, area: Area) -> Self {
        if matches!(area, Area::Core) {
            self
        } else {
            Self(self.0 & !area.bit())
        }
    }
}

impl Default for AreaSet {
    fn default() -> Self {
        Self::ALL
    }
}

impl std::fmt::Display for Area {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identificador_ida_e_volta() {
        for area in Area::ALL {
            assert_eq!(Area::from_id(area.id()), Some(area));
        }
        assert_eq!(Area::from_id("docker"), None);
    }

    #[test]
    fn so_o_nucleo_e_obrigatorio() {
        assert!(!Area::Core.is_optional());
        assert!(Area::OPTIONAL.iter().all(|area| area.is_optional()));
        assert_eq!(Area::OPTIONAL.len() + 1, Area::ALL.len());
    }

    #[test]
    fn conjunto_sempre_contem_o_nucleo() {
        let mut enabled = EnabledAreas::all_on();
        enabled.set(Area::Servers, false);
        let set = AreaSet::from_enabled(&enabled);
        assert!(set.contains(Area::Core));
        assert!(set.contains(Area::Network));
        assert!(!set.contains(Area::Servers));
        assert!(AreaSet::ALL.without(Area::Core).contains(Area::Core));
        assert!(!AreaSet::ALL.without(Area::Vpn).contains(Area::Vpn));
    }

    #[test]
    fn serializa_com_o_mesmo_identificador() {
        assert_eq!(
            serde_json::to_value(Area::Servers).expect("json"),
            serde_json::json!("servers")
        );
    }
}
