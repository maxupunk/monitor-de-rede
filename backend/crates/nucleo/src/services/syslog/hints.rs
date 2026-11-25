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
//! - **o sistema do equipamento**, que decide quais comandos serão enviados. O
//!   catálogo é o de [`crate::services::devices::systems`], o mesmo do cadastro
//!   e do assistente da VPN — a dedução aqui só escolhe uma entrada dele.
//!
//! Nada aqui é decisão final: tudo volta para a tela como sugestão editável. O
//! módulo responde "o que dá para saber", não "o que vai ser feito".

use std::{
    net::{IpAddr, SocketAddr, UdpSocket},
    time::Duration,
};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use tokio::{net::TcpStream, time::timeout};

use netmonitor_core::devices::{
    access::{DeviceAccess, SnmpSystemInfo},
    snmp_credentials::{self, SnmpCredentials},
};

use super::nat::NatDetector;
use crate::{
    models::_entities::{device_interfaces, devices, discovery_results},
    services::{devices::systems, shared::errors::AppResult},
};

/// Quanto se espera por uma porta antes de considerá-la fechada.
///
/// Curto de propósito: são duas sondas em série na abertura de um diálogo, e o
/// alvo está na rede local. Porta filtrada por firewall gasta o teto inteiro,
/// e é por isso que ele não pode ser generoso.
const TETO_DA_SONDA: Duration = Duration::from_millis(1500);

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
    /// Sistema em vigor — declarado no cadastro ou deduzido.
    pub operating_system: String,
    /// De onde veio — ver [`systems::source`].
    pub operating_system_source: &'static str,
    /// A frase que explica **por que** este sistema, para a tela poder mostrar
    /// a conclusão em vez de só afirmá-la.
    pub operating_system_reason: String,
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
    access: Option<&dyn DeviceAccess>,
    dispositivo: &devices::Model,
    nat: &NatDetector,
) -> AppResult<ProvisionHints> {
    let host = dispositivo
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .and_then(|texto| texto.parse::<IpAddr>().ok());

    // SNMP, sondas SSH/Telnet e busca do MAC correm em paralelo para não somar
    // os timeouts na abertura do diálogo.
    let snmp_fut = async {
        match host {
            Some(endereco) if dispositivo.snmp_enabled => {
                sistema_por_snmp(db, access, endereco, dispositivo).await
            }
            _ => None,
        }
    };
    let probes_fut = async {
        match host {
            Some(endereco) => tokio::join!(sonda_ssh(endereco), porta_aberta(endereco, 23)),
            None => ((false, None), false),
        }
    };
    let (sistema_snmp, (ssh, telnet_open), mac_address) =
        tokio::join!(snmp_fut, probes_fut, mac_conhecido(db, dispositivo));
    let (ssh_open, ssh_banner) = ssh;
    let mac_address = mac_address?;

    let deteccao = systems::detect(&systems::Evidence {
        declared: dispositivo.operating_system.as_deref(),
        sys_object_id: sistema_snmp
            .as_ref()
            .and_then(|info| info.sys_object_id.as_deref()),
        sys_descr: sistema_snmp
            .as_ref()
            .and_then(|info| info.sys_descr.as_deref()),
        ssh_banner: ssh_banner.as_deref(),
        name: Some(&dispositivo.name),
        vendor: dispositivo.vendor.as_deref(),
        model: dispositivo.model.as_deref(),
        observed: systems::Evidence::from_device(dispositivo).observed,
    });

    // Num container em rede bridge a rota responde com o IP da ponte — correto
    // para o kernel, inútil para o roteador, que não alcança a ponte do Docker.
    // Omitir é o que deixa a tela pedir o endereço em vez de gravar um palpite
    // que falha em silêncio. O `server_addresses::da_rede_local` aplica o mesmo
    // critério à entrada "rede local" da lista.
    let (server_address, server_address_source) = if nat.bridged_container() {
        (
            None,
            "container em rede bridge — o detectado seria o IP da ponte",
        )
    } else {
        match host.and_then(local_address_toward) {
            Some(endereco) => (Some(endereco.to_string()), "rota até o equipamento"),
            None => (None, "desconhecido"),
        }
    };

    Ok(ProvisionHints {
        server_address,
        server_address_source,
        ssh_open,
        telnet_open,
        operating_system: deteccao.system.id.to_owned(),
        operating_system_source: deteccao.source,
        operating_system_reason: deteccao.reason,
        mac_address,
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
        .order_by_desc(discovery_results::Column::Id)
        .one(db)
        .await?
        .and_then(|linha| linha.mac_address)
        .filter(|valor| !valor.trim().is_empty()))
}

/// Lê a identidade SNMP do equipamento pelo acesso da Rede — ver
/// `netmonitor_network::device_access::snmp_identity`. Sem a área não há
/// palpite.
pub async fn identidade_snmp(
    access: Option<&dyn DeviceAccess>,
    host: IpAddr,
    credencial: &SnmpCredentials,
) -> Option<SnmpSystemInfo> {
    access?.snmp_identity(host, credencial).await
}

/// A identidade SNMP com a credencial do dispositivo (a dele ou a padrão).
/// Credencial ilegível vira "sem palpite", como agente fora do ar.
async fn sistema_por_snmp(
    db: &sea_orm::DatabaseConnection,
    access: Option<&dyn DeviceAccess>,
    host: IpAddr,
    dispositivo: &devices::Model,
) -> Option<SnmpSystemInfo> {
    let credencial = snmp_credentials::for_device(db, dispositivo).await.ok()?;
    identidade_snmp(access, host, &credencial).await
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
/// ponto de vista do kernel e inútil para o roteador. Quem separa os dois
/// casos é o chamador: [`collect`] e o `server_addresses` o descartam nesse
/// ambiente.
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

/// Valida hostname segundo RFC 1123 (`[a-zA-Z0-9.-]`, rótulos de 1..=63 bytes,
/// sem hífen nas pontas e sem caracteres de controle/shell).
///
/// Como o valor é interpolado em comandos SSH/Telnet enviados a roteadores e
/// hosts, restringir o conjunto de caracteres bloqueia injeção de comandos.
///
/// Nome todo numérico (`127.1`, `10.0.0.256`) não é hostname: o resolvedor de
/// muito equipamento o lê como IP abreviado — `127.1` é o loopback — e ele
/// escaparia do crivo de [`utilizavel`]. O ponto final (`127.0.0.1.`) sai pelo
/// mesmo motivo: é a forma de contrabandear um IP como "nome".
fn is_valid_hostname(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 || host.ends_with('.') {
        return false;
    }
    let ultimo_rotulo = host.rsplit('.').next().unwrap_or(host);
    if ultimo_rotulo.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

/// Filtra um endereço vindo da tela (`IP`, `IP:porta`, `[IPv6]:porta`,
/// `hostname` ou `hostname:porta`). Texto vazio ou inútil vira `None`, para o
/// chamador cair no palpite do servidor em vez de gravar lixo no equipamento.
#[must_use]
pub fn sanitiza_endereco(bruto: Option<&str>) -> Option<String> {
    let limpo = bruto.map(str::trim).filter(|valor| !valor.is_empty())?;
    if let Ok(endereco) = limpo.parse::<IpAddr>() {
        return utilizavel(endereco).then(|| limpo.to_owned());
    }
    if let Ok(socket) = limpo.parse::<SocketAddr>() {
        return (socket.port() > 0 && utilizavel(socket.ip())).then(|| limpo.to_owned());
    }
    let host = match limpo.rsplit_once(':') {
        Some((host, porta)) if porta.parse::<u16>().is_ok_and(|porta| porta > 0) => host,
        Some(_) => return None,
        None => limpo,
    };
    // `localhost` é o nome que todo mundo resolve para o lugar errado, então
    // sai pelo nome mesmo.
    if host.eq_ignore_ascii_case("localhost") || host.eq_ignore_ascii_case("localhost.localdomain")
    {
        return None;
    }
    is_valid_hostname(host).then(|| limpo.to_owned())
}

/// Separa a porta de um endereço já saneado (`IP:porta`, `[IPv6]:porta`,
/// `nome:porta`). Sem porta, devolve o próprio endereço.
#[must_use]
pub fn sem_porta(endereco: &str) -> &str {
    if endereco.parse::<IpAddr>().is_ok() {
        return endereco;
    }
    if endereco.parse::<SocketAddr>().is_ok() {
        // `[::1]:80` → `::1`; `10.0.0.1:80` → `10.0.0.1`.
        let host = endereco.rsplit_once(':').map_or(endereco, |(host, _)| host);
        return host.trim_start_matches('[').trim_end_matches(']');
    }
    match endereco.rsplit_once(':') {
        Some((host, porta)) if porta.parse::<u16>().is_ok() => host,
        _ => endereco,
    }
}

/// O endereço foi recusado por apontar o equipamento para ele mesmo
/// (`localhost`, loopback, não-especificado, link-local) — e não por ter um
/// formato inválido. É o que separa as duas mensagens de erro da tela.
#[must_use]
pub fn aponta_para_si_mesmo(bruto: &str) -> bool {
    let host = sem_porta(bruto.trim());
    host.eq_ignore_ascii_case("localhost")
        || host.eq_ignore_ascii_case("localhost.localdomain")
        || host.parse::<IpAddr>().is_ok_and(|ip| !utilizavel(ip))
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

/// Teto para ler a linha de identificação depois da conexão.
///
/// Separado do teto de conexão porque são esperas diferentes: conectar depende
/// da rede, e a linha vem imediatamente depois — servidor que não a manda em
/// meio segundo não vai mandar.
const TETO_DO_BANNER: Duration = Duration::from_millis(1200);

/// Máximo que se lê da identificação. O RFC 4253 limita a 255 bytes.
const TETO_DE_BYTES: usize = 255;

/// Sonda a porta 22 e, de quebra, lê o que o servidor SSH anuncia.
///
/// A identificação (`SSH-2.0-dropbear_2022.82`) é a **primeira coisa** que o
/// servidor envia, antes de qualquer negociação ou autenticação — o mesmo
/// `connect` que já sondava a porta a traz de graça. E ela responde uma pergunta
/// que o SNMP não responde: o agente de um OpenWrt costuma devolver só o `uname`
/// (`Linux bpi-r3 6.12.87 aarch64`), enquanto o `dropbear` no banner diz
/// exatamente qual firmware é.
///
/// Nada é enviado por este lado, e a conexão morre com o socket. Falha na
/// leitura devolve `None` sem afetar o "a porta está aberta": são duas
/// perguntas, e a segunda não pode estragar a primeira.
pub async fn sonda_ssh(host: IpAddr) -> (bool, Option<String>) {
    sonda_ssh_em(host, 22).await
}

/// A mesma sonda numa porta escolhida — o SSH de muitos equipamentos não está
/// na 22 (a porta da credencial cadastrada diz onde está).
pub async fn sonda_ssh_em(host: IpAddr, porta: u16) -> (bool, Option<String>) {
    use tokio::io::AsyncReadExt;

    let Ok(Ok(mut fluxo)) = timeout(
        TETO_DA_SONDA,
        TcpStream::connect(SocketAddr::new(host, porta)),
    )
    .await
    else {
        return (false, None);
    };

    let mut buffer = [0_u8; TETO_DE_BYTES];
    let lidos = match timeout(TETO_DO_BANNER, fluxo.read(&mut buffer)).await {
        Ok(Ok(lidos)) if lidos > 0 => lidos,
        _ => return (true, None),
    };

    // Só a primeira linha: o que vem depois já é a negociação binária, e
    // arrastá-la para dentro de um texto de diagnóstico não ajuda ninguém.
    let texto = String::from_utf8_lossy(&buffer[..lidos]);
    let linha = texto
        .lines()
        .next()
        .map(str::trim)
        .filter(|linha| !linha.is_empty())
        .map(str::to_owned);
    (true, linha)
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
        assert_eq!(sanitiza_endereco(Some("localhost:8080")), None);
        assert_eq!(sanitiza_endereco(Some(" 127.0.0.1 ")), None);
        assert_eq!(sanitiza_endereco(Some("127.0.0.1:8080")), None);
        assert_eq!(sanitiza_endereco(Some("::1")), None);
        assert_eq!(sanitiza_endereco(Some("[::1]:8080")), None);
        assert_eq!(sanitiza_endereco(Some("0.0.0.0")), None);
        assert_eq!(sanitiza_endereco(Some("172.20.0.5:0")), None);
    }

    #[test]
    fn endereco_de_verdade_passa_inteiro() {
        assert_eq!(
            sanitiza_endereco(Some(" 192.168.1.10 ")),
            Some("192.168.1.10".to_owned())
        );
        assert_eq!(
            sanitiza_endereco(Some("172.20.0.5:9000")),
            Some("172.20.0.5:9000".to_owned())
        );
        // Nome que não é `localhost` continua valendo: o roteador pode muito
        // bem resolvê-lo, e recusar aqui tiraria uma opção legítima.
        assert_eq!(
            sanitiza_endereco(Some("netmonitor.lan")),
            Some("netmonitor.lan".to_owned())
        );
        assert_eq!(
            sanitiza_endereco(Some("casa.ddns.net:8080")),
            Some("casa.ddns.net:8080".to_owned())
        );
    }

    #[test]
    fn hostname_com_metacaracteres_de_shell_e_recusado() {
        for invalido in [
            "10.0.0.1; id",
            "host$(whoami)",
            "host|cat /etc/passwd",
            "host\nid",
            "-inicia-com-hifen",
            "termina-com-hifen-",
            "espaco no meio",
        ] {
            assert_eq!(
                sanitiza_endereco(Some(invalido)),
                None,
                "aceitou hostname perigoso: {invalido:?}"
            );
        }
    }

    #[test]
    fn nome_todo_numerico_e_ponto_final_nao_contrabandeiam_ip() {
        for invalido in [
            "127.0.0.1.",
            "127.1",
            "127.1:514",
            "10.0.0.256",
            "1.2.3.4.5",
            "2130706433",
            "netmonitor.lan.",
        ] {
            assert_eq!(
                sanitiza_endereco(Some(invalido)),
                None,
                "aceitou {invalido:?}"
            );
        }
        // Rótulo numérico no meio do nome continua valendo.
        assert_eq!(
            sanitiza_endereco(Some("10.rede.lan")),
            Some("10.rede.lan".to_owned())
        );
    }

    #[test]
    fn a_porta_sai_sem_levar_o_host_junto() {
        assert_eq!(sem_porta("172.20.0.5:9000"), "172.20.0.5");
        assert_eq!(sem_porta("[2001:db8::1]:9000"), "2001:db8::1");
        assert_eq!(sem_porta("2001:db8::1"), "2001:db8::1");
        assert_eq!(sem_porta("casa.ddns.net:8080"), "casa.ddns.net");
        assert_eq!(sem_porta("netmonitor.lan"), "netmonitor.lan");
    }

    #[test]
    fn so_loopback_e_afins_contam_como_apontar_para_si() {
        for proprio in [
            "localhost",
            "localhost:8080",
            "127.0.0.1",
            "[::1]:80",
            "0.0.0.0",
        ] {
            assert!(aponta_para_si_mesmo(proprio), "{proprio}");
        }
        for formato in ["127.1", "host$(id)", "10.0.0.1; id", "127.0.0.1."] {
            assert!(!aponta_para_si_mesmo(formato), "{formato}");
        }
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

    // A dedução do sistema mudou de casa: os testes dela vivem em
    // `services::devices::systems`, junto do catálogo que a decide.

    #[tokio::test]
    async fn porta_fechada_no_loopback_responde_falso_sem_travar() {
        // `127.0.0.1` só, conforme as diretrizes de teste: nada sai da máquina.
        assert!(!porta_aberta(ip("127.0.0.1"), 9).await);
    }
}
