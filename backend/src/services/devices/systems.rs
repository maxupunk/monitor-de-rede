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
//! | [`super::adapters::DeviceAdapter::syslog`] | a ativação de log |
//! | [`super::adapters::DeviceAdapter::supports_access`] | o seletor de meio de acesso |
//! | [`super::adapters::DeviceAdapter::vpn_profile`] | o assistente da VPN |
//! | [`super::adapters::DeviceAdapter::aliases`] | a dedução por `sysDescr` do SNMP |
//!
//! # O `id` é o sistema, não o fabricante
//!
//! Por isso `routeros` e não `mikrotik`: RouterOS é o sistema, MikroTik é quem
//! fabrica o equipamento — e o mesmo fabricante vende aparelho com SwOS. O
//! assistente da VPN continua falando `mikrotik` porque é o nome do gerador de
//! configuração registrado lá; a tradução vive no adapter da plataforma, e um
//! teste garante que os dois lados não se percam.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::adapters::{registry, DeviceAccessMethod, DevicePlatform};

/// Um sistema do catálogo.
pub type OperatingSystem = DevicePlatform;

/// Valor que a API aceita para "não declarei — deduza".
pub const AUTO: &str = "auto";

/// Sem evidência, usa o adapter neutro, sem comandos específicos de plataforma.
/// A origem `padrão` distingue ausência de identificação de declaração manual.
pub const FALLBACK: &str = "other";

#[must_use]
pub fn catalog() -> &'static [&'static OperatingSystem] {
    registry::platforms()
}

#[must_use]
pub fn find(id: &str) -> Option<&'static OperatingSystem> {
    registry::find(id).map(|adapter| adapter.platform())
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
        let aceitos = catalog()
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
    /// O SNMP identificou — `sysObjectId` ou `sysDescr`.
    pub const SNMP: &str = "snmp";
    /// A identificação que o servidor SSH do equipamento anuncia.
    pub const PROBE: &str = "sonda";
    /// Deduzido do fabricante/modelo em texto livre do cadastro.
    pub const REGISTRY: &str = "cadastro";
    /// Nada identificou — ver [`super::FALLBACK`].
    pub const DEFAULT: &str = "padrão";
}

/// Tudo que se tem sobre um equipamento na hora de decidir.
///
/// Struct e não uma lista de parâmetros porque cada chamador tem um subconjunto
/// diferente: o cadastro só tem texto livre, a tela de log tem SNMP e sonda de
/// porta. Com `Default`, cada um preenche o que sabe e o resto fica ausente em
/// vez de virar `None` posicional que ninguém consegue ler na chamada.
#[derive(Debug, Clone, Copy, Default)]
pub struct Evidence<'a> {
    pub declared: Option<&'a str>,
    pub sys_object_id: Option<&'a str>,
    pub sys_descr: Option<&'a str>,
    /// A linha de identificação do servidor SSH (`SSH-2.0-dropbear_2022.82`).
    pub ssh_banner: Option<&'a str>,
    pub name: Option<&'a str>,
    pub vendor: Option<&'a str>,
    pub model: Option<&'a str>,
}

/// A conclusão, com o porquê.
#[derive(Debug, Clone)]
pub struct Detection {
    pub system: &'static OperatingSystem,
    pub source: &'static str,
    /// A frase que a tela mostra. É ela que permite conferir a conclusão sem
    /// abrir o código — e foi a falta dela que deixou um OpenWrt passar por
    /// Linux sem ninguém entender de onde vinha.
    pub reason: String,
}

/// Qual sistema atribuir a um equipamento, e com que confiança.
///
/// A ordem das evidências, da mais forte para a mais fraca:
///
/// 1. **a declaração do operador** — quem declarou sabe de algo que o servidor
///    não observa;
/// 2. **o `sysObjectId`** — número de empresa da IANA, registro e não prosa;
/// 3. **um apelido específico no `sysDescr`** — "OpenWrt", "RouterOS", "EdgeOS";
/// 4. **a identificação do servidor SSH** — `dropbear` é OpenWrt, `ROSSSH` é
///    RouterOS. É o que resolve o firmware embarcado cujo agente SNMP responde
///    só o `uname`;
/// 5. **o apelido genérico no `sysDescr`** — a palavra "Linux", que quase todo
///    firmware embarcado diz e que por isso só decide quando nada mais decidiu;
/// 6. o fabricante/modelo do cadastro, específico antes de genérico;
/// 7. o padrão.
///
/// O salto que importa é o 4 vir **antes** do 5. Um OpenWrt cujo `sysDescr` é
/// `Linux bpi-r3 6.12.87 aarch64` casava com "linux" no passo 3 e parava ali.
#[must_use]
pub fn detect(evidencia: &Evidence) -> Detection {
    if let Some(sistema) = evidencia
        .declared
        .and_then(|bruto| parse(bruto).ok().flatten())
    {
        return achado(
            sistema,
            source::DECLARED,
            format!("definido no cadastro como \"{}\"", sistema.label),
        );
    }

    if let Some((sistema, oid)) = casa_oid(evidencia.sys_object_id) {
        return achado(
            sistema,
            source::SNMP,
            format!(
                "o sysObjectId do equipamento começa com {oid}, que é de {}",
                sistema.label
            ),
        );
    }

    if let Some(sistema) = casa(evidencia.sys_descr, Especificidade::Especifico) {
        return achado(
            sistema,
            source::SNMP,
            format!("o sysDescr do SNMP identifica {}", sistema.label),
        );
    }

    if let Some((sistema, marca)) = casa_banner(evidencia.ssh_banner) {
        return achado(
            sistema,
            source::PROBE,
            format!(
                "o servidor SSH se identifica como `{marca}`, que é o padrão do {}",
                sistema.label
            ),
        );
    }

    if let Some(sistema) = casa(evidencia.sys_descr, Especificidade::Generico) {
        return achado(
            sistema,
            source::SNMP,
            format!(
                "o sysDescr diz apenas {} — nenhum sistema mais específico se identificou",
                sistema.label
            ),
        );
    }

    for nivel in [Especificidade::Especifico, Especificidade::Generico] {
        for texto in [evidencia.vendor, evidencia.model, evidencia.name] {
            if let Some(sistema) = casa(texto, nivel) {
                return achado(
                    sistema,
                    source::REGISTRY,
                    format!(
                        "deduzido de \"{}\" no cadastro",
                        texto.unwrap_or_default().trim()
                    ),
                );
            }
        }
    }

    achado(
        find(FALLBACK).expect("o padrão precisa estar no catálogo"),
        source::DEFAULT,
        "sistema não identificado — as evidências não permitem determinar o firmware; selecione manualmente se souber".to_owned(),
    )
}

fn achado(system: &'static OperatingSystem, source: &'static str, reason: String) -> Detection {
    Detection {
        system,
        source,
        reason,
    }
}

/// Se o apelido descreve um sistema ou uma família inteira. Ver
/// [`DevicePlatform::generic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Especificidade {
    Especifico,
    Generico,
}

fn casa(texto: Option<&str>, nivel: Especificidade) -> Option<&'static OperatingSystem> {
    let bruto = texto?.to_ascii_lowercase();
    if bruto.trim().is_empty() {
        return None;
    }
    registry::all()
        .iter()
        .copied()
        .filter(|adapter| adapter.platform().generic == (nivel == Especificidade::Generico))
        .find(|adapter| {
            adapter
                .aliases()
                .iter()
                .any(|agulha| bruto.contains(agulha))
        })
        .map(|adapter| adapter.platform())
}

/// Casa pelo prefixo, e não por igualdade: o `sysObjectId` completo carrega a
/// linha de produto depois do número da empresa (`…14988.1.1.3.11`).
fn casa_oid(oid: Option<&str>) -> Option<(&'static OperatingSystem, &'static str)> {
    let bruto = oid?.trim().trim_start_matches('.');
    if bruto.is_empty() {
        return None;
    }
    registry::all().iter().find_map(|adapter| {
        adapter
            .sys_object_ids()
            .iter()
            .find(|prefixo| {
                bruto == **prefixo
                    || bruto
                        .strip_prefix(**prefixo)
                        .is_some_and(|resto| resto.starts_with('.'))
            })
            .map(|prefixo| (adapter.platform(), *prefixo))
    })
}

fn casa_banner(banner: Option<&str>) -> Option<(&'static OperatingSystem, &'static str)> {
    let bruto = banner?.to_ascii_lowercase();
    if bruto.trim().is_empty() {
        return None;
    }
    registry::all().iter().find_map(|adapter| {
        adapter
            .ssh_banners()
            .iter()
            .find(|marca| bruto.contains(*marca))
            .map(|marca| (adapter.platform(), *marca))
    })
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

/// O resultado de `POST /api/devices/identify`.
///
/// Carrega **a evidência junto com a conclusão** de propósito. Foi a falta disso
/// que deixou um OpenWrt passar por Linux sem ninguém entender por quê: o campo
/// dizia "Linux" e não havia onde conferir que a razão era um `sysDescr` com o
/// `uname` genérico.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct IdentifyResult {
    pub operating_system: String,
    pub label: String,
    /// `declarado`, `snmp`, `sonda`, `cadastro` ou `padrão`.
    pub source: String,
    pub reason: String,
    /// O que o SNMP respondeu, para a tela poder mostrar o texto cru.
    pub sys_descr: Option<String>,
    pub sys_object_id: Option<String>,
    /// A identificação do servidor SSH, quando a porta 22 respondeu.
    pub ssh_banner: Option<String>,
    /// Dados de inventário que o formulário pode preencher sem adivinhar.
    /// Nulos quando a sonda/descoberta não trouxe evidência suficiente.
    pub suggested_vendor: Option<String>,
    pub suggested_model: Option<String>,
    /// Nome anunciado pelo próprio equipamento, preferencialmente via
    /// `sysName`; a tela só o aplica enquanto o operador não tiver digitado.
    pub suggested_name: Option<String>,
    /// Forma de acesso deduzida pela mesma regra usada depois do cadastro.
    pub access_mode: String,
    pub access_mode_reason: String,
    /// Se alguma evidência ao vivo chegou. Falso significa que a conclusão saiu
    /// de cache ou só do cadastro.
    pub probed: bool,
    /// A sonda atual não respondeu e a evidência SNMP veio da última descoberta.
    pub from_discovery: bool,
}

/// Escolhe e normaliza um nome anunciado pelo equipamento.
///
/// A ordem expressa a confiança: `sysName` veio de uma consulta feita agora;
/// hostname e mDNS podem ser cache de uma descoberta anterior. Endereços IP e
/// textos com controles não são nomes úteis para o inventário.
#[must_use]
pub fn suggest_name(
    sys_name: Option<&str>,
    discovered_hostname: Option<&str>,
    mdns_name: Option<&str>,
) -> Option<String> {
    [sys_name, discovered_hostname, mdns_name]
        .into_iter()
        .flatten()
        .find_map(|candidate| {
            let normalized = candidate.trim().trim_end_matches('.').trim();
            (!normalized.is_empty()
                && normalized.len() <= 120
                && normalized.parse::<std::net::IpAddr>().is_err()
                && !normalized.chars().any(char::is_control))
            .then(|| normalized.to_owned())
        })
}

#[must_use]
pub fn options() -> Vec<OperatingSystemOption> {
    registry::all()
        .iter()
        .map(|adapter| OperatingSystemOption {
            id: adapter.platform().id.to_owned(),
            label: adapter.platform().label.to_owned(),
            icon: adapter.platform().icon.to_owned(),
            supports_syslog: adapter.syslog().is_some(),
            supports_mac_telnet: adapter.supports_access(DeviceAccessMethod::MacTelnet),
            vpn_profile: adapter.vpn_profile().map(str::to_owned),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_nome_prefere_sysname_e_descarta_ip_ou_controle() {
        assert_eq!(
            suggest_name(Some("  roteador-borda  "), Some("descoberta.local"), None).as_deref(),
            Some("roteador-borda")
        );
        assert_eq!(
            suggest_name(Some("10.0.0.1"), Some("openwrt.local."), None).as_deref(),
            Some("openwrt.local")
        );
        assert!(suggest_name(Some("nome\nmalicioso"), None, None).is_none());
    }

    #[test]
    fn o_catalogo_cobre_as_receitas_de_syslog_e_os_perfis_da_vpn() {
        use crate::services::{syslog::snippets, vpn::profiles::registry};

        for receita in snippets::systems() {
            let adapter = super::registry::find(receita)
                .unwrap_or_else(|| panic!("receita {receita} sem adapter"));
            assert!(
                adapter.syslog().is_some(),
                "{receita} tem receita mas o adapter diz que não"
            );
        }
        for adapter in super::registry::with_syslog() {
            assert!(
                snippets::systems().contains(&adapter.platform().id),
                "{} promete receita de syslog que não existe",
                adapter.platform().id
            );
        }

        for card in registry::list() {
            assert!(
                super::registry::by_vpn_profile(&card.profile).is_some(),
                "o perfil de VPN {} não tem sistema no catálogo",
                card.profile
            );
        }
        for adapter in super::registry::all() {
            if let Some(perfil) = adapter.vpn_profile() {
                assert!(
                    registry::has(perfil),
                    "{} aponta para o perfil {perfil}, que não existe",
                    adapter.platform().id
                );
            }
        }
    }

    fn por_descricao(descricao: &str) -> Detection {
        detect(&Evidence {
            sys_descr: Some(descricao),
            ..Evidence::default()
        })
    }

    #[test]
    fn o_snmp_tem_precedencia_sobre_o_texto_livre_do_cadastro() {
        // O `vendor` do cadastro costuma vir do OUI do MAC, que identifica o
        // fabricante da placa — não o sistema que roda nela.
        let achado = detect(&Evidence {
            vendor: Some("Routerboard"),
            sys_descr: Some("OpenWrt 23.05 Linux"),
            ..Evidence::default()
        });
        assert_eq!((achado.system.id, achado.source), ("openwrt", source::SNMP));
    }

    #[test]
    fn a_declaracao_vence_ate_o_snmp() {
        let achado = detect(&Evidence {
            declared: Some("linux"),
            vendor: Some("MikroTik"),
            sys_descr: Some("RouterOS 7.14"),
            ..Evidence::default()
        });
        assert_eq!(
            (achado.system.id, achado.source),
            ("linux", source::DECLARED)
        );
    }

    #[test]
    fn declaracao_ilegivel_nao_derruba_a_deducao() {
        let achado = detect(&Evidence {
            declared: Some("qualquer-coisa"),
            vendor: Some("MikroTik"),
            ..Evidence::default()
        });
        assert_eq!(
            (achado.system.id, achado.source),
            ("routeros", source::REGISTRY)
        );
    }

    #[test]
    fn sem_nada_o_padrao_e_declarado_como_padrao() {
        let achado = detect(&Evidence::default());
        assert_eq!(
            (achado.system.id, achado.source),
            (FALLBACK, source::DEFAULT)
        );
        let achado = detect(&Evidence {
            declared: Some("   "),
            vendor: Some("   "),
            sys_descr: Some(""),
            ..Evidence::default()
        });
        assert_eq!(achado.source, source::DEFAULT);
    }

    #[test]
    fn o_banner_do_ssh_separa_o_openwrt_do_linux_generico() {
        // O caso real que motivou tudo isto: o agente SNMP de um OpenWrt
        // responde só o `uname`, e a palavra "Linux" bastava para encerrar a
        // dedução no sistema errado. O `dropbear` chega antes de qualquer
        // autenticação, no mesmo `connect` que já sondava a porta 22.
        let uname = "Linux bpi-r3-assistencia 6.12.87 #0 SMP Wed May 13 22:42:09 2026 aarch64";
        assert_eq!(por_descricao(uname).system.id, "linux");

        let achado = detect(&Evidence {
            sys_descr: Some(uname),
            ssh_banner: Some("SSH-2.0-dropbear_2022.82"),
            ..Evidence::default()
        });
        assert_eq!(
            (achado.system.id, achado.source),
            ("openwrt", source::PROBE)
        );
        assert!(achado.reason.contains("dropbear"), "{}", achado.reason);
    }

    #[test]
    fn a_descricao_especifica_vence_o_banner() {
        // Um Debian rodando dropbear existe. Quando o `sysDescr` nomeia o
        // sistema, ele é evidência mais direta do que o servidor SSH escolhido.
        let achado = detect(&Evidence {
            sys_descr: Some("RouterOS 7.14 on CCR2004"),
            ssh_banner: Some("SSH-2.0-dropbear_2022.82"),
            ..Evidence::default()
        });
        assert_eq!(
            (achado.system.id, achado.source),
            ("routeros", source::SNMP)
        );
    }

    #[test]
    fn o_sys_object_id_vence_qualquer_texto() {
        // Número de empresa da IANA é registro, não prosa de firmware.
        let achado = detect(&Evidence {
            sys_object_id: Some("1.3.6.1.4.1.14988.1.1.3.11"),
            sys_descr: Some("Linux router 5.6.3"),
            ..Evidence::default()
        });
        assert_eq!(
            (achado.system.id, achado.source),
            ("routeros", source::SNMP)
        );
        assert!(achado.reason.contains("14988"), "{}", achado.reason);
    }

    #[test]
    fn o_oid_do_net_snmp_nao_identifica_sistema_nenhum() {
        // `1.3.6.1.4.1.8072` é o agente, não o sistema — e ele roda tanto num
        // OpenWrt quanto num Debian. Mapeá-lo para `linux` reintroduziria o
        // mesmo erro por outro caminho.
        let achado = detect(&Evidence {
            sys_object_id: Some("1.3.6.1.4.1.8072.3.2.10"),
            ssh_banner: Some("SSH-2.0-dropbear_2022.82"),
            ..Evidence::default()
        });
        assert_eq!(
            (achado.system.id, achado.source),
            ("openwrt", source::PROBE)
        );
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
            assert_eq!(
                por_descricao(descricao).system.id,
                esperado,
                "não reconheceu {descricao:?}"
            );
        }
    }

    #[test]
    fn so_o_linux_e_generico() {
        // O genérico decide por último. Marcar outro sistema assim faria ele
        // atropelar evidência mais específica sem ninguém perceber.
        let genericos: Vec<&str> = catalog()
            .iter()
            .filter(|sistema| sistema.generic)
            .map(|sistema| sistema.id)
            .collect();
        assert_eq!(genericos, vec!["linux"]);
    }

    #[test]
    fn toda_conclusao_vem_com_um_motivo_legivel() {
        // Sem o motivo, "está como Linux" não tem como ser conferido — que foi
        // exatamente o que aconteceu com o OpenWrt identificado errado.
        for evidencia in [
            Evidence::default(),
            Evidence {
                declared: Some("openwrt"),
                ..Evidence::default()
            },
            Evidence {
                sys_descr: Some("Linux x 6.1"),
                ..Evidence::default()
            },
            Evidence {
                vendor: Some("MikroTik"),
                ..Evidence::default()
            },
        ] {
            let achado = detect(&evidencia);
            assert!(
                achado.reason.trim().len() > 10,
                "motivo pobre: {:?}",
                achado.reason
            );
        }
    }

    #[test]
    fn outro_nao_oferece_comandos_especificos() {
        let outro = find("other").expect("no catálogo");
        let adapter = super::registry::find("other").expect("adapter");
        assert!(adapter.aliases().is_empty());
        assert!(adapter.syslog().is_none());
        assert_eq!(parse("other"), Ok(Some(outro)));
    }

    #[test]
    fn controlador_mppt_e_embarcado_e_nao_routeros() {
        let achado = detect(&Evidence {
            name: Some("Volt"),
            sys_descr: Some("Controlador de Carga MPPT 12V/24V/48V-30A"),
            sys_object_id: Some("1.3.6.1.4.1.17095.1"),
            ..Evidence::default()
        });
        assert_eq!(
            (achado.system.id, achado.source),
            ("embedded", source::SNMP)
        );
        assert!(registry::find(achado.system.id).unwrap().syslog().is_none());
    }

    #[test]
    fn prefixo_oid_respeita_limites_dos_componentes() {
        assert!(casa_oid(Some("1.3.6.1.4.1.149880.1")).is_none());
        assert_eq!(
            casa_oid(Some(".1.3.6.1.4.1.14988.1")).unwrap().0.id,
            "routeros"
        );
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
            .filter(|sistema| {
                super::registry::find(sistema.id)
                    .is_some_and(|adapter| adapter.supports_access(DeviceAccessMethod::MacTelnet))
            })
            .map(|sistema| sistema.id)
            .collect();
        assert_eq!(com, vec!["routeros", "openwrt"]);
    }
}
