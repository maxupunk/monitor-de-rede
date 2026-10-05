//! Classificação por pontuação do que a descoberta encontrou.
//!
//! Cada evidência — porta aberta, palavra no nome, serviço mDNS, tipo UPnP,
//! `sysObjectID`, plataforma de um adaptador — vota num tipo com o peso de
//! [`super::fingerprints`]. Vence o tipo mais votado; a confiança cresce com a
//! vantagem sobre o segundo, e os votos vencedores viram a explicação que o
//! operador lê ("por que achamos que é uma câmera").
//!
//! Conhecimento de plataforma (MikroTik, OpenWrt, Ubiquiti, Windows) continua
//! nos adapters (`adapters::registry`); aqui só se soma o voto deles.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::{
    fingerprints::{self, kind, Match},
    merger::DiscoveredHost,
};
use crate::services::{devices::adapters::registry, vendors::builtin};

/// Pontuação mínima para afirmar um tipo; abaixo disso a resposta é dúvida.
const MIN_SCORE: i32 = 18;
/// Explicações guardadas por host — as mais fortes.
const MAX_REASONS: usize = 4;
/// Peso do voto de um adaptador de plataforma.
const ADAPTER_WEIGHT: i32 = 45;
/// O gateway cadastrado da rede é, por definição, o roteador dela.
const GATEWAY_WEIGHT: i32 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Classification {
    pub device_type: &'static str,
    pub confidence: i32,
    pub reasons: Vec<String>,
    /// Segundo colocado, quando ficou perto do vencedor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternative: Option<&'static str>,
}

/// De onde veio um texto, e quanto ele merece de crédito.
#[derive(Clone, Copy)]
enum Source {
    Snmp,
    Upnp,
    Web,
    Mdns,
    Name,
    Netbios,
    Vendor,
}

impl Source {
    const fn label(self) -> &'static str {
        match self {
            Self::Snmp => "SNMP",
            Self::Upnp => "Descrição UPnP",
            Self::Web => "Página web",
            Self::Mdns => "Anúncio mDNS",
            Self::Name => "Nome",
            Self::Netbios => "Nome NetBIOS",
            Self::Vendor => "Fabricante",
        }
    }

    /// Percentual aplicado ao peso da palavra: o `sysDescr` foi escrito pelo
    /// fabricante; um fabricante de placa de rede é só um indício.
    const fn credit(self) -> i32 {
        match self {
            Self::Snmp => 130,
            Self::Upnp => 120,
            Self::Web | Self::Mdns => 110,
            Self::Name | Self::Netbios => 100,
            Self::Vendor => 80,
        }
    }
}

/// Texto quebrado em palavras minúsculas, cercado de espaços para casar
/// palavra inteira (`" ups "`) ou começo de palavra (`" shelly"`).
struct Words(String);

impl Words {
    fn new(text: &str) -> Self {
        let words: Vec<String> = text
            .split(|char: char| !char.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_lowercase)
            .collect();
        Self(format!(" {} ", words.join(" ")))
    }

    fn matches(&self, needle: &str, mode: Match) -> bool {
        match mode {
            Match::Exact => self.0.contains(&format!(" {needle} ")),
            Match::Prefix => self.0.contains(&format!(" {needle}")),
        }
    }
}

/// Votos por tipo. Um mesmo indício (a mesma palavra em duas fontes) conta
/// uma vez, com o maior peso.
#[derive(Default)]
struct Ballot {
    votes: BTreeMap<&'static str, BTreeMap<String, (i32, String)>>,
}

impl Ballot {
    fn vote(&mut self, kind: &'static str, key: &str, weight: i32, reason: String) {
        if weight <= 0 {
            return;
        }
        let entry = self
            .votes
            .entry(kind)
            .or_default()
            .entry(key.to_string())
            .or_insert((0, String::new()));
        if weight > entry.0 {
            *entry = (weight, reason);
        }
    }

    /// Soma com retorno decrescente: o segundo indício vale metade, o
    /// terceiro um quarto — dez palavras fracas não batem uma porta forte.
    fn ranked(self) -> Vec<(&'static str, i32, Vec<String>)> {
        let mut ranked: Vec<_> = self
            .votes
            .into_iter()
            .map(|(kind, entries)| {
                let mut entries: Vec<_> = entries.into_values().collect();
                entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
                let score: i32 = entries
                    .iter()
                    .enumerate()
                    .map(|(index, (weight, _))| weight >> index.min(4))
                    .sum();
                let reasons = entries
                    .into_iter()
                    .take(MAX_REASONS)
                    .map(|(_, reason)| reason)
                    .collect();
                (kind, score, reasons)
            })
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        ranked
    }
}

fn text<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value, |current, key| current.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn strings<'a>(value: &'a Value, path: &[&str]) -> Vec<&'a str> {
    path.iter()
        .try_fold(value, |current, key| current.get(key))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Os textos do host, cada um com a sua fonte.
fn texts<'a>(host: &'a DiscoveredHost) -> Vec<(Source, &'a str)> {
    let data = &host.data;
    let mut texts = Vec::new();
    let mut push = |source: Source, value: Option<&'a str>| {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            texts.push((source, value));
        }
    };
    push(Source::Name, host.hostname.as_deref());
    push(Source::Name, host.mdns_name.as_deref());
    push(Source::Vendor, host.vendor.as_deref());
    for key in [
        "sysDescr",
        "sysName",
        "label",
        "hardwareVendor",
        "hardwareModel",
    ] {
        push(Source::Snmp, text(data, &["identity", key]));
    }
    for key in ["server", "title", "realm", "location"] {
        push(Source::Web, text(data, &["http", key]));
    }
    push(Source::Upnp, text(data, &["server"]));
    for key in [
        "friendlyName",
        "manufacturer",
        "modelName",
        "modelDescription",
    ] {
        push(Source::Upnp, text(data, &["ssdp", key]));
    }
    push(Source::Mdns, text(data, &["mdns", "model"]));
    for instance in strings(data, &["mdns", "instances"]) {
        push(Source::Mdns, Some(instance));
    }
    push(Source::Netbios, text(data, &["netbios", "name"]));
    texts
}

fn vote_keywords(ballot: &mut Ballot, texts: &[(Source, &str)]) {
    for (source, value) in texts {
        let words = Words::new(value);
        for rule in fingerprints::KEYWORD_RULES {
            if words.matches(rule.needle, rule.mode) {
                ballot.vote(
                    rule.kind,
                    rule.needle,
                    rule.weight * source.credit() / 100,
                    format!("{} menciona \"{}\"", source.label(), rule.needle),
                );
            }
        }
    }
}

fn vote_adapters(ballot: &mut Ballot, texts: &[(Source, &str)]) {
    let corpus = texts
        .iter()
        .map(|(_, value)| value.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    for adapter in registry::all() {
        if let Some(hint) = adapter.device_type_hint(&corpus) {
            let label = adapter.platform().label;
            ballot.vote(
                canonical(hint),
                adapter.platform().id,
                ADAPTER_WEIGHT,
                format!("Plataforma {label} reconhecida"),
            );
        }
    }
}

fn vote_ports(ballot: &mut Ballot, ports: &[u16]) {
    for port in ports {
        if let Some(rule) = fingerprints::port_rule(*port) {
            if let Some(kind) = rule.kind {
                ballot.vote(
                    kind,
                    &format!("port:{port}"),
                    rule.weight,
                    format!("Porta {port} aberta ({})", rule.label),
                );
            }
        }
    }
}

fn vote_services(ballot: &mut Ballot, data: &Value) {
    for service in strings(data, &["mdns", "services"]) {
        if let Some(rule) = fingerprints::mdns_rule(service) {
            if let Some(kind) = rule.kind {
                ballot.vote(
                    kind,
                    service,
                    rule.weight,
                    format!("Anuncia {} por mDNS", rule.label),
                );
            }
        }
    }
    let upnp_types: Vec<String> = strings(data, &["ssdp", "types"])
        .into_iter()
        .chain(text(data, &["ssdp", "deviceType"]))
        .map(str::to_ascii_lowercase)
        .collect();
    for upnp_type in &upnp_types {
        for rule in fingerprints::UPNP_RULES {
            if upnp_type.contains(rule.needle) {
                ballot.vote(
                    rule.kind,
                    rule.needle,
                    rule.weight,
                    format!("Anuncia o tipo UPnP \"{}\"", rule.needle),
                );
            }
        }
    }
}

fn vote_snmp(ballot: &mut Ballot, data: &Value) {
    let Some(oid) = text(data, &["identity", "sysObjectId"]) else {
        return;
    };
    if let Some(entry) = fingerprints::enterprise_for(oid) {
        if let Some(kind) = entry.kind {
            ballot.vote(
                kind,
                entry.prefix,
                entry.weight,
                format!("SNMP identifica o fabricante {}", entry.vendor),
            );
        }
    }
}

fn vote_link_layer(ballot: &mut Ballot, host: &DiscoveredHost) {
    if text(&host.data, &["netbios", "name"]).is_some() {
        ballot.vote(
            kind::WORKSTATION,
            "netbios",
            20,
            "Responde NetBIOS (Windows/Samba)".into(),
        );
    }
    let Some(mac) = host.mac_address.as_deref() else {
        return;
    };
    if builtin::is_docker(mac) {
        ballot.vote(
            kind::SERVER,
            "docker-mac",
            35,
            "MAC de contêiner Docker".into(),
        );
    } else if builtin::is_locally_administered(mac) {
        ballot.vote(
            kind::MOBILE,
            "private-mac",
            25,
            "MAC aleatório de privacidade (celular ou notebook)".into(),
        );
        ballot.vote(
            kind::WORKSTATION,
            "private-mac",
            10,
            "MAC aleatório de privacidade (celular ou notebook)".into(),
        );
    }
}

/// Os adapters ainda falam `access_point`; o cadastro grava `ap`.
fn canonical(kind_id: &'static str) -> &'static str {
    match kind_id {
        "access_point" => kind::AP,
        other => other,
    }
}

/// Classifica um host com tudo o que a descoberta juntou. `gateway` é o
/// gateway cadastrado da rede varrida, quando há.
#[must_use]
pub fn classify(host: &DiscoveredHost, gateway: Option<&str>) -> Classification {
    let ranked = ballot_for(host, gateway).ranked();
    decide(host, &ranked)
}

/// Pontos da heurística para cada tipo que recebeu algum voto, do mais
/// votado ao menos. É o que permite conferir um palpite de fora (o do Laya)
/// contra a evidência que existe.
#[must_use]
pub fn scores(host: &DiscoveredHost, gateway: Option<&str>) -> Vec<(&'static str, i32)> {
    ballot_for(host, gateway)
        .ranked()
        .into_iter()
        .map(|(kind, score, _)| (kind, score))
        .collect()
}

fn ballot_for(host: &DiscoveredHost, gateway: Option<&str>) -> Ballot {
    let texts = texts(host);
    let mut ballot = Ballot::default();
    vote_adapters(&mut ballot, &texts);
    vote_keywords(&mut ballot, &texts);
    vote_ports(&mut ballot, &host.open_ports);
    vote_services(&mut ballot, &host.data);
    vote_snmp(&mut ballot, &host.data);
    vote_link_layer(&mut ballot, host);
    if gateway.is_some_and(|gateway| gateway == host.ip_address) {
        ballot.vote(
            kind::ROUTER,
            "gateway",
            GATEWAY_WEIGHT,
            "É o gateway cadastrado da rede".into(),
        );
    }
    ballot
}

fn decide(host: &DiscoveredHost, ranked: &[(&'static str, i32, Vec<String>)]) -> Classification {
    match ranked.first() {
        Some((device_type, score, reasons)) if *score >= MIN_SCORE => {
            let runner_up = ranked.get(1);
            let second = runner_up.map_or(0, |(_, score, _)| *score);
            Classification {
                device_type,
                confidence: (100 * score / (score + second + 25)).clamp(20, 98),
                reasons: reasons.clone(),
                alternative: runner_up
                    .filter(|(_, score_two, _)| score_two * 2 >= *score)
                    .map(|(kind, _, _)| *kind),
            }
        }
        _ if host
            .open_ports
            .iter()
            .any(|port| fingerprints::is_web_port(*port)) =>
        {
            Classification {
                device_type: kind::WEB_DEVICE,
                confidence: 25,
                reasons: vec!["Só há uma página web para identificar o aparelho".into()],
                alternative: None,
            }
        }
        _ => Classification {
            device_type: kind::UNKNOWN,
            confidence: 10,
            reasons: Vec::new(),
            alternative: None,
        },
    }
}

/// O tipo que o fabricante (e o próprio MAC) sugere sozinho — o palpite do
/// cadastro de dispositivo enquanto o operador digita o MAC. `None` quando a
/// evidência não basta para afirmar um tipo.
#[must_use]
pub fn hint_from_mac(mac_address: &str, vendor: Option<&str>) -> Option<Classification> {
    let host = DiscoveredHost {
        mac_address: Some(mac_address.to_string()),
        vendor: vendor.map(str::to_string),
        ..DiscoveredHost::default()
    };
    let classification = classify(&host, None);
    (classification.device_type != kind::UNKNOWN && classification.device_type != kind::WEB_DEVICE)
        .then_some(classification)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn host(
        hostname: Option<&str>,
        vendor: Option<&str>,
        ports: &[u16],
        data: Value,
    ) -> DiscoveredHost {
        DiscoveredHost {
            ip_address: "10.0.0.9".into(),
            hostname: hostname.map(str::to_string),
            vendor: vendor.map(str::to_string),
            open_ports: ports.to_vec(),
            data,
            ..DiscoveredHost::default()
        }
    }

    fn kind_of(host: &DiscoveredHost) -> &'static str {
        classify(host, None).device_type
    }

    #[test]
    fn plataformas_sao_classificadas_pelos_adapters() {
        assert_eq!(
            kind_of(&host(None, Some("MikroTik"), &[], json!({}))),
            "router"
        );
        assert_eq!(
            kind_of(&host(Some("OpenWrt AP"), None, &[], json!({}))),
            "router"
        );
        assert_eq!(kind_of(&host(Some("UniFi AP"), None, &[], json!({}))), "ap");
        assert_eq!(
            kind_of(&host(None, Some("Microsoft Windows"), &[], json!({}))),
            "server"
        );
    }

    #[test]
    fn identifica_iot_por_fabricante_nome_e_servico() {
        let esp = host(Some("ESP_3A4B5C"), Some("Espressif"), &[80], json!({}));
        assert_eq!(kind_of(&esp), "iot");
        let shelly = host(
            None,
            None,
            &[80],
            json!({ "mdns": { "services": ["_shelly._tcp", "_http._tcp"] } }),
        );
        let classification = classify(&shelly, None);
        assert_eq!(classification.device_type, "iot");
        assert!(classification.reasons[0].contains("Shelly"));
    }

    #[test]
    fn identifica_tv_impressora_e_celular() {
        let tv = host(
            None,
            Some("LG Electronics"),
            &[],
            json!({ "ssdp": { "types": ["urn:schemas-upnp-org:device:MediaRenderer:1"] } }),
        );
        assert_eq!(kind_of(&tv), "media");
        assert_eq!(
            kind_of(&host(None, None, &[80, 9100, 631], json!({}))),
            "printer"
        );
        assert_eq!(
            kind_of(&host(Some("iPhone-de-Ana"), None, &[62078], json!({}))),
            "mobile"
        );
    }

    #[test]
    fn roteador_por_upnp_e_por_ser_gateway() {
        let igd = host(
            None,
            None,
            &[80, 53],
            json!({ "ssdp": { "types": ["urn:schemas-upnp-org:device:InternetGatewayDevice:1"] } }),
        );
        assert_eq!(kind_of(&igd), "router");
        let bare = host(None, None, &[80], json!({}));
        assert_eq!(kind_of(&bare), "web_device");
        assert_eq!(classify(&bare, Some("10.0.0.9")).device_type, "router");
    }

    #[test]
    fn snmp_e_texto_do_fabricante_pesam() {
        let volt = host(
            Some("Volt"),
            Some("Volt Tecnologia"),
            &[],
            json!({ "identity": { "sysDescr": "Controlador de Carga MPPT 12V/24V/48V-30A embedded" } }),
        );
        assert_eq!(kind_of(&volt), "ups");
        let mikrotik = host(
            Some("HeartOfGold"),
            Some("MikroTik"),
            &[22, 80, 443],
            json!({ "identity": { "sysDescr": "Linux RB922-terraco 6.12.94 #0 Mon Jun 29 12:59:20 2026 mips" } }),
        );
        assert_eq!(kind_of(&mikrotik), "router");
        let printer = host(
            None,
            None,
            &[80],
            json!({ "identity": { "sysObjectId": "1.3.6.1.4.1.11.2.3.9.1" } }),
        );
        assert_eq!(kind_of(&printer), "printer");
    }

    #[test]
    fn camera_por_porta_rtsp_e_palavra_inteira() {
        assert_eq!(kind_of(&host(None, None, &[80, 554], json!({}))), "camera");
        // "ups" não pode casar dentro de "groups".
        assert_eq!(
            kind_of(&host(Some("groups"), None, &[], json!({}))),
            "unknown"
        );
    }

    #[test]
    fn confianca_cai_quando_a_evidencia_se_divide() {
        let clear = classify(
            &host(None, Some("Hikvision"), &[554, 8000], json!({})),
            None,
        );
        let split = classify(&host(Some("tv"), None, &[9100], json!({})), None);
        assert!(clear.confidence > split.confidence);
        assert!(split.alternative.is_some());
    }

    /// O caso real: OpenWrt numa Banana Pi R3, agente Net-SNMP, sem "openwrt"
    /// escrito em lugar nenhum — o Laya chegou a chamar de NAS com 98%.
    #[test]
    fn openwrt_pelo_kernel_vira_roteador() {
        let bpi = host(
            Some("HeartOfGold"),
            Some("Net-SNMP"),
            &[22, 80, 443],
            json!({ "identity": {
                "sysDescr": "Linux bpi-r3-assistencia 6.12.94 #0 SMP Mon Jun 29 12:59:20 2026 aarch64",
                "sysObjectId": "1.3.6.1.4.1.8072.3.2.10",
            } }),
        );
        let classification = classify(&bpi, None);
        assert_eq!(classification.device_type, "router");
        assert!(
            classification
                .reasons
                .iter()
                .any(|reason| reason.contains("OpenWrt")),
            "{:?}",
            classification.reasons
        );
    }

    #[test]
    fn fabricante_sozinho_sugere_tipo_so_quando_basta() {
        let iot = hint_from_mac("5c:cf:7f:00:11:22", Some("Espressif")).unwrap();
        assert_eq!(iot.device_type, "iot");
        assert!(iot.reasons[0].contains("espressif"));
        assert!(hint_from_mac("00:11:22:33:44:55", Some("Fabricante Genérico")).is_none());
    }

    #[test]
    fn mac_aleatorio_sugere_celular() {
        let mut phone = host(None, None, &[], json!({}));
        phone.mac_address = Some("da:a1:19:00:11:22".into());
        assert_eq!(kind_of(&phone), "mobile");
    }

    #[test]
    fn fabricantes_comuns_de_iot_sao_classificados_corretamente() {
        let hiflying = host(
            None,
            Some("Shanghai High-Flying Electronics Technology"),
            &[],
            json!({}),
        );
        assert_eq!(kind_of(&hiflying), "iot");

        let fnlink = host(None, Some("HUNAN FN-LINK TECHNOLOGY"), &[], json!({}));
        assert_eq!(kind_of(&fnlink), "iot");

        let millennial = host(None, Some("Millennial Net"), &[], json!({}));
        assert_eq!(kind_of(&millennial), "iot");
    }

    #[test]
    fn cameras_com_ouis_clonados_sao_classificadas_como_camera() {
        let motion = host(None, Some("Motion Control Systems"), &[], json!({}));
        assert_eq!(kind_of(&motion), "camera");

        let tokki = host(None, Some("JRC TOKKI"), &[], json!({}));
        assert_eq!(kind_of(&tokki), "camera");
    }
}
