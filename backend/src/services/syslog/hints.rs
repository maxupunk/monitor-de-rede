//! O que o servidor consegue descobrir sozinho antes de pedir dados ao usuário.
//!
//! Existe porque três campos da tela de ativação eram chute do operador, e os
//! três falham em silêncio quando o chute erra:
//!
//! - **o endereço deste servidor**, que vai para dentro do roteador. A tela
//!   mandava o host da barra de endereços; quem abre a interface em
//!   `http://localhost:3333` gravava `remote=localhost` no equipamento, e o
//!   roteador então manda o syslog para si mesmo. Não há erro, não há aviso, e
//!   nada chega — o pior desfecho possível;
//! - **o meio de acesso**, que o operador escolhia por tentativa. Sondar a
//!   porta antes custa um `connect` e evita esperar o timeout de um SSH que
//!   está desligado;
//! - **o fabricante**, que decide quais comandos serão enviados. O cadastro tem
//!   um campo de texto livre, e o SNMP costuma saber melhor.
//!
//! Nada aqui é decisão final: tudo volta para a tela como sugestão editável. O
//! módulo responde "o que dá para saber", não "o que vai ser feito".

use std::{
    net::{IpAddr, SocketAddr, UdpSocket},
    time::Duration,
};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use tokio::{net::TcpStream, time::timeout};

use super::nat::NatDetector;
use crate::{
    models::_entities::{device_interfaces, devices, discovery_results},
    services::{
        shared::errors::AppResult,
        snmp::{
            client::{SnmpClient, SnmpConfig},
            collectors::collect_system,
        },
    },
};

/// Quanto se espera por uma porta antes de considerá-la fechada.
///
/// Curto de propósito: são duas sondas em série na abertura de um diálogo, e o
/// alvo está na rede local. Porta filtrada por firewall gasta o teto inteiro,
/// e é por isso que ele não pode ser generoso.
const TETO_DA_SONDA: Duration = Duration::from_millis(900);

/// O que a tela recebe para se preencher sozinha.
#[derive(Debug, Clone, Default)]
pub struct ProvisionHints {
    /// Endereço deste servidor como o **equipamento** o alcançaria, ou `None`
    /// quando não há resposta confiável.
    pub server_address: Option<String>,
    /// De onde veio o palpite acima — a tela usa para explicar o campo.
    pub server_address_source: &'static str,
    pub ssh_open: bool,
    pub telnet_open: bool,
    /// Chave de receita deduzida do cadastro e do SNMP.
    pub vendor: String,
    /// De onde veio a dedução do fabricante.
    pub vendor_source: &'static str,
    pub mac_address: Option<String>,
    /// Se este processo consegue falar em camada 2 com a rede do equipamento.
    /// Falso dentro de um container em rede bridge — e é o que inviabiliza o
    /// MAC-Telnet ali.
    pub layer2_reachable: bool,
}

/// Junta tudo que se sabe sobre um dispositivo antes de abrir a tela.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn collect(
    db: &DatabaseConnection,
    dispositivo: &devices::Model,
    nat: &NatDetector,
) -> AppResult<ProvisionHints> {
    let host = dispositivo
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .and_then(|texto| texto.parse::<IpAddr>().ok());

    // A descrição do SNMP identifica o **sistema**; o `vendor` do cadastro
    // costuma vir do OUI do MAC, que identifica o fabricante da placa. Por isso
    // vale a consulta ao vivo quando o SNMP está ligado.
    let descricao = match host {
        Some(endereco) if dispositivo.snmp_enabled => sys_descr(endereco, dispositivo).await,
        _ => None,
    };
    let (vendor, vendor_source) = deduz_vendor(
        dispositivo.vendor.as_deref(),
        descricao.as_deref().or(dispositivo.model.as_deref()),
    );

    // As duas sondas em paralelo: em série elas somariam o teto de cada uma na
    // abertura do diálogo.
    let (ssh_open, telnet_open) = match host {
        Some(endereco) => tokio::join!(porta_aberta(endereco, 22), porta_aberta(endereco, 23)),
        None => (false, false),
    };

    let (server_address, server_address_source) = match host.and_then(local_address_toward) {
        Some(endereco) => (Some(endereco.to_string()), "rota até o equipamento"),
        None => (None, "desconhecido"),
    };

    Ok(ProvisionHints {
        server_address,
        server_address_source,
        ssh_open,
        telnet_open,
        vendor,
        vendor_source,
        mac_address: mac_conhecido(db, dispositivo).await?,
        // A difusão do MAC-Telnet não atravessa a ponte do Docker.
        layer2_reachable: !nat.bridged_container(),
    })
}

/// O MAC do equipamento, que o MAC-Telnet endereça.
///
/// **`devices` não tem coluna de MAC** — ele mora em `device_interfaces`
/// (coletado por SNMP) e em `discovery_results` (coletado por ARP). A ordem é
/// essa: o MAC de uma interface do próprio aparelho é dado dele, enquanto o do
/// ARP é o que respondeu por aquele IP, e num IP que trocou de dono responde o
/// aparelho errado.
pub async fn mac_conhecido(
    db: &DatabaseConnection,
    dispositivo: &devices::Model,
) -> AppResult<Option<String>> {
    let da_interface = device_interfaces::Entity::find()
        .filter(device_interfaces::Column::DeviceId.eq(dispositivo.id))
        .filter(device_interfaces::Column::MacAddress.is_not_null())
        // Pela ordem do `ifTable`: a primeira interface do equipamento é a que
        // tem mais chance de ser a de gerência, e é dela que o MAC serve.
        .order_by_asc(device_interfaces::Column::SnmpIndex)
        .one(db)
        .await?
        .and_then(|linha| linha.mac_address);
    if let Some(mac) = da_interface.filter(|valor| !valor.trim().is_empty()) {
        return Ok(Some(mac));
    }

    let Some(ip) = dispositivo.ip_address.as_deref() else {
        return Ok(None);
    };
    Ok(discovery_results::Entity::find()
        .filter(discovery_results::Column::IpAddress.eq(ip))
        .filter(discovery_results::Column::MacAddress.is_not_null())
        .one(db)
        .await?
        .and_then(|linha| linha.mac_address)
        .filter(|valor| !valor.trim().is_empty()))
}

/// Lê o `sysDescr` do equipamento. Falha vira `None` — é um palpite, e um
/// agente SNMP fora do ar não pode impedir a tela de abrir.
async fn sys_descr(host: IpAddr, dispositivo: &devices::Model) -> Option<String> {
    let comunidade = dispositivo.snmp_community.clone().unwrap_or_default();
    if comunidade.trim().is_empty() {
        return None;
    }
    let mut config = SnmpConfig::v2c(host.to_string(), comunidade, 161);
    // Teto curto: isto roda na abertura de um diálogo, não num ciclo de coleta.
    config.timeout_ms = 1_500;
    let cliente = SnmpClient::new(config);
    collect_system(&cliente)
        .await
        .ok()
        .and_then(|sistema| sistema.sys_descr)
        .filter(|texto| !texto.trim().is_empty())
}

/// Descobre o endereço local que o sistema operacional usaria para falar com
/// `destino`.
///
/// O truque é `connect` num socket **UDP**: ele não envia pacote nenhum, só faz
/// o kernel consultar a tabela de rotas e vincular o socket ao endereço de
/// saída. É a única forma portátil de responder "por qual IP eu apareço para
/// este destino" — enumerar interfaces daria a lista, não a escolha, e numa
/// máquina com VPN, bridge do Docker e duas placas a lista tem cinco respostas
/// erradas para cada certa.
///
/// **Isso responde pela rota, não pela alcançabilidade.** Dentro de um
/// container em rede bridge o endereço devolvido é o da bridge — correto do
/// ponto de vista do kernel e inútil para o roteador. Quem separa os dois casos
/// é [`enderecos_utilizaveis`].
#[must_use]
pub fn local_address_toward(destino: IpAddr) -> Option<IpAddr> {
    let vinculo: SocketAddr = if destino.is_ipv4() {
        "0.0.0.0:0".parse().ok()?
    } else {
        "[::]:0".parse().ok()?
    };
    let socket = UdpSocket::bind(vinculo).ok()?;
    // A porta é irrelevante: nada é transmitido. Só o `connect` importa.
    socket.connect(SocketAddr::new(destino, 9)).ok()?;
    let local = socket.local_addr().ok()?.ip();
    utilizavel(local).then_some(local)
}

/// Se um endereço serve para ser gravado num roteador.
///
/// `localhost` é o caso que motivou o módulo: é um endereço válido, aceito por
/// todo comando de configuração, e que faz o equipamento mandar o log para si
/// mesmo. Não-especificado (`0.0.0.0`) e link-local caem pelo mesmo critério —
/// são endereços que existem e não levam a lugar nenhum.
#[must_use]
pub fn utilizavel(endereco: IpAddr) -> bool {
    !(endereco.is_loopback() || endereco.is_unspecified() || link_local(endereco))
}

fn link_local(endereco: IpAddr) -> bool {
    match endereco {
        IpAddr::V4(v4) => v4.is_link_local(),
        // `is_unicast_link_local` ainda não é estável; a faixa é fe80::/10.
        IpAddr::V6(v6) => (v6.segments()[0] & 0xffc0) == 0xfe80,
    }
}

/// Filtra um endereço vindo da tela. Texto vazio ou inútil vira `None`, para o
/// chamador cair no palpite do servidor em vez de gravar lixo no equipamento.
#[must_use]
pub fn sanitiza_endereco(bruto: Option<&str>) -> Option<String> {
    let limpo = bruto.map(str::trim).filter(|valor| !valor.is_empty())?;
    // Um nome (não um IP) não pode ser validado aqui: `netmonitor.local` pode
    // muito bem ser resolvível pelo roteador. Só o que **é** endereço passa
    // pelo crivo — e `localhost` é o nome que todo mundo resolve para o lugar
    // errado, então ele sai pelo nome mesmo.
    if limpo.eq_ignore_ascii_case("localhost")
        || limpo.eq_ignore_ascii_case("localhost.localdomain")
    {
        return None;
    }
    if let Ok(endereco) = limpo.parse::<IpAddr>() {
        return utilizavel(endereco).then(|| limpo.to_owned());
    }
    Some(limpo.to_owned())
}

/// Sonda TCP de porta única. Conexão recusada e tempo esgotado dão o mesmo
/// resultado de propósito: dos dois lados a tela precisa dizer "não dá para
/// usar este meio", e distinguir não muda nada para quem lê.
pub async fn porta_aberta(host: IpAddr, porta: u16) -> bool {
    matches!(
        timeout(
            TETO_DA_SONDA,
            TcpStream::connect(SocketAddr::new(host, porta))
        )
        .await,
        Ok(Ok(_))
    )
}

/// Traduz o fabricante de texto livre para uma chave de receita.
///
/// O `vendor` de `devices` vem do OUI do MAC, do SNMP ou da digitação do
/// operador — e a descrição do sistema (`sysDescr`) costuma ser mais específica
/// que os três. Por isso ela entra primeiro quando existe.
#[must_use]
pub fn deduz_vendor(vendor: Option<&str>, descricao: Option<&str>) -> (String, &'static str) {
    if let Some(chave) = casa_vendor(descricao) {
        return (chave.to_owned(), "snmp");
    }
    if let Some(chave) = casa_vendor(vendor) {
        return (chave.to_owned(), "cadastro");
    }
    // O RouterOS é o parque para o qual este sistema foi feito, e a tela mostra
    // a escolha antes de enviar qualquer coisa.
    ("routeros".to_owned(), "padrão")
}

fn casa_vendor(texto: Option<&str>) -> Option<&'static str> {
    let bruto = texto?.to_ascii_lowercase();
    if bruto.is_empty() {
        return None;
    }
    for (agulha, chave) in [
        ("mikrotik", "routeros"),
        ("routeros", "routeros"),
        ("routerboard", "routeros"),
        ("openwrt", "openwrt"),
        ("lede", "openwrt"),
        ("dd-wrt", "openwrt"),
        ("ubiquiti", "ubiquiti"),
        ("edgeos", "ubiquiti"),
        ("edgerouter", "ubiquiti"),
        ("edgeswitch", "ubiquiti"),
        ("unifi", "ubiquiti"),
        ("vyatta", "ubiquiti"),
        ("debian", "linux"),
        ("ubuntu", "linux"),
        ("rsyslog", "linux"),
        ("linux", "linux"),
    ] {
        if bruto.contains(agulha) {
            return Some(chave);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(texto: &str) -> IpAddr {
        texto.parse().expect("ip do teste")
    }

    #[test]
    fn o_localhost_nunca_passa_como_endereco_do_servidor() {
        // O bug que motivou o módulo: gravado no roteador, faz o equipamento
        // mandar o log para si mesmo, sem erro e sem nada chegando.
        assert_eq!(sanitiza_endereco(Some("localhost")), None);
        assert_eq!(sanitiza_endereco(Some("LOCALHOST")), None);
        assert_eq!(sanitiza_endereco(Some(" 127.0.0.1 ")), None);
        assert_eq!(sanitiza_endereco(Some("::1")), None);
        assert_eq!(sanitiza_endereco(Some("0.0.0.0")), None);
    }

    #[test]
    fn endereco_de_verdade_passa_inteiro() {
        assert_eq!(
            sanitiza_endereco(Some(" 192.168.1.10 ")),
            Some("192.168.1.10".to_owned())
        );
        // Nome que não é `localhost` continua valendo: o roteador pode muito
        // bem resolvê-lo, e recusar aqui tiraria uma opção legítima.
        assert_eq!(
            sanitiza_endereco(Some("netmonitor.lan")),
            Some("netmonitor.lan".to_owned())
        );
    }

    #[test]
    fn campo_vazio_vira_ausencia_e_nao_string_vazia() {
        assert_eq!(sanitiza_endereco(None), None);
        assert_eq!(sanitiza_endereco(Some("")), None);
        assert_eq!(sanitiza_endereco(Some("   ")), None);
    }

    #[test]
    fn o_link_local_e_recusado_junto_com_o_loopback() {
        assert!(
            !utilizavel(ip("169.254.3.4")),
            "APIPA não leva a lugar nenhum"
        );
        assert!(!utilizavel(ip("fe80::1")));
        assert!(utilizavel(ip("192.168.1.10")));
        assert!(utilizavel(ip("10.0.0.1")));
    }

    #[test]
    fn a_rota_local_e_descoberta_sem_enviar_pacote() {
        // Endereço de documentação (RFC 5737): existe rota padrão para ele em
        // qualquer máquina, e nada é transmitido — o `connect` de UDP só
        // consulta a tabela de rotas.
        let local = local_address_toward(ip("203.0.113.1"));
        if let Some(endereco) = local {
            assert!(utilizavel(endereco), "devolveu {endereco}, que não serve");
        }
        // Loopback como destino resolve para loopback como origem, e o filtro
        // precisa recusar — senão o palpite volta a ser `127.0.0.1`.
        assert_eq!(local_address_toward(ip("127.0.0.1")), None);
    }

    #[test]
    fn o_snmp_tem_precedencia_sobre_o_cadastro() {
        // O `vendor` do cadastro costuma vir do OUI do MAC, que identifica o
        // fabricante da placa — não o sistema que roda nela.
        let (chave, origem) = deduz_vendor(Some("Routerboard"), Some("OpenWrt 23.05 Linux"));
        assert_eq!((chave.as_str(), origem), ("openwrt", "snmp"));
    }

    #[test]
    fn sem_snmp_o_cadastro_decide() {
        let (chave, origem) = deduz_vendor(Some("MikroTik"), None);
        assert_eq!((chave.as_str(), origem), ("routeros", "cadastro"));
    }

    #[test]
    fn sem_nada_o_padrao_e_declarado_como_padrao() {
        // A origem importa: a tela avisa que foi chute, em vez de apresentar um
        // palpite como se fosse informação.
        let (chave, origem) = deduz_vendor(None, None);
        assert_eq!((chave.as_str(), origem), ("routeros", "padrão"));
        let (_, origem) = deduz_vendor(Some("   "), Some(""));
        assert_eq!(origem, "padrão");
    }

    #[test]
    fn as_familias_conhecidas_sao_reconhecidas_pela_descricao_do_snmp() {
        for (descricao, esperado) in [
            ("RouterOS CCR2004", "routeros"),
            ("OpenWrt 22.03.5", "openwrt"),
            ("EdgeOS v2.0.9", "ubiquiti"),
            ("UniFi AP-AC-Pro", "ubiquiti"),
            ("Linux servidor 6.1.0-18-amd64", "linux"),
        ] {
            let (chave, _) = deduz_vendor(None, Some(descricao));
            assert_eq!(chave, esperado, "não reconheceu {descricao:?}");
        }
    }

    #[tokio::test]
    async fn porta_fechada_no_loopback_responde_falso_sem_travar() {
        // `127.0.0.1` só, conforme as diretrizes de teste: nada sai da máquina.
        assert!(!porta_aberta(ip("127.0.0.1"), 9).await);
    }
}
