//! Assinaturas que dizem que tipo de equipamento a descoberta encontrou.
//!
//! São tabelas, não código: reconhecer um aparelho novo é acrescentar uma
//! linha (aberto para extensão, fechado para modificação). Quem pontua com
//! elas é o [`super::device_identifier`]; os scanners leem daqui só o que
//! precisam perguntar à rede — as portas a testar e os serviços mDNS a pedir.
//!
//! O peso é a força da evidência, não uma probabilidade: uma porta de
//! impressão RAW (9100) vale mais que um "tv" solto num nome.

/// Tipos que a descoberta devolve — os ids de `devices::kinds::DEVICE_KINDS`
/// mais os dois de dúvida (`web_device`, `unknown`), que o Laya tenta resolver.
pub mod kind {
    pub const ROUTER: &str = "router";
    pub const SWITCH: &str = "switch";
    pub const FIREWALL: &str = "firewall";
    pub const AP: &str = "ap";
    pub const SERVER: &str = "server";
    pub const NAS: &str = "nas";
    pub const WORKSTATION: &str = "workstation";
    pub const MOBILE: &str = "mobile";
    pub const PRINTER: &str = "printer";
    pub const CAMERA: &str = "camera";
    pub const MEDIA: &str = "media";
    pub const IOT: &str = "iot";
    pub const VOIP: &str = "voip";
    pub const UPS: &str = "ups";
    pub const WEB_DEVICE: &str = "web_device";
    pub const UNKNOWN: &str = "unknown";
}

use kind::{
    AP, CAMERA, FIREWALL, IOT, MEDIA, MOBILE, NAS, PRINTER, ROUTER, SERVER, SWITCH, UPS, VOIP,
    WORKSTATION,
};

/// Uma porta TCP conhecida: o nome do serviço e, quando ele denuncia o
/// aparelho, o tipo e o peso.
pub struct PortRule {
    pub port: u16,
    pub label: &'static str,
    pub kind: Option<&'static str>,
    pub weight: i32,
}

const fn port(port: u16, label: &'static str, kind: &'static str, weight: i32) -> PortRule {
    PortRule {
        port,
        label,
        kind: Some(kind),
        weight,
    }
}

const fn service(port: u16, label: &'static str) -> PortRule {
    PortRule {
        port,
        label,
        kind: None,
        weight: 0,
    }
}

/// Portas testadas em cada host vivo. A lista **é** a tabela: não existe uma
/// segunda constante para esquecer de atualizar.
pub const PORT_RULES: &[PortRule] = &[
    service(21, "FTP"),
    port(22, "SSH", SERVER, 8),
    port(23, "Telnet", IOT, 6),
    port(25, "SMTP", SERVER, 20),
    port(53, "DNS", ROUTER, 15),
    service(80, "HTTP"),
    service(81, "HTTP alternativo"),
    port(88, "Kerberos", SERVER, 30),
    port(102, "Siemens S7", IOT, 35),
    port(139, "NetBIOS", WORKSTATION, 10),
    port(389, "LDAP", SERVER, 25),
    service(443, "HTTPS"),
    port(445, "SMB", WORKSTATION, 12),
    port(502, "Modbus", IOT, 35),
    port(515, "LPD", PRINTER, 30),
    port(548, "AFP", NAS, 20),
    port(554, "RTSP", CAMERA, 35),
    port(631, "IPP", PRINTER, 25),
    port(1400, "Sonos", MEDIA, 40),
    port(1433, "SQL Server", SERVER, 25),
    port(1723, "PPTP", ROUTER, 15),
    port(1883, "MQTT", IOT, 30),
    port(2049, "NFS", NAS, 20),
    port(3306, "MySQL", SERVER, 25),
    port(3389, "RDP", WORKSTATION, 25),
    port(5000, "UPnP / DSM", NAS, 10),
    port(5001, "Synology DSM", NAS, 18),
    port(5060, "SIP", VOIP, 35),
    port(5432, "PostgreSQL", SERVER, 25),
    port(5555, "ADB", MEDIA, 10),
    port(5900, "VNC", WORKSTATION, 15),
    port(6053, "ESPHome", IOT, 45),
    port(6379, "Redis", SERVER, 25),
    port(6668, "Tuya", IOT, 40),
    port(7547, "TR-069", ROUTER, 30),
    port(8000, "HTTP alternativo", CAMERA, 8),
    port(8001, "Samsung TV", MEDIA, 20),
    port(8008, "Google Cast", MEDIA, 30),
    port(8009, "Google Cast", MEDIA, 35),
    port(8060, "Roku", MEDIA, 45),
    service(8080, "HTTP alternativo"),
    port(8123, "Home Assistant", IOT, 35),
    port(8291, "Winbox (MikroTik)", ROUTER, 50),
    service(8443, "HTTPS alternativo"),
    port(8554, "RTSP alternativo", CAMERA, 25),
    port(8728, "API RouterOS", ROUTER, 40),
    port(8883, "MQTT/TLS", IOT, 20),
    port(9100, "Impressão RAW", PRINTER, 40),
    port(27017, "MongoDB", SERVER, 25),
    port(34567, "DVR (XMEye)", CAMERA, 40),
    port(37777, "Dahua", CAMERA, 45),
    service(49152, "UPnP"),
    port(62078, "iPhone/iPad", MOBILE, 45),
];

/// Portas que só provam que um host silencioso ao ping existe. Curta de
/// propósito: é testada em todo endereço que não respondeu ICMP nem ARP.
pub const LIVENESS_PORTS: &[u16] = &[
    80, 443, 22, 445, 139, 3389, 8080, 554, 53, 23, 8291, 62078, 9100, 7547, 8443, 5000,
];

/// Prova de vida quando a rede responde ping: quem não respondeu quase sempre
/// é Windows com firewall (445, 3389) ou um painel web. Seis portas por
/// endereço mudo mantêm a rajada pequena — um proxy de rede (Docker Desktop)
/// trava com milhares de SYN pendentes para IPs inexistentes.
pub const LIVENESS_PORTS_LIGHT: &[u16] = &[445, 3389, 80, 443, 22, 8080];

/// Portas em que se pede a página inicial, na ordem de preferência.
pub const WEB_PORTS: &[(u16, &str)] = &[
    (80, "http"),
    (8080, "http"),
    (443, "https"),
    (8443, "https"),
    (8000, "http"),
    (81, "http"),
    (5000, "http"),
    (5001, "https"),
    (8123, "http"),
];

#[must_use]
pub fn port_rule(port: u16) -> Option<&'static PortRule> {
    PORT_RULES.iter().find(|rule| rule.port == port)
}

/// O nome do serviço de uma porta, para mostrar ao operador.
#[must_use]
pub fn port_label(port: u16) -> Option<&'static str> {
    port_rule(port).map(|rule| rule.label)
}

#[must_use]
pub fn is_web_port(port: u16) -> bool {
    WEB_PORTS.iter().any(|(web, _)| *web == port)
}

/// Como uma palavra-chave casa com o texto já quebrado em palavras.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Match {
    /// Palavra (ou sequência de palavras) inteira: `ups` não casa com `groups`.
    Exact,
    /// Começo de palavra: `shelly` casa com `shelly1-abc`.
    Prefix,
}

pub struct KeywordRule {
    pub needle: &'static str,
    pub mode: Match,
    pub kind: &'static str,
    pub weight: i32,
}

const fn word(needle: &'static str, kind: &'static str, weight: i32) -> KeywordRule {
    KeywordRule {
        needle,
        mode: Match::Exact,
        kind,
        weight,
    }
}

const fn prefix(needle: &'static str, kind: &'static str, weight: i32) -> KeywordRule {
    KeywordRule {
        needle,
        mode: Match::Prefix,
        kind,
        weight,
    }
}

/// Palavras que denunciam o tipo em qualquer texto do host: nome, fabricante,
/// `sysDescr`, título da página web, descrição UPnP, nomes mDNS e NetBIOS.
///
/// Plataformas com adaptador (MikroTik, OpenWrt, Ubiquiti, Windows) não estão
/// aqui: quem as reconhece é o `device_type_hint` do adaptador.
pub const KEYWORD_RULES: &[KeywordRule] = &[
    // Roteadores, CPEs e ONTs.
    prefix("router", ROUTER, 35),
    prefix("roteador", ROUTER, 35),
    word("gateway", ROUTER, 25),
    word("internet gateway", ROUTER, 45),
    prefix("routerboard", ROUTER, 45),
    prefix("edgerouter", ROUTER, 50),
    prefix("fritz", ROUTER, 45),
    word("cpe", ROUTER, 25),
    word("ont", ROUTER, 30),
    word("onu", ROUTER, 30),
    prefix("gpon", ROUTER, 35),
    prefix("epon", ROUTER, 30),
    word("modem", ROUTER, 30),
    prefix("archer", ROUTER, 35),
    word("deco", ROUTER, 30),
    prefix("mercusys", ROUTER, 30),
    prefix("tenda", ROUTER, 25),
    prefix("draytek", ROUTER, 40),
    prefix("vigor", ROUTER, 30),
    prefix("linksys", ROUTER, 25),
    prefix("netgear", ROUTER, 15),
    prefix("zxhn", ROUTER, 40),
    prefix("hg8", ROUTER, 30),
    prefix("livebox", ROUTER, 40),
    prefix("pppoe", ROUTER, 20),
    prefix("vyos", ROUTER, 45),
    prefix("juniper", ROUTER, 25),
    prefix("ccr", ROUTER, 35),
    prefix("rb9", ROUTER, 30),
    prefix("rb7", ROUTER, 30),
    prefix("rb4", ROUTER, 30),
    prefix("rb3", ROUTER, 30),
    prefix("rb2", ROUTER, 30),
    prefix("rb1", ROUTER, 30),
    word("tp link", ROUTER, 15),
    word("d link", ROUTER, 10),
    prefix("asus", ROUTER, 10),
    word("zte", ROUTER, 20),
    prefix("huawei", ROUTER, 10),
    prefix("cisco", ROUTER, 15),
    // Fabricantes de CPE/ONT entregues pelas operadoras (nome do IEEE).
    prefix("sagemcom", ROUTER, 35),
    prefix("technicolor", ROUTER, 30),
    prefix("arcadyan", ROUTER, 35),
    prefix("askey", ROUTER, 30),
    prefix("sercomm", ROUTER, 30),
    prefix("zyxel", ROUTER, 30),
    prefix("fiberhome", ROUTER, 35),
    // Firewalls.
    word("firewall", FIREWALL, 40),
    prefix("pfsense", FIREWALL, 55),
    prefix("opnsense", FIREWALL, 55),
    prefix("fortigate", FIREWALL, 55),
    prefix("fortinet", FIREWALL, 45),
    prefix("sophos", FIREWALL, 45),
    prefix("sonicwall", FIREWALL, 50),
    prefix("watchguard", FIREWALL, 50),
    word("palo alto", FIREWALL, 45),
    prefix("checkpoint", FIREWALL, 40),
    prefix("untangle", FIREWALL, 45),
    prefix("ipfire", FIREWALL, 50),
    // Switches.
    prefix("switch", SWITCH, 35),
    prefix("catalyst", SWITCH, 45),
    prefix("procurve", SWITCH, 45),
    prefix("edgeswitch", SWITCH, 50),
    prefix("swos", SWITCH, 50),
    prefix("jetstream", SWITCH, 45),
    prefix("dgs", SWITCH, 35),
    prefix("sg350", SWITCH, 40),
    prefix("nexus", SWITCH, 30),
    prefix("arista", SWITCH, 45),
    prefix("comware", SWITCH, 40),
    prefix("usw", SWITCH, 40),
    prefix("crs", SWITCH, 30),
    prefix("css3", SWITCH, 30),
    // Access points e enlaces sem fio.
    word("access point", AP, 45),
    prefix("uap", AP, 45),
    prefix("unifi", AP, 25),
    prefix("eap", AP, 35),
    prefix("omada", AP, 25),
    prefix("aironet", AP, 50),
    prefix("ruckus", AP, 40),
    prefix("aruba", AP, 30),
    prefix("airos", AP, 40),
    prefix("airmax", AP, 40),
    prefix("nanostation", AP, 45),
    prefix("nanobeam", AP, 45),
    prefix("litebeam", AP, 45),
    prefix("powerbeam", AP, 45),
    prefix("cambium", AP, 35),
    prefix("mimosa", AP, 35),
    prefix("wap", AP, 30),
    word("ap", AP, 15),
    prefix("extender", AP, 30),
    prefix("repetidor", AP, 30),
    prefix("repeater", AP, 30),
    prefix("mesh", AP, 15),
    prefix("ubiquiti", AP, 20),
    // Servidores e virtualização.
    word("server", SERVER, 30),
    word("servidor", SERVER, 30),
    word("srv", SERVER, 25),
    prefix("vmware", SERVER, 40),
    prefix("esxi", SERVER, 50),
    prefix("proxmox", SERVER, 50),
    word("hyper v", SERVER, 40),
    word("windows server", SERVER, 50),
    prefix("ubuntu", SERVER, 20),
    prefix("debian", SERVER, 20),
    prefix("centos", SERVER, 25),
    prefix("rhel", SERVER, 25),
    prefix("almalinux", SERVER, 25),
    prefix("qemu", SERVER, 40),
    word("kvm", SERVER, 25),
    prefix("virtualbox", SERVER, 40),
    prefix("parallels", SERVER, 30),
    prefix("xenserver", SERVER, 40),
    prefix("docker", SERVER, 35),
    prefix("kubernetes", SERVER, 35),
    word("k8s", SERVER, 30),
    prefix("idrac", SERVER, 45),
    word("ilo", SERVER, 40),
    prefix("ipmi", SERVER, 40),
    prefix("supermicro", SERVER, 35),
    prefix("poweredge", SERVER, 45),
    prefix("proliant", SERVER, 45),
    prefix("zabbix", SERVER, 25),
    word("vm", SERVER, 15),
    prefix("raspberry", SERVER, 15),
    // Armazenamento.
    word("nas", NAS, 40),
    word("storage", NAS, 25),
    prefix("synology", NAS, 50),
    prefix("diskstation", NAS, 55),
    prefix("rackstation", NAS, 55),
    prefix("qnap", NAS, 50),
    prefix("truenas", NAS, 55),
    prefix("freenas", NAS, 55),
    prefix("unraid", NAS, 50),
    prefix("readynas", NAS, 55),
    word("my cloud", NAS, 40),
    prefix("mycloud", NAS, 40),
    prefix("openmediavault", NAS, 55),
    prefix("terramaster", NAS, 50),
    prefix("asustor", NAS, 50),
    prefix("drobo", NAS, 45),
    // Computadores.
    prefix("desktop", WORKSTATION, 35),
    prefix("laptop", WORKSTATION, 35),
    prefix("notebook", WORKSTATION, 35),
    prefix("workstation", WORKSTATION, 35),
    prefix("macbook", WORKSTATION, 50),
    word("imac", WORKSTATION, 50),
    word("mac mini", WORKSTATION, 45),
    prefix("macmini", WORKSTATION, 45),
    word("windows 10", WORKSTATION, 35),
    word("windows 11", WORKSTATION, 35),
    word("windows 7", WORKSTATION, 30),
    prefix("thinkpad", WORKSTATION, 45),
    prefix("thinkcentre", WORKSTATION, 45),
    prefix("optiplex", WORKSTATION, 45),
    prefix("latitude", WORKSTATION, 35),
    prefix("elitebook", WORKSTATION, 45),
    prefix("probook", WORKSTATION, 40),
    prefix("vostro", WORKSTATION, 40),
    prefix("inspiron", WORKSTATION, 40),
    prefix("ideapad", WORKSTATION, 40),
    prefix("chromebook", WORKSTATION, 45),
    word("pc", WORKSTATION, 25),
    prefix("lenovo", WORKSTATION, 15),
    word("intel", WORKSTATION, 12),
    prefix("dell", WORKSTATION, 10),
    prefix("realtek", WORKSTATION, 8),
    // Celulares e tablets.
    prefix("iphone", MOBILE, 60),
    prefix("ipad", MOBILE, 55),
    prefix("android", MOBILE, 45),
    prefix("galaxy", MOBILE, 40),
    prefix("redmi", MOBILE, 45),
    word("poco", MOBILE, 35),
    word("pixel", MOBILE, 30),
    prefix("oneplus", MOBILE, 45),
    prefix("motorola", MOBILE, 30),
    word("moto", MOBILE, 25),
    prefix("celular", MOBILE, 45),
    prefix("smartphone", MOBILE, 45),
    prefix("tablet", MOBILE, 40),
    prefix("apple", MOBILE, 15),
    prefix("oppo", MOBILE, 45),
    prefix("realme", MOBILE, 45),
    prefix("samsung", MOBILE, 15),
    // Smart TVs, consoles e mídia.
    word("smart tv", MEDIA, 50),
    word("tv", MEDIA, 30),
    word("tv box", MEDIA, 45),
    prefix("tvbox", MEDIA, 45),
    prefix("roku", MEDIA, 50),
    prefix("chromecast", MEDIA, 55),
    word("google tv", MEDIA, 50),
    word("android tv", MEDIA, 50),
    word("apple tv", MEDIA, 55),
    prefix("appletv", MEDIA, 55),
    word("fire tv", MEDIA, 50),
    prefix("firetv", MEDIA, 50),
    prefix("mibox", MEDIA, 50),
    prefix("bravia", MEDIA, 50),
    prefix("webos", MEDIA, 45),
    prefix("tizen", MEDIA, 40),
    prefix("sonos", MEDIA, 50),
    prefix("homepod", MEDIA, 50),
    prefix("playstation", MEDIA, 50),
    word("ps4", MEDIA, 40),
    word("ps5", MEDIA, 45),
    prefix("xbox", MEDIA, 50),
    prefix("nintendo", MEDIA, 50),
    prefix("mediarenderer", MEDIA, 35),
    prefix("dlna", MEDIA, 25),
    prefix("kodi", MEDIA, 40),
    prefix("plex", MEDIA, 30),
    prefix("soundbar", MEDIA, 45),
    prefix("denon", MEDIA, 35),
    prefix("marantz", MEDIA, 35),
    prefix("bose", MEDIA, 35),
    prefix("projetor", MEDIA, 35),
    prefix("projector", MEDIA, 35),
    prefix("lg electronics", MEDIA, 20),
    word("lg", MEDIA, 15),
    prefix("sony", MEDIA, 20),
    prefix("google", MEDIA, 15),
    // IoT, automação e industrial.
    word("iot", IOT, 45),
    word("esp", IOT, 35),
    prefix("esp32", IOT, 45),
    prefix("esp8266", IOT, 45),
    prefix("espressif", IOT, 40),
    prefix("tasmota", IOT, 60),
    prefix("esphome", IOT, 60),
    prefix("shelly", IOT, 60),
    prefix("sonoff", IOT, 55),
    prefix("tuya", IOT, 50),
    word("smart plug", IOT, 50),
    prefix("smartplug", IOT, 50),
    word("smart life", IOT, 45),
    word("philips hue", IOT, 55),
    word("philips lighting", IOT, 50),
    word("hue bridge", IOT, 55),
    prefix("signify", IOT, 45),
    prefix("ewelink", IOT, 50),
    prefix("zigbee", IOT, 45),
    prefix("zwave", IOT, 45),
    word("z wave", IOT, 45),
    word("home assistant", IOT, 55),
    prefix("homeassistant", IOT, 55),
    prefix("hassio", IOT, 55),
    prefix("hubitat", IOT, 55),
    prefix("smartthings", IOT, 55),
    prefix("nest", IOT, 35),
    word("google home", IOT, 50),
    word("nest hub", IOT, 50),
    word("echo", IOT, 35),
    prefix("alexa", IOT, 45),
    prefix("amazon", IOT, 20),
    prefix("xiaomi", IOT, 20),
    prefix("yeelight", IOT, 55),
    prefix("broadlink", IOT, 55),
    word("kasa", IOT, 45),
    word("tapo", IOT, 35),
    prefix("wled", IOT, 60),
    prefix("arduino", IOT, 45),
    word("mqtt", IOT, 30),
    prefix("thermostat", IOT, 45),
    prefix("termostato", IOT, 45),
    prefix("sensor", IOT, 35),
    prefix("lifx", IOT, 55),
    prefix("meross", IOT, 55),
    prefix("govee", IOT, 55),
    prefix("switchbot", IOT, 55),
    prefix("nanoleaf", IOT, 55),
    prefix("homekit", IOT, 35),
    prefix("roborock", IOT, 55),
    prefix("roomba", IOT, 55),
    prefix("ecovacs", IOT, 55),
    prefix("vacuum", IOT, 40),
    prefix("aspirador", IOT, 40),
    word("ar condicionado", IOT, 40),
    prefix("midea", IOT, 40),
    prefix("daikin", IOT, 40),
    prefix("lampada", IOT, 40),
    prefix("bulb", IOT, 40),
    prefix("tomada", IOT, 40),
    prefix("doorbell", IOT, 50),
    prefix("campainha", IOT, 45),
    prefix("fechadura", IOT, 45),
    prefix("alarme", IOT, 35),
    word("plc", IOT, 40),
    prefix("modbus", IOT, 40),
    prefix("zkteco", IOT, 45),
    prefix("controlid", IOT, 45),
    word("control id", IOT, 45),
    prefix("catraca", IOT, 45),
    prefix("raspberrypi", IOT, 20),
    // Quem fabrica módulos e acessórios IoT, como aparece no registro do IEEE.
    prefix("allterco", IOT, 55),
    prefix("aqara", IOT, 55),
    word("lumi united", IOT, 50),
    prefix("itead", IOT, 55),
    prefix("ecobee", IOT, 55),
    word("silicon laboratories", IOT, 25),
    word("nordic semiconductor", IOT, 30),
    prefix("murata", IOT, 20),
    word("texas instruments", IOT, 15),
    // Câmeras e gravadores.
    prefix("camera", CAMERA, 45),
    prefix("câmera", CAMERA, 45),
    prefix("ipcam", CAMERA, 50),
    word("ip camera", CAMERA, 50),
    prefix("hikvision", CAMERA, 55),
    prefix("dnvrs", CAMERA, 50),
    prefix("dahua", CAMERA, 55),
    prefix("onvif", CAMERA, 50),
    word("nvr", CAMERA, 50),
    word("dvr", CAMERA, 50),
    word("xvr", CAMERA, 50),
    prefix("mhdx", CAMERA, 45),
    word("vip", CAMERA, 25),
    prefix("reolink", CAMERA, 55),
    prefix("wyze", CAMERA, 45),
    prefix("amcrest", CAMERA, 55),
    prefix("uniview", CAMERA, 55),
    prefix("foscam", CAMERA, 55),
    prefix("imou", CAMERA, 50),
    prefix("ezviz", CAMERA, 55),
    prefix("yoosee", CAMERA, 55),
    prefix("webcam", CAMERA, 45),
    word("ipc", CAMERA, 40),
    prefix("cctv", CAMERA, 50),
    prefix("hanwha", CAMERA, 45),
    prefix("wisenet", CAMERA, 50),
    prefix("vivotek", CAMERA, 55),
    prefix("mobotix", CAMERA, 55),
    prefix("netsurveillance", CAMERA, 55),
    prefix("axis", CAMERA, 30),
    prefix("arlo", CAMERA, 50),
    // Impressoras.
    prefix("printer", PRINTER, 50),
    prefix("impressora", PRINTER, 50),
    prefix("laserjet", PRINTER, 55),
    prefix("officejet", PRINTER, 55),
    prefix("deskjet", PRINTER, 55),
    prefix("pagewide", PRINTER, 50),
    prefix("epson", PRINTER, 40),
    prefix("brother", PRINTER, 40),
    prefix("canon", PRINTER, 30),
    prefix("lexmark", PRINTER, 50),
    prefix("xerox", PRINTER, 50),
    prefix("ricoh", PRINTER, 45),
    prefix("kyocera", PRINTER, 50),
    prefix("imageclass", PRINTER, 55),
    prefix("imagerunner", PRINTER, 55),
    word("mfp", PRINTER, 45),
    word("print server", PRINTER, 45),
    prefix("ecotank", PRINTER, 55),
    prefix("pixma", PRINTER, 55),
    prefix("bizhub", PRINTER, 55),
    prefix("konica", PRINTER, 40),
    prefix("zebra", PRINTER, 35),
    prefix("bematech", PRINTER, 45),
    prefix("elgin", PRINTER, 30),
    prefix("jetdirect", PRINTER, 55),
    // Telefonia IP.
    word("voip", VOIP, 50),
    word("sip", VOIP, 30),
    word("ip phone", VOIP, 50),
    prefix("yealink", VOIP, 55),
    prefix("grandstream", VOIP, 55),
    prefix("polycom", VOIP, 55),
    prefix("snom", VOIP, 55),
    prefix("fanvil", VOIP, 55),
    prefix("gigaset", VOIP, 45),
    prefix("khomp", VOIP, 40),
    prefix("asterisk", VOIP, 40),
    prefix("issabel", VOIP, 45),
    prefix("freepbx", VOIP, 45),
    word("3cx", VOIP, 45),
    word("pabx", VOIP, 45),
    word("pbx", VOIP, 45),
    word("ata", VOIP, 30),
    word("tip", VOIP, 25),
    // Nobreaks e energia.
    word("ups", UPS, 45),
    word("smart ups", UPS, 55),
    prefix("nobreak", UPS, 55),
    word("apc", UPS, 35),
    prefix("eaton", UPS, 40),
    prefix("powerware", UPS, 50),
    word("pdu", UPS, 45),
    prefix("mppt", UPS, 55),
    word("controlador de carga", UPS, 55),
    prefix("inverter", UPS, 40),
    prefix("inversor", UPS, 40),
    prefix("solar", UPS, 30),
    prefix("ragtech", UPS, 55),
    prefix("cyberpower", UPS, 50),
    prefix("netagent", UPS, 45),
    prefix("riello", UPS, 45),
    prefix("liebert", UPS, 45),
    prefix("vertiv", UPS, 40),
];

/// Serviço anunciado por mDNS/Bonjour.
pub struct MdnsRule {
    pub service: &'static str,
    pub label: &'static str,
    pub kind: Option<&'static str>,
    pub weight: i32,
}

const fn mdns(
    service: &'static str,
    label: &'static str,
    kind: &'static str,
    weight: i32,
) -> MdnsRule {
    MdnsRule {
        service,
        label,
        kind: Some(kind),
        weight,
    }
}

const fn mdns_only(service: &'static str, label: &'static str) -> MdnsRule {
    MdnsRule {
        service,
        label,
        kind: None,
        weight: 0,
    }
}

/// Os serviços perguntados na rede **são** estes: a consulta mDNS sai daqui.
pub const MDNS_RULES: &[MdnsRule] = &[
    mdns("_ipp._tcp", "Impressão IPP", PRINTER, 45),
    mdns("_ipps._tcp", "Impressão IPPS", PRINTER, 45),
    mdns("_printer._tcp", "Impressão LPD", PRINTER, 45),
    mdns("_pdl-datastream._tcp", "Impressão RAW", PRINTER, 50),
    mdns("_uscan._tcp", "Digitalização", PRINTER, 35),
    mdns("_scanner._tcp", "Digitalização", PRINTER, 30),
    mdns("_googlecast._tcp", "Google Cast", MEDIA, 55),
    mdns("_airplay._tcp", "AirPlay", MEDIA, 35),
    mdns("_raop._tcp", "AirPlay áudio", MEDIA, 25),
    mdns("_spotify-connect._tcp", "Spotify Connect", MEDIA, 35),
    mdns("_sonos._tcp", "Sonos", MEDIA, 55),
    mdns("_androidtvremote2._tcp", "Android TV", MEDIA, 55),
    mdns("_amzn-wplay._tcp", "Fire TV", MEDIA, 45),
    mdns("_googlezone._tcp", "Google Home", MEDIA, 30),
    mdns("_sleep-proxy._udp", "Bonjour Sleep Proxy", MEDIA, 15),
    mdns("_hap._tcp", "HomeKit", IOT, 50),
    mdns("_homekit._tcp", "HomeKit", IOT, 40),
    mdns("_matter._tcp", "Matter", IOT, 50),
    mdns("_matterc._udp", "Matter", IOT, 50),
    mdns("_meshcop._udp", "Thread", IOT, 40),
    mdns("_hue._tcp", "Philips Hue", IOT, 55),
    mdns("_esphomelib._tcp", "ESPHome", IOT, 60),
    mdns("_shelly._tcp", "Shelly", IOT, 60),
    mdns("_mqtt._tcp", "MQTT", IOT, 30),
    mdns("_home-assistant._tcp", "Home Assistant", IOT, 55),
    mdns("_wled._tcp", "WLED", IOT, 60),
    mdns("_miio._udp", "Xiaomi Mi Home", IOT, 50),
    mdns("_ewelink._tcp", "eWeLink", IOT, 55),
    mdns("_nanoleafapi._tcp", "Nanoleaf", IOT, 55),
    mdns("_arduino._tcp", "Arduino", IOT, 50),
    mdns("_smb._tcp", "Compartilhamento SMB", WORKSTATION, 12),
    mdns("_afpovertcp._tcp", "Compartilhamento AFP", NAS, 25),
    mdns("_adisk._tcp", "Time Machine", NAS, 35),
    mdns("_nfs._tcp", "NFS", NAS, 20),
    mdns("_workstation._tcp", "Estação de trabalho", WORKSTATION, 25),
    mdns("_rdp._tcp", "Área de trabalho remota", WORKSTATION, 20),
    mdns("_rfb._tcp", "VNC", WORKSTATION, 15),
    mdns("_teamviewer._tcp", "TeamViewer", WORKSTATION, 20),
    mdns("_companion-link._tcp", "Apple Continuity", MOBILE, 25),
    mdns("_apple-mobdev2._tcp", "iPhone/iPad", MOBILE, 50),
    mdns("_rdlink._tcp", "Apple Remote", MOBILE, 25),
    mdns("_ssh._tcp", "SSH", SERVER, 8),
    mdns("_sftp-ssh._tcp", "SFTP", SERVER, 8),
    mdns("_rtsp._tcp", "RTSP", CAMERA, 30),
    mdns("_axis-video._tcp", "Câmera Axis", CAMERA, 60),
    mdns("_sip._udp", "SIP", VOIP, 30),
    mdns_only("_http._tcp", "Página web"),
    mdns_only("_https._tcp", "Página web segura"),
    mdns_only("_device-info._tcp", "Informações do aparelho"),
    mdns_only("_services._dns-sd._udp", "Catálogo de serviços"),
];

#[must_use]
pub fn mdns_rule(service: &str) -> Option<&'static MdnsRule> {
    MDNS_RULES.iter().find(|rule| rule.service == service)
}

/// Pedaço do tipo de dispositivo UPnP (`urn:...:device:<Tipo>:1`) ou do ST.
pub struct UpnpRule {
    pub needle: &'static str,
    pub kind: &'static str,
    pub weight: i32,
}

const fn upnp(needle: &'static str, kind: &'static str, weight: i32) -> UpnpRule {
    UpnpRule {
        needle,
        kind,
        weight,
    }
}

pub const UPNP_RULES: &[UpnpRule] = &[
    upnp("internetgatewaydevice", ROUTER, 55),
    upnp("wanconnectiondevice", ROUTER, 40),
    upnp("wandevice", ROUTER, 40),
    upnp("wanipconnection", ROUTER, 35),
    upnp("wfadevice", AP, 25),
    upnp("mediarenderer", MEDIA, 40),
    upnp("dial-multiscreen", MEDIA, 50),
    upnp("zoneplayer", MEDIA, 55),
    upnp("remotecontrolreceiver", MEDIA, 45),
    upnp("tvdevice", MEDIA, 45),
    upnp("roku:ecp", MEDIA, 55),
    upnp("mediaserver", NAS, 20),
    upnp("printer", PRINTER, 45),
    upnp("printbasic", PRINTER, 45),
    upnp("scanner", PRINTER, 25),
    upnp("networkcamera", CAMERA, 50),
    upnp("digitalsecuritycamera", CAMERA, 50),
    upnp("controllee", IOT, 50),
    upnp("lightswitch", IOT, 50),
    upnp("dimmablelight", IOT, 45),
    upnp("binarylight", IOT, 45),
    upnp("homeautomation", IOT, 45),
    upnp("hvac", IOT, 40),
];

/// Número de empresa IANA no `sysObjectID` (`1.3.6.1.4.1.<n>`).
pub struct Enterprise {
    pub prefix: &'static str,
    pub vendor: &'static str,
    pub kind: Option<&'static str>,
    pub weight: i32,
}

const fn enterprise(
    prefix: &'static str,
    vendor: &'static str,
    kind: &'static str,
    weight: i32,
) -> Enterprise {
    Enterprise {
        prefix,
        vendor,
        kind: Some(kind),
        weight,
    }
}

const fn enterprise_vendor(prefix: &'static str, vendor: &'static str) -> Enterprise {
    Enterprise {
        prefix,
        vendor,
        kind: None,
        weight: 0,
    }
}

pub const ENTERPRISES: &[Enterprise] = &[
    enterprise("1.3.6.1.4.1.14988", "MikroTik", ROUTER, 45),
    enterprise("1.3.6.1.4.1.9", "Cisco", SWITCH, 15),
    enterprise("1.3.6.1.4.1.41112", "Ubiquiti", AP, 30),
    enterprise("1.3.6.1.4.1.10002", "Ubiquiti", AP, 30),
    enterprise_vendor("1.3.6.1.4.1.4881", "Intelbras"),
    enterprise("1.3.6.1.4.1.311", "Microsoft", SERVER, 20),
    enterprise("1.3.6.1.4.1.17095", "Volt Tecnologia", UPS, 50),
    enterprise("1.3.6.1.4.1.318", "APC", UPS, 60),
    enterprise("1.3.6.1.4.1.534", "Eaton", UPS, 55),
    enterprise("1.3.6.1.4.1.11.2.3.9", "HP", PRINTER, 60),
    enterprise("1.3.6.1.4.1.2435", "Brother", PRINTER, 60),
    enterprise("1.3.6.1.4.1.1248", "Epson", PRINTER, 60),
    enterprise("1.3.6.1.4.1.1602", "Canon", PRINTER, 55),
    enterprise("1.3.6.1.4.1.641", "Lexmark", PRINTER, 60),
    enterprise("1.3.6.1.4.1.253", "Xerox", PRINTER, 60),
    enterprise("1.3.6.1.4.1.367", "Ricoh", PRINTER, 60),
    enterprise("1.3.6.1.4.1.1347", "Kyocera", PRINTER, 60),
    enterprise("1.3.6.1.4.1.6574", "Synology", NAS, 60),
    enterprise("1.3.6.1.4.1.24681", "QNAP", NAS, 60),
    enterprise("1.3.6.1.4.1.8072", "Net-SNMP", SERVER, 12),
    enterprise("1.3.6.1.4.1.6876", "VMware", SERVER, 50),
    enterprise("1.3.6.1.4.1.674", "Dell", SERVER, 30),
    enterprise("1.3.6.1.4.1.232", "HPE", SERVER, 30),
    enterprise("1.3.6.1.4.1.12356", "Fortinet", FIREWALL, 60),
    enterprise("1.3.6.1.4.1.25461", "Palo Alto Networks", FIREWALL, 60),
    enterprise("1.3.6.1.4.1.2620", "Check Point", FIREWALL, 60),
    enterprise("1.3.6.1.4.1.39165", "Hikvision", CAMERA, 60),
    enterprise("1.3.6.1.4.1.2636", "Juniper", ROUTER, 40),
    enterprise("1.3.6.1.4.1.2011", "Huawei", ROUTER, 15),
    enterprise("1.3.6.1.4.1.3902", "ZTE", ROUTER, 25),
    enterprise("1.3.6.1.4.1.25506", "H3C", SWITCH, 35),
    enterprise("1.3.6.1.4.1.171", "D-Link", SWITCH, 30),
    enterprise("1.3.6.1.4.1.11863", "TP-Link", SWITCH, 30),
    enterprise("1.3.6.1.4.1.4526", "Netgear", SWITCH, 30),
    enterprise("1.3.6.1.4.1.1916", "Extreme Networks", SWITCH, 40),
    enterprise("1.3.6.1.4.1.30065", "Arista", SWITCH, 50),
    enterprise("1.3.6.1.4.1.14823", "Aruba", AP, 40),
    enterprise("1.3.6.1.4.1.25053", "Ruckus", AP, 50),
    enterprise("1.3.6.1.4.1.8691", "Moxa", IOT, 45),
];

/// A empresa dona do `sysObjectID`, pelo prefixo mais longo (o HP de
/// impressora, `11.2.3.9`, vence o HP genérico). Prefixo só casa em fronteira
/// de número: `...1.9` não reivindica `...1.95`.
#[must_use]
pub fn enterprise_for(sys_object_id: &str) -> Option<&'static Enterprise> {
    let oid = sys_object_id.trim().trim_start_matches('.');
    ENTERPRISES
        .iter()
        .filter(|entry| {
            oid == entry.prefix
                || oid
                    .strip_prefix(entry.prefix)
                    .is_some_and(|rest| rest.starts_with('.'))
        })
        .max_by_key(|entry| entry.prefix.len())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn tabelas_nao_repetem_chave() {
        let mut ports = HashSet::new();
        assert!(PORT_RULES.iter().all(|rule| ports.insert(rule.port)));
        let mut services = HashSet::new();
        assert!(MDNS_RULES.iter().all(|rule| services.insert(rule.service)));
        let mut needles = HashSet::new();
        for rule in KEYWORD_RULES {
            assert!(
                needles.insert(rule.needle),
                "palavra repetida: {}",
                rule.needle
            );
            assert_eq!(rule.needle, rule.needle.to_lowercase(), "{}", rule.needle);
        }
    }

    /// Um tipo votado aqui que o cadastro não conhece viraria "Outro" na hora
    /// de adicionar o equipamento.
    #[test]
    fn todo_tipo_votado_existe_no_cadastro() {
        let known: HashSet<_> = crate::services::devices::kinds::DEVICE_KINDS
            .iter()
            .map(|kind| kind.id)
            .collect();
        let voted = PORT_RULES
            .iter()
            .filter_map(|rule| rule.kind)
            .chain(KEYWORD_RULES.iter().map(|rule| rule.kind))
            .chain(MDNS_RULES.iter().filter_map(|rule| rule.kind))
            .chain(UPNP_RULES.iter().map(|rule| rule.kind))
            .chain(ENTERPRISES.iter().filter_map(|entry| entry.kind));
        for kind in voted {
            assert!(known.contains(kind), "tipo fora do cadastro: {kind}");
        }
    }

    #[test]
    fn portas_de_vida_e_web_sao_testadas_no_enriquecimento() {
        for port in LIVENESS_PORTS
            .iter()
            .chain(LIVENESS_PORTS_LIGHT)
            .chain(WEB_PORTS.iter().map(|(port, _)| port))
        {
            assert!(port_rule(*port).is_some(), "porta {port} fora da tabela");
        }
    }

    #[test]
    fn empresa_casa_pelo_prefixo_mais_longo_e_na_fronteira() {
        assert_eq!(
            enterprise_for(".1.3.6.1.4.1.11.2.3.9.1").map(|e| e.kind),
            Some(Some(PRINTER))
        );
        assert_eq!(
            enterprise_for("1.3.6.1.4.1.14988.1").map(|e| e.vendor),
            Some("MikroTik")
        );
        assert_eq!(
            enterprise_for("1.3.6.1.4.1.9.1.1").map(|e| e.vendor),
            Some("Cisco")
        );
        assert!(enterprise_for("1.3.6.1.4.1.95.1").is_none());
    }
}
