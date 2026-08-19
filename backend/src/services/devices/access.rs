//! De onde este servidor alcança cada equipamento.
//!
//! # A pergunta que isto responde
//!
//! Tudo que o NetMonitor manda um equipamento fazer — enviar syslog, fechar um
//! túnel, apontar um backup — precisa dizer **por qual endereço voltar**. A
//! lista de [`crate::services::server_addresses`] cataloga esses endereços; o
//! que faltava era ligar cada equipamento a um deles.
//!
//! Antes disso, cada tela pedia o endereço de novo. E pedir de novo não é só
//! trabalho repetido: é uma pergunta que o operador responde por eliminação,
//! sem elementos para escolher, num campo onde errar falha em silêncio dentro
//! do roteador.
//!
//! | forma de acesso | endereço que o equipamento usa |
//! |---|---|
//! | `local`  | o IP deste servidor na LAN |
//! | `vpn`    | o IP deste servidor dentro do túnel |
//! | `remote` | o IP público ou o DDNS |
//!
//! # Declaração e dedução são coisas diferentes
//!
//! A coluna `devices.access_mode` guarda **o que o operador declarou**, e
//! `NULL` é o valor honesto para "não declarei". A dedução é recalculada a cada
//! leitura, a partir de evidência que envelhece — peer da VPN, faixa do túnel,
//! endereço privado ou global. Gravar a dedução na coluna a congelaria: um
//! equipamento que sai da VPN continuaria marcado como VPN até alguém reparar.
//!
//! Por isso a declaração vence a dedução, e não o contrário: quem declara sabe
//! de algo que o servidor não tem como observar — a filial atrás de outra VPN,
//! por exemplo, cujo IP privado é indistinguível do de um vizinho de LAN.

use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr},
};

use ipnet::IpNet;
use sea_orm::{ConnectionTrait, EntityTrait};

use crate::{
    models::_entities::{devices, networks, vpn_peers, vpn_servers},
    services::{shared::errors::AppResult, vpn::cidr::parse_cidr},
};

/// As três situações que mudam o endereço a ser gravado no equipamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Local,
    Vpn,
    Remote,
}

impl AccessMode {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Vpn => "vpn",
            Self::Remote => "remote",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Local => "Rede local",
            Self::Vpn => "Túnel VPN",
            Self::Remote => "Internet (remoto)",
        }
    }

    /// O tipo de endereço da lista do servidor que corresponde a esta forma de
    /// acesso. É esta função que faz a declaração do cadastro valer alguma
    /// coisa concreta — sem ela, `access_mode` seria mais um rótulo decorativo.
    #[must_use]
    pub const fn address_kind(self) -> &'static str {
        match self {
            Self::Local => "lan",
            Self::Vpn => "vpn",
            Self::Remote => "public",
        }
    }

    /// Lê o valor vindo da API. `auto` e vazio significam "sem declaração".
    ///
    /// # Errors
    ///
    /// Valor fora do vocabulário — a mensagem lista o que é aceito, porque um
    /// "valor inválido" seco obriga a caçar o enum no código.
    pub fn parse(bruto: &str) -> Result<Option<Self>, String> {
        match bruto.trim().to_ascii_lowercase().as_str() {
            "" | "auto" | "automatic" | "automático" => Ok(None),
            "local" | "lan" => Ok(Some(Self::Local)),
            "vpn" => Ok(Some(Self::Vpn)),
            "remote" | "remoto" | "public" => Ok(Some(Self::Remote)),
            outro => Err(format!(
                "Forma de acesso desconhecida: `{outro}`. Use `auto`, `local`, `vpn` ou `remote`."
            )),
        }
    }
}

/// A conclusão sobre um equipamento.
#[derive(Debug, Clone)]
pub struct ResolvedAccess {
    pub mode: AccessMode,
    /// Se veio do cadastro (verdadeiro) ou de dedução (falso). A tela mostra a
    /// diferença: dedução apresentada como certeza é o começo de um erro que
    /// ninguém revisa.
    pub declared: bool,
    /// Por que esta conclusão. É a frase que a tela exibe.
    pub reason: String,
}

/// O que se consulta uma vez para julgar muitos dispositivos.
///
/// Existe como contexto, e não como função por dispositivo, porque
/// `present_many` serializa a lista inteira: uma consulta por linha traria de
/// volta o N+1 que aquela função foi escrita para eliminar.
#[derive(Debug, Clone, Default)]
pub struct AccessContext {
    peers: HashSet<i64>,
    vpn_network_id: Option<i64>,
    vpn_cidr: Option<String>,
    /// `(nome, cidr)` das redes cadastradas, exceto a do túnel.
    redes: Vec<(String, String)>,
}

impl AccessContext {
    /// Carrega o contexto em três consultas, independente do tamanho da lista.
    ///
    /// # Errors
    ///
    /// Propaga erro do banco.
    pub async fn load<C: ConnectionTrait>(db: &C) -> AppResult<Self> {
        let peers: HashSet<i64> = vpn_peers::Entity::find()
            .all(db)
            .await?
            .into_iter()
            .map(|linha| linha.device_id)
            .collect();
        let servidor = vpn_servers::Entity::find().one(db).await?;
        let vpn_network_id = servidor.map(|linha| linha.network_id);

        let mut vpn_cidr = None;
        let mut redes = Vec::new();
        for rede in networks::Entity::find().all(db).await? {
            if Some(rede.id) == vpn_network_id {
                vpn_cidr = Some(rede.cidr);
            } else {
                redes.push((rede.name, rede.cidr));
            }
        }

        Ok(Self {
            peers,
            vpn_network_id,
            vpn_cidr,
            redes,
        })
    }

    /// A rede cadastrada (exceto a do túnel) que contém **os dois** endereços.
    ///
    /// É a prova de que equipamento e servidor se alcançam direto: mesmo CIDR
    /// no inventário significa sem NAT e sem túnel no caminho. É o que salva a
    /// sugestão de endereço num container em rede bridge, onde a rota do kernel
    /// responde o IP da ponte em vez do IP da máquina.
    #[must_use]
    pub fn rede_em_comum(&self, a: IpAddr, b: IpAddr) -> Option<&(String, String)> {
        self.redes
            .iter()
            .find(|(_, cidr)| contem(cidr, a) && contem(cidr, b))
    }

    /// A conclusão para um dispositivo.
    #[must_use]
    pub fn resolve(&self, device: &devices::Model) -> ResolvedAccess {
        if let Some(declarado) = device
            .access_mode
            .as_deref()
            .and_then(|bruto| AccessMode::parse(bruto).ok().flatten())
        {
            return ResolvedAccess {
                mode: declarado,
                declared: true,
                reason: format!("definido no cadastro como \"{}\"", declarado.label()),
            };
        }
        self.deduz(device)
    }

    /// A dedução, da evidência mais forte para a mais fraca.
    fn deduz(&self, device: &devices::Model) -> ResolvedAccess {
        // Peer é fato registrado, não inferência: existe uma linha ligando este
        // dispositivo ao servidor WireGuard.
        if self.peers.contains(&device.id) {
            return deduzido(
                AccessMode::Vpn,
                "é um peer do túnel WireGuard deste servidor",
            );
        }

        let ip = device
            .ip_address
            .as_deref()
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .and_then(|texto| texto.parse::<IpAddr>().ok());

        if device.network_id.is_some() && device.network_id == self.vpn_network_id {
            return deduzido(AccessMode::Vpn, "está cadastrado na rede do túnel");
        }
        if let (Some(IpAddr::V4(v4)), Some(cidr)) = (ip, self.vpn_cidr.as_deref()) {
            if dentro(v4, cidr) {
                return deduzido(
                    AccessMode::Vpn,
                    &format!("o IP está dentro da faixa do túnel ({cidr})"),
                );
            }
        }

        let Some(ip) = ip else {
            // Sem IP não há evidência nenhuma. O padrão é a rede local porque é
            // o caso mais comum — e a frase diz que é padrão, não conclusão.
            return deduzido(
                AccessMode::Local,
                "sem IP cadastrado — assumida a rede local",
            );
        };

        if let IpAddr::V4(v4) = ip {
            for (nome, cidr) in &self.redes {
                if dentro(v4, cidr) {
                    return deduzido(
                        AccessMode::Local,
                        &format!("o IP está na rede cadastrada \"{nome}\" ({cidr})"),
                    );
                }
            }
        }

        if privado(ip) {
            deduzido(
                AccessMode::Local,
                "o IP é de faixa privada, alcançável sem sair desta rede",
            )
        } else {
            deduzido(
                AccessMode::Remote,
                "o IP é público — o equipamento está fora desta rede",
            )
        }
    }
}

fn deduzido(mode: AccessMode, motivo: &str) -> ResolvedAccess {
    ResolvedAccess {
        mode,
        declared: false,
        reason: motivo.to_owned(),
    }
}

/// Se um IPv4 cai dentro de um CIDR. CIDR ilegível responde `false` — uma rede
/// mal cadastrada não pode classificar equipamento nenhum.
fn dentro(ip: Ipv4Addr, cidr: &str) -> bool {
    let Ok(faixa) = parse_cidr(cidr) else {
        return false;
    };
    let bruto = u32::from(ip);
    bruto >= u32::from(faixa.network_address) && bruto <= u32::from(faixa.broadcast_address)
}

/// O mesmo teste para qualquer versão de IP — o [`dentro`] é IPv4 porque a
/// dedução de acesso só tem faixa de túnel em v4; a rede em comum precisa
/// valer também para v6.
fn contem(cidr: &str, ip: IpAddr) -> bool {
    cidr.parse::<IpNet>().is_ok_and(|rede| rede.contains(&ip))
}

/// Faixas que não trafegam pela internet.
///
/// A CGNAT (100.64.0.0/10) entra junto com as três da RFC 1918 porque é o que
/// operadora entrega em link residencial — e um equipamento ali não é alcançável
/// de fora, que é justamente o que esta pergunta quer saber.
fn privado(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
        }
        IpAddr::V6(v6) => {
            // ULA (fc00::/7) e link-local (fe80::/10) — o equivalente das faixas
            // privadas em IPv6. `is_global` ainda não é estável.
            v6.is_loopback()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispositivo(id: i64, ip: Option<&str>) -> devices::Model {
        devices::Model {
            id,
            site_id: None,
            network_id: None,
            parent_id: None,
            ip_address: ip.map(str::to_owned),
            name: "equipamento".to_owned(),
            r#type: "router".to_owned(),
            vendor: None,
            model: None,
            serial_number: None,
            description: None,
            is_monitored: true,
            snmp_enabled: false,
            snmp_community: None,
            snmp_version: None,
            snmp_poll_interval_seconds: 15,
            access_mode: None,
            operating_system: None,
            system_key: None,
            status: "unknown".to_owned(),
            last_seen_at: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    fn contexto() -> AccessContext {
        AccessContext {
            peers: HashSet::new(),
            vpn_network_id: Some(7),
            vpn_cidr: Some("10.8.0.0/24".to_owned()),
            redes: vec![("Matriz".to_owned(), "192.168.1.0/24".to_owned())],
        }
    }

    #[test]
    fn a_rede_em_comum_exige_os_dois_enderecos_na_mesma_faixa() {
        let ctx = contexto();
        let dentro = ctx.rede_em_comum(
            "192.168.1.10".parse().unwrap(),
            "192.168.1.20".parse().unwrap(),
        );
        assert_eq!(dentro.map(|(nome, _)| nome.as_str()), Some("Matriz"));

        // Só um dos dois dentro não é rede em comum — é justamente o caso de
        // um equipamento de LAN e um endereço que não o alcança.
        assert!(ctx
            .rede_em_comum("192.168.1.10".parse().unwrap(), "10.0.0.5".parse().unwrap())
            .is_none());

        // A faixa do túnel não entra: quem está nela é VPN, e a dedução já
        // responde por esse caso.
        assert!(ctx
            .rede_em_comum("10.8.0.9".parse().unwrap(), "10.8.0.1".parse().unwrap())
            .is_none());

        // CIDR mal cadastrado não classifica nada, em vez de derrubar a tela.
        let mut quebrado = contexto();
        quebrado
            .redes
            .push(("Quebrada".to_owned(), "não é cidr".to_owned()));
        assert!(quebrado
            .rede_em_comum(
                "192.168.1.10".parse().unwrap(),
                "192.168.1.20".parse().unwrap()
            )
            .is_some());
    }

    #[test]
    fn a_declaracao_do_cadastro_vence_toda_evidencia() {
        // O caso que justifica a coluna: a filial atrás de outra VPN tem IP
        // privado e é indistinguível de um vizinho de LAN. Se a dedução
        // vencesse, declarar não serviria para nada.
        let mut device = dispositivo(1, Some("192.168.1.10"));
        device.access_mode = Some("remote".to_owned());
        let resolvido = contexto().resolve(&device);
        assert_eq!(resolvido.mode, AccessMode::Remote);
        assert!(resolvido.declared);
        assert!(
            resolvido.reason.contains("cadastro"),
            "{}",
            resolvido.reason
        );
    }

    #[test]
    fn declaracao_ilegivel_nao_derruba_a_deducao() {
        // Valor estranho no banco (importação, edição à mão) não pode fazer a
        // tela ficar sem resposta: a dedução continua valendo.
        let mut device = dispositivo(1, Some("192.168.1.10"));
        device.access_mode = Some("qualquer-coisa".to_owned());
        let resolvido = contexto().resolve(&device);
        assert_eq!(resolvido.mode, AccessMode::Local);
        assert!(!resolvido.declared);
    }

    #[test]
    fn peer_da_vpn_e_fato_e_nao_palpite() {
        let mut ctx = contexto();
        ctx.peers.insert(42);
        // IP de LAN e ainda assim VPN: o vínculo registrado vence a faixa.
        let resolvido = ctx.resolve(&dispositivo(42, Some("192.168.1.10")));
        assert_eq!(resolvido.mode, AccessMode::Vpn);
        assert!(resolvido.reason.contains("peer"), "{}", resolvido.reason);
    }

    #[test]
    fn o_ip_dentro_da_faixa_do_tunel_e_vpn() {
        let resolvido = contexto().resolve(&dispositivo(1, Some("10.8.0.9")));
        assert_eq!(resolvido.mode, AccessMode::Vpn);
        assert!(resolvido.reason.contains("10.8.0.0/24"));
    }

    #[test]
    fn a_rede_cadastrada_nomeia_o_motivo() {
        // "endereço privado" seria verdade e inútil; o nome da rede é o que
        // deixa o operador conferir se a conclusão bate com a realidade dele.
        let resolvido = contexto().resolve(&dispositivo(1, Some("192.168.1.10")));
        assert_eq!(resolvido.mode, AccessMode::Local);
        assert!(resolvido.reason.contains("Matriz"), "{}", resolvido.reason);
    }

    #[test]
    fn ip_publico_vira_acesso_remoto() {
        let resolvido = contexto().resolve(&dispositivo(1, Some("200.150.10.1")));
        assert_eq!(resolvido.mode, AccessMode::Remote);
    }

    #[test]
    fn a_faixa_de_cgnat_nao_e_internet() {
        // 100.64/10 é o que a operadora entrega em link residencial: o
        // equipamento ali não é alcançável de fora, então tratá-lo como remoto
        // mandaria o roteador para um endereço público que não responde.
        let resolvido = contexto().resolve(&dispositivo(1, Some("100.100.5.3")));
        assert_eq!(resolvido.mode, AccessMode::Local);
    }

    #[test]
    fn sem_ip_a_resposta_diz_que_e_suposicao() {
        let resolvido = contexto().resolve(&dispositivo(1, None));
        assert_eq!(resolvido.mode, AccessMode::Local);
        assert!(
            resolvido.reason.contains("assumida"),
            "{}",
            resolvido.reason
        );
    }

    #[test]
    fn o_vocabulario_aceito_e_o_que_a_mensagem_de_erro_promete() {
        assert_eq!(AccessMode::parse("auto"), Ok(None));
        assert_eq!(AccessMode::parse("  "), Ok(None));
        assert_eq!(AccessMode::parse("LOCAL"), Ok(Some(AccessMode::Local)));
        assert_eq!(AccessMode::parse("vpn"), Ok(Some(AccessMode::Vpn)));
        assert_eq!(AccessMode::parse("remote"), Ok(Some(AccessMode::Remote)));
        let erro = AccessMode::parse("nuvem").expect_err("devia recusar");
        for aceito in ["auto", "local", "vpn", "remote"] {
            assert!(
                erro.contains(aceito),
                "a mensagem não cita {aceito}: {erro}"
            );
        }
    }

    #[test]
    fn cada_forma_de_acesso_aponta_para_um_endereco_da_lista() {
        // Sem este vínculo, `access_mode` seria rótulo decorativo.
        assert_eq!(AccessMode::Local.address_kind(), "lan");
        assert_eq!(AccessMode::Vpn.address_kind(), "vpn");
        assert_eq!(AccessMode::Remote.address_kind(), "public");
    }
}
