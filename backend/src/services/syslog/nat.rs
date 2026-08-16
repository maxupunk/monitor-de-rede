//! Detecção da origem mascarada por NAT — o item aberto da ADR 008.
//!
//! O spike da Fase 1 deixou uma pergunta sem medir: *qual é o IP de origem
//! observado dentro do container?* A publicação `514:5514` do compose pode
//! preservar o endereço do roteador ou reescrevê-lo para o gateway da bridge.
//! Ela reescreve — sempre no Docker Desktop (Windows e macOS, onde tudo passa
//! pelo proxy da VM) e no Linux sempre que o `userland-proxy` entra no caminho.
//!
//! Quando isso acontece, **o parque inteiro chega com um único IP de origem** e
//! três coisas quebram de uma vez:
//!
//! 1. o passo 1 do resolvedor (`source_ip == devices.ip_address`) não casa com
//!    ninguém, e o passo 3 (CIDR das redes) também não — o gateway da bridge
//!    não pertence a nenhuma rede cadastrada. Todo log vira fonte desconhecida
//!    e é descartado;
//! 2. o limitador por fonte passa a ver **um** remetente de 30 roteadores, e os
//!    50 msg/s viram teto do parque inteiro;
//! 3. o *bind* manual da tela de origens — o escape natural do operador — é uma
//!    armadilha: vincular `172.17.0.1` a um dispositivo faz **todos** os
//!    roteadores virarem aquele dispositivo. É exatamente a contaminação que o
//!    §3 do roadmap diz ser pior do que não vincular.
//!
//! Este módulo responde só à pergunta "este IP de origem é um gateway de NAT?".
//! O que fazer com a resposta é do `resolver` (resolve por hostname) e do
//! `ingest` (separa o limitador e a lista de fontes por hostname).
//!
//! **Detectar não conserta a topologia.** Um roteador que não manda `HOSTNAME`
//! continua indistinguível dos outros atrás do NAT. A correção de verdade é
//! `network_mode: host` no compose — documentada lá e repetida no aviso da
//! tela. O que este módulo compra é o sistema funcionar mesmo sem ela, e dizer
//! ao operador o que está acontecendo em vez de descartar em silêncio.

use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr},
};

/// Faixas privadas que o Docker usa por padrão para as suas bridges.
///
/// Não é "todo IP privado": `192.168.0.0/16` inteiro está fora de propósito —
/// é onde moram os roteadores de verdade, e tratá-lo como NAT descartaria o
/// caso normal. Só entram as faixas que o Docker aloca sozinho quando ninguém
/// pediu nada:
///
/// - `172.17.0.0/16` … `172.31.0.0/16` — o pool padrão do `docker0` e das
///   redes que o compose cria;
/// - `192.168.65.0/24` — a rede da VM do Docker Desktop (macOS e Windows), de
///   onde sai o `192.168.65.1` que o container enxerga como origem de tudo.
///
/// Só o **endereço de gateway** conta, e não a faixa inteira: um container
/// vizinho que legitimamente mande syslog de `172.18.0.7` é uma fonte real,
/// não um mascaramento. Daí o terceiro campo — o tamanho da sub-rede que o
/// Docker recorta da faixa. O gateway é sempre o primeiro endereço útil dela:
/// `/16` dentro do `172.16.0.0/12` dá `172.18.0.1`; o `/24` do Docker Desktop
/// dá `192.168.65.1`.
const FAIXAS_DE_BRIDGE: &[FaixaDeBridge] = &[
    FaixaDeBridge {
        base: Ipv4Addr::new(172, 16, 0, 0),
        prefixo: 12,
        prefixo_da_subrede: 16,
    },
    FaixaDeBridge {
        base: Ipv4Addr::new(192, 168, 65, 0),
        prefixo: 24,
        prefixo_da_subrede: 24,
    },
];

/// Uma faixa do pool do Docker e o recorte que ele faz dentro dela.
struct FaixaDeBridge {
    base: Ipv4Addr,
    prefixo: u8,
    prefixo_da_subrede: u8,
}

/// Variável que acrescenta (ou substitui) endereços tratados como gateway.
///
/// Existe porque nenhuma heurística cobre um `docker network create` com
/// `--subnet` fora do pool padrão, nem um proxy reverso de syslog na frente.
/// Aceita lista separada por vírgula.
pub const ENV_GATEWAYS: &str = "SYSLOG_NAT_GATEWAYS";

/// O que se sabe sobre o mascaramento neste processo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NatDetector {
    /// Endereços conhecidos como gateway: o do próprio container mais os
    /// declarados na variável de ambiente.
    gateways: BTreeSet<IpAddr>,
    /// A rota padrão vista de dentro, separada dos endereços declarados à mão:
    /// é ela que diz em que tipo de rede este processo está.
    default_gateway: Option<IpAddr>,
    /// Se o processo está mesmo dentro de um container. Só informativo — a
    /// decisão de mascaramento é por endereço, não por ambiente.
    pub containerized: bool,
}

impl NatDetector {
    /// Monta o detector lendo o ambiente e o `/proc` do container.
    ///
    /// Roda uma vez por processo: o gateway de um container não muda enquanto
    /// ele vive.
    #[must_use]
    pub fn detect() -> Self {
        let mut gateways = BTreeSet::new();
        let default_gateway = gateway_padrao();
        if let Some(gateway) = default_gateway {
            gateways.insert(gateway);
        }
        for endereco in declarados_no_ambiente() {
            gateways.insert(endereco);
        }
        Self {
            gateways,
            default_gateway,
            containerized: em_container(),
        }
    }

    /// Detector vazio — nada é considerado mascarado. É o que o teste usa
    /// quando quer exercitar o caminho normal.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Se este endereço é o gateway de um NAT em vez de um remetente real.
    ///
    /// Duas evidências, em ordem de confiança: bater com o gateway lido do
    /// `/proc/net/route` (ou declarado na variável) é prova; cair no `.1` de
    /// uma faixa de bridge do Docker é indício forte, e cobre o caso do
    /// processo rodando fora do container mas recebendo de um.
    #[must_use]
    pub fn is_masked(&self, endereco: IpAddr) -> bool {
        if self.gateways.contains(&endereco) {
            return true;
        }
        let IpAddr::V4(v4) = endereco else {
            return false;
        };
        FAIXAS_DE_BRIDGE.iter().any(|faixa| faixa.e_gateway(v4))
    }

    /// Os endereços conhecidos, para a tela poder nomeá-los no aviso.
    #[must_use]
    pub fn gateways(&self) -> Vec<String> {
        self.gateways.iter().map(ToString::to_string).collect()
    }

    /// Se este processo está num container em rede **bridge** — isto é, sem
    /// acesso à camada 2 da rede onde os equipamentos moram.
    ///
    /// A evidência é a rota padrão apontar para uma ponte do Docker. Em
    /// `network_mode: host` ela aponta para o gateway da LAN, e o processo
    /// enxerga a rede como qualquer outro programa da máquina.
    ///
    /// Quem depende disso é o MAC-Telnet: ele descobre o equipamento por
    /// difusão na LAN, e difusão não atravessa a ponte. Sem esta resposta a
    /// tela ofereceria um meio de acesso que não tem como funcionar ali.
    #[must_use]
    pub fn bridged_container(&self) -> bool {
        self.containerized
            && self
                .default_gateway
                .is_some_and(|rota| self.is_masked(rota))
    }
}

impl FaixaDeBridge {
    /// Se o endereço é o gateway da sub-rede que o Docker recortaria aqui.
    ///
    /// Duas perguntas: pertence à faixa, e é o primeiro endereço útil da sua
    /// sub-rede? Só o segundo separa a ponte (`172.18.0.1`) do container
    /// vizinho (`172.18.0.7`), que é fonte legítima.
    fn e_gateway(&self, endereco: Ipv4Addr) -> bool {
        let bits = u32::from(endereco);
        if bits & mascara(self.prefixo) != u32::from(self.base) & mascara(self.prefixo) {
            return false;
        }
        let inicio_da_subrede = bits & mascara(self.prefixo_da_subrede);
        bits == inicio_da_subrede + 1
    }
}

/// Máscara de um prefixo CIDR. `/0` vira zero — o `checked_shl` evita o
/// *overflow* que um `<<32` causaria.
fn mascara(prefixo: u8) -> u32 {
    u32::MAX
        .checked_shl(u32::from(32 - prefixo.min(32)))
        .unwrap_or_default()
}

/// Lê os gateways declarados à mão. Entrada inválida é ignorada, nunca fatal:
/// um erro de digitação no compose não pode calar a ingestão.
fn declarados_no_ambiente() -> Vec<IpAddr> {
    std::env::var(ENV_GATEWAYS)
        .unwrap_or_default()
        .split(',')
        .filter_map(|item| item.trim().parse::<IpAddr>().ok())
        .collect()
}

/// O gateway padrão visto de dentro do container, lido do `/proc/net/route`.
///
/// O formato é tabular, com os endereços em hexadecimal **little-endian**: a
/// rota padrão é a linha com `Destination` zerado, e o `Gateway` dela é o IP
/// procurado. Fora do Linux o arquivo não existe e a função devolve `None` —
/// que é o correto, porque fora do Linux não há bridge do Docker no caminho.
fn gateway_padrao() -> Option<IpAddr> {
    let conteudo = std::fs::read_to_string("/proc/net/route").ok()?;
    for linha in conteudo.lines().skip(1) {
        let mut colunas = linha.split_whitespace();
        let _interface = colunas.next()?;
        let destino = colunas.next()?;
        let gateway = colunas.next()?;
        if destino != "00000000" {
            continue;
        }
        if let Some(endereco) = hex_little_endian(gateway) {
            return Some(IpAddr::V4(endereco));
        }
    }
    None
}

/// `0011A8C0` → `192.168.17.0`.
///
/// O kernel escreve o endereço na ordem de bytes do host, que é little-endian
/// nas máquinas em que este sistema roda. Ler como big-endian devolve o IP ao
/// contrário — e um `0.17.168.192` não casa com nada, então o mascaramento
/// passaria despercebido em vez de dar erro.
fn hex_little_endian(texto: &str) -> Option<Ipv4Addr> {
    let bruto = u32::from_str_radix(texto, 16).ok()?;
    Some(Ipv4Addr::from(bruto.to_le_bytes()))
}

/// Se o processo roda dentro de um container. Usado só para o texto do aviso:
/// nada muda de comportamento por causa disto.
fn em_container() -> bool {
    std::path::Path::new("/.dockerenv").exists()
        || std::fs::read_to_string("/proc/1/cgroup")
            .is_ok_and(|texto| texto.contains("docker") || texto.contains("containerd"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn ip(texto: &str) -> IpAddr {
        texto.parse().expect("ip do teste")
    }

    #[test]
    fn o_gateway_da_bridge_padrao_e_reconhecido_sem_configuracao() {
        let detector = NatDetector::none();
        // O `docker0` e as redes que o compose cria.
        assert!(detector.is_masked(ip("172.17.0.1")));
        assert!(detector.is_masked(ip("172.18.0.1")));
        assert!(detector.is_masked(ip("172.31.0.1")));
        // A VM do Docker Desktop no Windows e no macOS.
        assert!(detector.is_masked(ip("192.168.65.1")));
    }

    #[test]
    fn o_ip_de_um_roteador_de_verdade_nunca_e_tratado_como_mascarado() {
        let detector = NatDetector::none();
        // A faixa doméstica inteira fica de fora: é onde moram os roteadores.
        assert!(!detector.is_masked(ip("192.168.1.1")));
        assert!(!detector.is_masked(ip("192.168.88.1")));
        assert!(!detector.is_masked(ip("10.0.0.1")));
        // Fora do pool que o Docker aloca sozinho.
        assert!(!detector.is_masked(ip("172.15.0.1")));
        assert!(!detector.is_masked(ip("172.32.0.1")));
    }

    #[test]
    fn container_vizinho_nao_e_gateway() {
        // Só o `.1` da ponte mascara. Um container que mande syslog do próprio
        // endereço é fonte real e precisa continuar resolvendo pelo inventário.
        let detector = NatDetector::none();
        assert!(!detector.is_masked(ip("172.17.0.7")));
        assert!(!detector.is_masked(ip("172.18.3.42")));
    }

    #[test]
    #[serial]
    fn a_variavel_acrescenta_gateway_fora_do_pool_padrao() {
        // `docker network create --subnet` fora do pool, ou um relay de syslog
        // na frente: nenhuma heurística cobre, e a variável é a saída.
        std::env::set_var(ENV_GATEWAYS, "10.44.0.1, 203.0.113.7 ,lixo");
        let detector = NatDetector::detect();
        assert!(detector.is_masked(ip("10.44.0.1")));
        assert!(detector.is_masked(ip("203.0.113.7")));
        assert!(!detector.is_masked(ip("10.44.0.2")));
        std::env::remove_var(ENV_GATEWAYS);
    }

    #[test]
    #[serial]
    fn entrada_invalida_na_variavel_nao_derruba_a_deteccao() {
        std::env::set_var(ENV_GATEWAYS, "não é ip");
        let detector = NatDetector::detect();
        assert!(!detector.is_masked(ip("172.15.0.1")), "seguiu funcionando");
        std::env::remove_var(ENV_GATEWAYS);
    }

    #[test]
    fn a_rota_padrao_e_lida_em_little_endian() {
        // `/proc/net/route` guarda `192.168.17.0` como `0011A8C0`.
        assert_eq!(
            hex_little_endian("0011A8C0"),
            Some(Ipv4Addr::new(192, 168, 17, 0))
        );
        // `172.17.0.1` — o gateway típico do `docker0`.
        assert_eq!(
            hex_little_endian("010011AC"),
            Some(Ipv4Addr::new(172, 17, 0, 1))
        );
        assert_eq!(hex_little_endian("não é hex"), None);
    }

    #[test]
    fn ipv6_nunca_e_mascarado_por_heuristica() {
        // Não há pool equivalente documentado; chutar aqui descartaria log
        // legítimo de rede IPv6.
        let detector = NatDetector::none();
        assert!(!detector.is_masked(ip("fd00::1")));
    }
}
