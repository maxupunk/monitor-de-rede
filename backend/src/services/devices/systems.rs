//! O catálogo de sistemas dos equipamentos — **um só**, para todas as telas.
//!
//! # Por que existir
//!
//! A mesma pergunta era feita em três lugares com três vocabulários diferentes:
//!
//! - o assistente da VPN oferecia `mikrotik`, `openwrt`, `linux`, `windows` e
//!   `mobile`, que são os perfis para os quais existe gerador de configuração;
//! - a ativação de log oferecia `routeros`, `openwrt`, `ubiquiti` e `linux`, que
//!   são as receitas de syslog;
//! - o cadastro do dispositivo pedia "Fabricante" em texto livre, que costuma
//!   vir do OUI do MAC e identifica **quem fez a placa**, não o sistema que roda
//!   nela.
//!
//! Três listas para uma coisa só. O operador que cadastrava "MikroTik" no texto
//! livre, escolhia o perfil `mikrotik` na VPN e depois via `routeros` na tela de
//! log não tinha como saber que eram a mesma escolha — e a cada tela nova a
//! divergência crescia.
//!
//! Aqui a lista é uma. Cada entrada diz o que aquele sistema **suporta**, e são
//! as capacidades que os subsistemas consultam:
//!
//! | campo | quem lê |
//! |---|---|
//! | [`OperatingSystem::syslog`] | a ativação de log — o `id` é a chave da receita em [`crate::services::syslog::snippets`] |
//! | [`OperatingSystem::mac_telnet`] | o seletor de meio de acesso |
//! | [`OperatingSystem::vpn_profile`] | o assistente da VPN |
//! | [`OperatingSystem::aliases`] | a dedução por `sysDescr` do SNMP |
//!
//! # O `id` é o sistema, não o fabricante
//!
//! Por isso `routeros` e não `mikrotik`: RouterOS é o sistema, MikroTik é quem
//! fabrica o equipamento — e o mesmo fabricante vende aparelho com SwOS. O
//! assistente da VPN continua falando `mikrotik` porque é o nome do gerador de
//! configuração registrado lá; a tradução vive no [`OperatingSystem::vpn_profile`]
//! desta tabela, e um teste garante que os dois lados não se percam.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Um sistema do catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatingSystem {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    /// Tem receita de syslog — e nesse caso o `id` **é** a chave dela.
    pub syslog: bool,
    /// Atende MAC-Telnet (RouterOS de fábrica; OpenWrt com `mactelnetd`).
    pub mac_telnet: bool,
    /// Perfil equivalente no assistente da VPN, quando existe.
    pub vpn_profile: Option<&'static str>,
    /// Palavras que identificam este sistema num texto livre — `sysDescr` do
    /// SNMP, campo de fabricante ou modelo do cadastro.
    pub aliases: &'static [&'static str],
}

/// Valor que a API aceita para "não declarei — deduza".
pub const AUTO: &str = "auto";

/// O sistema assumido quando nada identifica o equipamento.
///
/// Não é `other`: `other` significa "declarei que é outro", e aí não há receita
/// nenhuma a oferecer. Aqui o que houve foi ausência de evidência, e o parque
/// para o qual este sistema foi feito é RouterOS. A tela mostra a origem
/// `padrão` justamente para a escolha ser conferida antes de aplicar.
pub const FALLBACK: &str = "routeros";

/// A ordem importa duas vezes: é a ordem em que as telas listam, e é a ordem em
/// que os apelidos são procurados. `OpenWrt 23.05 Linux` precisa casar com
/// `openwrt` antes de chegar em `linux` — que é o que aconteceria se a ordem
/// fosse alfabética.
const CATALOGO: &[OperatingSystem] = &[
    OperatingSystem {
        id: "routeros",
        label: "MikroTik RouterOS",
        icon: "mdi-router-network",
        syslog: true,
        mac_telnet: true,
        vpn_profile: Some("mikrotik"),
        aliases: &["mikrotik", "routeros", "routerboard"],
    },
    OperatingSystem {
        id: "openwrt",
        label: "OpenWrt",
        icon: "mdi-router-wireless",
        syslog: true,
        mac_telnet: true,
        vpn_profile: Some("openwrt"),
        aliases: &["openwrt", "lede", "dd-wrt"],
    },
    OperatingSystem {
        id: "ubiquiti",
        label: "Ubiquiti EdgeOS / UniFi",
        icon: "mdi-router",
        syslog: true,
        mac_telnet: false,
        vpn_profile: None,
        aliases: &[
            "ubiquiti",
            "edgeos",
            "edgerouter",
            "edgeswitch",
            "unifi",
            "vyatta",
        ],
    },
    OperatingSystem {
        id: "linux",
        label: "Linux",
        icon: "mdi-linux",
        syslog: true,
        mac_telnet: false,
        vpn_profile: Some("linux"),
        aliases: &["debian", "ubuntu", "rsyslog", "linux"],
    },
    OperatingSystem {
        id: "windows",
        label: "Windows",
        icon: "mdi-microsoft-windows",
        syslog: false,
        mac_telnet: false,
        vpn_profile: Some("windows"),
        aliases: &["windows", "microsoft"],
    },
    OperatingSystem {
        id: "mobile",
        label: "Celular (Android / iOS)",
        icon: "mdi-cellphone",
        syslog: false,
        mac_telnet: false,
        vpn_profile: Some("mobile"),
        aliases: &["android", "iphone", "ipados", "ios"],
    },
    OperatingSystem {
        id: "other",
        label: "Outro sistema",
        icon: "mdi-help-circle-outline",
        syslog: false,
        mac_telnet: false,
        vpn_profile: None,
        // Nenhum: `other` é escolha declarada, nunca dedução. Deduzir "outro"
        // seria o mesmo que não deduzir, com a aparência de conclusão.
        aliases: &[],
    },
];

#[must_use]
pub fn catalog() -> &'static [OperatingSystem] {
    CATALOGO
}

#[must_use]
pub fn find(id: &str) -> Option<&'static OperatingSystem> {
    let procurado = id.trim();
    CATALOGO
        .iter()
        .find(|sistema| sistema.id.eq_ignore_ascii_case(procurado))
}

/// Lê o valor vindo da API. `auto` e vazio significam "sem declaração".
///
/// # Errors
///
/// Valor fora do catálogo — a mensagem lista o que é aceito, porque um "valor
/// inválido" seco obriga a caçar a tabela no código.
pub fn parse(bruto: &str) -> Result<Option<&'static OperatingSystem>, String> {
    let limpo = bruto.trim();
    if limpo.is_empty() || limpo.eq_ignore_ascii_case(AUTO) {
        return Ok(None);
    }
    find(limpo).map(Some).ok_or_else(|| {
        let aceitos = CATALOGO
            .iter()
            .map(|sistema| sistema.id)
            .collect::<Vec<_>>()
            .join("`, `");
        format!("Sistema desconhecido: `{limpo}`. Use `{AUTO}`, `{aceitos}`.")
    })
}

/// De onde saiu a conclusão. A tela mostra: dedução apresentada como certeza é
/// o começo de um erro que ninguém revisa.
pub mod source {
    /// O operador escolheu no cadastro.
    pub const DECLARED: &str = "declarado";
    /// O `sysDescr` do SNMP identificou.
    pub const SNMP: &str = "snmp";
    /// Deduzido do fabricante/modelo em texto livre do cadastro.
    pub const REGISTRY: &str = "cadastro";
    /// Nada identificou — ver [`super::FALLBACK`].
    pub const DEFAULT: &str = "padrão";
}

/// Qual sistema atribuir a um equipamento, e com que confiança.
///
/// A ordem é declaração → SNMP → cadastro → padrão. A declaração vem primeiro
/// pelo mesmo motivo de [`super::access`]: quem declarou sabe de algo que o
/// servidor não observa. Entre as duas evidências, o `sysDescr` ganha do texto
/// livre porque o `vendor` do cadastro costuma vir do OUI do MAC — que
/// identifica o fabricante da placa, não o sistema que roda nela.
#[must_use]
pub fn deduce(
    declarado: Option<&str>,
    vendor: Option<&str>,
    descricao: Option<&str>,
) -> (&'static OperatingSystem, &'static str) {
    if let Some(sistema) = declarado.and_then(|bruto| parse(bruto).ok().flatten()) {
        return (sistema, source::DECLARED);
    }
    if let Some(sistema) = casa(descricao) {
        return (sistema, source::SNMP);
    }
    if let Some(sistema) = casa(vendor) {
        return (sistema, source::REGISTRY);
    }
    (
        find(FALLBACK).expect("o padrão precisa estar no catálogo"),
        source::DEFAULT,
    )
}

fn casa(texto: Option<&str>) -> Option<&'static OperatingSystem> {
    let bruto = texto?.to_ascii_lowercase();
    if bruto.trim().is_empty() {
        return None;
    }
    CATALOGO
        .iter()
        .find(|sistema| sistema.aliases.iter().any(|agulha| bruto.contains(agulha)))
}

/// O que a tela recebe para montar o seletor.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct OperatingSystemOption {
    pub id: String,
    pub label: String,
    pub icon: String,
    /// Se a ativação automática de log tem comandos para este sistema. Sem
    /// isto a tela ofereceria uma ação que não tem como funcionar.
    pub supports_syslog: bool,
    pub supports_mac_telnet: bool,
    /// Perfil equivalente no assistente da VPN, quando existe.
    pub vpn_profile: Option<String>,
}

#[must_use]
pub fn options() -> Vec<OperatingSystemOption> {
    CATALOGO
        .iter()
        .map(|sistema| OperatingSystemOption {
            id: sistema.id.to_owned(),
            label: sistema.label.to_owned(),
            icon: sistema.icon.to_owned(),
            supports_syslog: sistema.syslog,
            supports_mac_telnet: sistema.mac_telnet,
            vpn_profile: sistema.vpn_profile.map(str::to_owned),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_catalogo_cobre_as_receitas_de_syslog_e_os_perfis_da_vpn() {
        // A prova de que a unificação é real e não um terceiro vocabulário
        // paralelo: toda receita e todo perfil precisa ter dono aqui.
        use crate::services::{syslog::snippets, vpn::profiles::registry};

        for receita in snippets::systems() {
            let sistema = find(receita).unwrap_or_else(|| panic!("receita {receita} sem sistema"));
            assert!(
                sistema.syslog,
                "{receita} tem receita mas o catálogo diz que não"
            );
        }
        for sistema in catalog().iter().filter(|sistema| sistema.syslog) {
            assert!(
                snippets::systems().contains(&sistema.id),
                "{} promete receita de syslog que não existe",
                sistema.id
            );
        }

        for card in registry::list() {
            assert!(
                catalog()
                    .iter()
                    .any(|sistema| sistema.vpn_profile == Some(card.profile.as_str())),
                "o perfil de VPN {} não tem sistema no catálogo",
                card.profile
            );
        }
        for sistema in catalog() {
            if let Some(perfil) = sistema.vpn_profile {
                assert!(
                    registry::has(perfil),
                    "{} aponta para o perfil {perfil}, que não existe",
                    sistema.id
                );
            }
        }
    }

    #[test]
    fn o_snmp_tem_precedencia_sobre_o_texto_livre_do_cadastro() {
        // O `vendor` do cadastro costuma vir do OUI do MAC, que identifica o
        // fabricante da placa — não o sistema que roda nela.
        let (sistema, origem) = deduce(None, Some("Routerboard"), Some("OpenWrt 23.05 Linux"));
        assert_eq!((sistema.id, origem), ("openwrt", source::SNMP));
    }

    #[test]
    fn a_declaracao_vence_ate_o_snmp() {
        let (sistema, origem) = deduce(Some("linux"), Some("MikroTik"), Some("RouterOS 7.14"));
        assert_eq!((sistema.id, origem), ("linux", source::DECLARED));
    }

    #[test]
    fn declaracao_ilegivel_nao_derruba_a_deducao() {
        let (sistema, origem) = deduce(Some("qualquer-coisa"), Some("MikroTik"), None);
        assert_eq!((sistema.id, origem), ("routeros", source::REGISTRY));
    }

    #[test]
    fn sem_nada_o_padrao_e_declarado_como_padrao() {
        let (sistema, origem) = deduce(None, None, None);
        assert_eq!((sistema.id, origem), (FALLBACK, source::DEFAULT));
        let (_, origem) = deduce(Some("   "), Some("   "), Some(""));
        assert_eq!(origem, source::DEFAULT);
    }

    #[test]
    fn a_ordem_dos_apelidos_impede_que_linux_engula_o_openwrt() {
        // Ordem alfabética quebraria isto em silêncio: quase toda descrição de
        // firmware embarcado contém a palavra "Linux".
        for (descricao, esperado) in [
            ("RouterOS CCR2004", "routeros"),
            ("OpenWrt 22.03.5 Linux 5.10", "openwrt"),
            ("EdgeOS v2.0.9", "ubiquiti"),
            ("UniFi AP-AC-Pro", "ubiquiti"),
            ("Linux servidor 6.1.0-18-amd64", "linux"),
            ("Microsoft Windows Server 2022", "windows"),
        ] {
            let (sistema, _) = deduce(None, None, Some(descricao));
            assert_eq!(sistema.id, esperado, "não reconheceu {descricao:?}");
        }
    }

    #[test]
    fn outro_e_escolha_declarada_e_nunca_deducao() {
        // Deduzir "outro" seria o mesmo que não deduzir, com a aparência de
        // conclusão — e ainda deixaria a ativação de log sem receita.
        let outro = find("other").expect("no catálogo");
        assert!(outro.aliases.is_empty());
        assert!(!outro.syslog);
        assert_eq!(parse("other"), Ok(Some(outro)));
    }

    #[test]
    fn o_vocabulario_aceito_e_o_que_a_mensagem_de_erro_promete() {
        assert_eq!(parse("auto"), Ok(None));
        assert_eq!(parse("   "), Ok(None));
        assert_eq!(
            parse("RouterOS").map(|s| s.map(|s| s.id)),
            Ok(Some("routeros"))
        );
        let erro = parse("cisco").expect_err("devia recusar");
        for aceito in catalog().iter().map(|sistema| sistema.id).chain([AUTO]) {
            assert!(
                erro.contains(aceito),
                "a mensagem não cita {aceito}: {erro}"
            );
        }
    }

    #[test]
    fn so_o_routeros_e_o_openwrt_atendem_mac_telnet() {
        // A tela oferece o meio de acesso a partir daqui; oferecer fora desta
        // dupla seria oferecer uma tentativa que não tem como dar certo.
        let com: Vec<&str> = catalog()
            .iter()
            .filter(|sistema| sistema.mac_telnet)
            .map(|sistema| sistema.id)
            .collect();
        assert_eq!(com, vec!["routeros", "openwrt"]);
    }
}
