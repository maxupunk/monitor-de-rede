//! Adapters concretos das plataformas suportadas.

use syslog_loose::SyslogSeverity;

use super::contract::{
    DeviceAccessMethod, DeviceAdapter, DevicePlatform, SyslogConfigurationAdapter,
};

pub struct RouterOsAdapter;
pub struct OpenWrtAdapter;
pub struct UbiquitiAdapter;
pub struct LinuxAdapter;
pub struct WindowsAdapter;
pub struct MobileAdapter;
pub struct OtherAdapter;

struct RouterOsSyslogAdapter;
struct OpenWrtSyslogAdapter;
struct LinuxSyslogAdapter;
struct UbiquitiSyslogAdapter;

static ROUTEROS_SYSLOG: RouterOsSyslogAdapter = RouterOsSyslogAdapter;
static OPENWRT_SYSLOG: OpenWrtSyslogAdapter = OpenWrtSyslogAdapter;
static LINUX_SYSLOG: LinuxSyslogAdapter = LinuxSyslogAdapter;
static UBIQUITI_SYSLOG: UbiquitiSyslogAdapter = UbiquitiSyslogAdapter;

pub static ROUTEROS: RouterOsAdapter = RouterOsAdapter;
pub static OPENWRT: OpenWrtAdapter = OpenWrtAdapter;
pub static UBIQUITI: UbiquitiAdapter = UbiquitiAdapter;
pub static LINUX: LinuxAdapter = LinuxAdapter;
pub static WINDOWS: WindowsAdapter = WindowsAdapter;
pub static MOBILE: MobileAdapter = MobileAdapter;
pub static OTHER: OtherAdapter = OtherAdapter;

static ROUTEROS_PLATFORM: DevicePlatform = DevicePlatform {
    id: "routeros",
    label: "MikroTik RouterOS",
    icon: "mdi-router-network",
    generic: false,
};
static OPENWRT_PLATFORM: DevicePlatform = DevicePlatform {
    id: "openwrt",
    label: "OpenWrt",
    icon: "mdi-router-wireless",
    generic: false,
};
static UBIQUITI_PLATFORM: DevicePlatform = DevicePlatform {
    id: "ubiquiti",
    label: "Ubiquiti EdgeOS / UniFi",
    icon: "mdi-router",
    generic: false,
};
static LINUX_PLATFORM: DevicePlatform = DevicePlatform {
    id: "linux",
    label: "Linux",
    icon: "mdi-linux",
    generic: true,
};
static WINDOWS_PLATFORM: DevicePlatform = DevicePlatform {
    id: "windows",
    label: "Windows",
    icon: "mdi-microsoft-windows",
    generic: false,
};
static MOBILE_PLATFORM: DevicePlatform = DevicePlatform {
    id: "mobile",
    label: "Celular (Android / iOS)",
    icon: "mdi-cellphone",
    generic: false,
};
static OTHER_PLATFORM: DevicePlatform = DevicePlatform {
    id: "other",
    label: "Outro sistema",
    icon: "mdi-help-circle-outline",
    generic: false,
};

impl DeviceAdapter for RouterOsAdapter {
    fn platform(&self) -> &'static DevicePlatform {
        &ROUTEROS_PLATFORM
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["mikrotik", "routeros", "routerboard"]
    }

    fn sys_object_ids(&self) -> &'static [&'static str] {
        &["1.3.6.1.4.1.14988"]
    }

    fn ssh_banners(&self) -> &'static [&'static str] {
        &["rosssh", "mikrotik"]
    }

    fn supports_access(&self, method: DeviceAccessMethod) -> bool {
        matches!(method, DeviceAccessMethod::MacTelnet)
    }

    fn syslog(&self) -> Option<&'static dyn SyslogConfigurationAdapter> {
        Some(&ROUTEROS_SYSLOG)
    }

    fn vpn_profile(&self) -> Option<&'static str> {
        Some("mikrotik")
    }

    fn device_type_hint(&self, evidence: &str) -> Option<&'static str> {
        self.aliases()
            .iter()
            .any(|alias| evidence.contains(alias))
            .then_some("router")
    }

    fn default_device_type(&self) -> &'static str {
        "router"
    }
}

impl DeviceAdapter for OpenWrtAdapter {
    fn platform(&self) -> &'static DevicePlatform {
        &OPENWRT_PLATFORM
    }

    fn aliases(&self) -> &'static [&'static str] {
        &[
            "openwrt", "lede", "dd-wrt", "gargoyle", "padavan", "gl.inet", "gl-inet", "glinet",
            "turris", "luci",
        ]
    }

    fn ssh_banners(&self) -> &'static [&'static str] {
        &["dropbear"]
    }

    fn supports_access(&self, method: DeviceAccessMethod) -> bool {
        matches!(method, DeviceAccessMethod::MacTelnet)
    }

    fn syslog(&self) -> Option<&'static dyn SyslogConfigurationAdapter> {
        Some(&OPENWRT_SYSLOG)
    }

    fn vpn_profile(&self) -> Option<&'static str> {
        Some("openwrt")
    }

    fn device_type_hint(&self, evidence: &str) -> Option<&'static str> {
        self.aliases()
            .iter()
            .any(|alias| evidence.contains(alias))
            .then_some("router")
    }

    fn default_device_type(&self) -> &'static str {
        "router"
    }
}

impl DeviceAdapter for UbiquitiAdapter {
    fn platform(&self) -> &'static DevicePlatform {
        &UBIQUITI_PLATFORM
    }

    fn aliases(&self) -> &'static [&'static str] {
        &[
            "ubiquiti",
            "edgeos",
            "edgerouter",
            "edgeswitch",
            "unifi",
            "vyatta",
        ]
    }

    fn sys_object_ids(&self) -> &'static [&'static str] {
        &["1.3.6.1.4.1.41112", "1.3.6.1.4.1.10002"]
    }

    fn syslog(&self) -> Option<&'static dyn SyslogConfigurationAdapter> {
        Some(&UBIQUITI_SYSLOG)
    }

    fn device_type_hint(&self, evidence: &str) -> Option<&'static str> {
        (evidence.contains("unifi") || evidence.contains("wireless")).then_some("access_point")
    }
}

impl DeviceAdapter for LinuxAdapter {
    fn platform(&self) -> &'static DevicePlatform {
        &LINUX_PLATFORM
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["debian", "ubuntu", "rsyslog", "linux"]
    }

    fn syslog(&self) -> Option<&'static dyn SyslogConfigurationAdapter> {
        Some(&LINUX_SYSLOG)
    }

    fn vpn_profile(&self) -> Option<&'static str> {
        Some("linux")
    }

    fn is_system_description(&self, value: &str) -> bool {
        let normalized = value.to_ascii_lowercase();
        normalized.starts_with("linux ") || normalized.contains(" kernel ")
    }
}

impl DeviceAdapter for WindowsAdapter {
    fn platform(&self) -> &'static DevicePlatform {
        &WINDOWS_PLATFORM
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["windows", "microsoft"]
    }

    fn sys_object_ids(&self) -> &'static [&'static str] {
        &["1.3.6.1.4.1.311"]
    }

    fn vpn_profile(&self) -> Option<&'static str> {
        Some("windows")
    }

    fn device_type_hint(&self, evidence: &str) -> Option<&'static str> {
        self.aliases()
            .iter()
            .any(|alias| evidence.contains(alias))
            .then_some("server")
    }
}

impl DeviceAdapter for MobileAdapter {
    fn platform(&self) -> &'static DevicePlatform {
        &MOBILE_PLATFORM
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["android", "iphone", "ipados", "ios"]
    }

    fn vpn_profile(&self) -> Option<&'static str> {
        Some("mobile")
    }
}

impl DeviceAdapter for OtherAdapter {
    fn platform(&self) -> &'static DevicePlatform {
        &OTHER_PLATFORM
    }
}

impl SyslogConfigurationAdapter for RouterOsSyslogAdapter {
    fn label(&self) -> &'static str {
        "MikroTik RouterOS"
    }

    fn note(&self) -> &'static str {
        "`bsd-syslog=yes` é recomendado: sem ele o RouterOS envia um formato próprio, sem data e sem nome do equipamento. A severidade e os tópicos continuam sendo lidos de qualquer forma. Se o roteador tiver mais de um IP, acrescente `src-address=` com o endereço cadastrado aqui."
    }

    fn commands(&self, target: &str, port: u16) -> Vec<String> {
        vec![
            format!(
                "/system logging action add name=netmonitor target=remote remote={target} remote-port={port} bsd-syslog=yes"
            ),
            "/system logging add topics=system action=netmonitor".to_owned(),
            "/system logging add topics=error action=netmonitor".to_owned(),
            "/system logging add topics=critical action=netmonitor".to_owned(),
            "/system logging add topics=interface action=netmonitor".to_owned(),
        ]
    }

    fn identity_command(&self, marker: &str) -> String {
        format!(r#":put ("{marker}" . [/system identity get name])"#)
    }

    fn test_command(&self, message: &str) -> String {
        format!(r#":log error "{message}""#)
    }

    fn topics(&self, app_name: &str) -> Option<String> {
        if !app_name.contains(',') {
            return None;
        }
        app_name
            .split(',')
            .all(|part| {
                !part.is_empty()
                    && part.chars().all(|character| {
                        character.is_ascii_alphanumeric() || character == '-' || character == '_'
                    })
            })
            .then(|| app_name.to_owned())
    }

    fn severity(&self, topics: &str) -> Option<i16> {
        topics
            .split(',')
            .filter_map(|topic| match topic {
                "emergency" => Some(SyslogSeverity::SEV_EMERG as i16),
                "alert" => Some(SyslogSeverity::SEV_ALERT as i16),
                "critical" => Some(SyslogSeverity::SEV_CRIT as i16),
                "error" => Some(SyslogSeverity::SEV_ERR as i16),
                "warning" => Some(SyslogSeverity::SEV_WARNING as i16),
                "info" => Some(SyslogSeverity::SEV_INFO as i16),
                "debug" => Some(SyslogSeverity::SEV_DEBUG as i16),
                _ => None,
            })
            .min()
    }
}

impl SyslogConfigurationAdapter for OpenWrtSyslogAdapter {
    fn label(&self) -> &'static str {
        "OpenWRT"
    }

    fn note(&self) -> &'static str {
        "O `log_port` é a porta publicada, não a interna. Depois de reiniciar o serviço, os registros aparecem em poucos segundos."
    }

    fn commands(&self, target: &str, port: u16) -> Vec<String> {
        vec![
            format!("uci set system.@system[0].log_ip='{target}'"),
            format!("uci set system.@system[0].log_port='{port}'"),
            "uci set system.@system[0].log_proto='udp'".to_owned(),
            "uci commit system && /etc/init.d/log restart".to_owned(),
        ]
    }

    fn identity_command(&self, marker: &str) -> String {
        unix_identity_command(marker)
    }

    fn test_command(&self, message: &str) -> String {
        logger_test_command(message)
    }
}

impl SyslogConfigurationAdapter for LinuxSyslogAdapter {
    fn label(&self) -> &'static str {
        "Linux (rsyslog)"
    }

    fn note(&self) -> &'static str {
        "Um único `@` usa UDP; `@@` usa TCP. As duas formas são aceitas — o servidor escuta nos dois protocolos. Na ativação automática, o usuário informado precisa poder usar `sudo`."
    }

    fn commands(&self, target: &str, port: u16) -> Vec<String> {
        vec![
            format!("echo '*.* @{target}:{port}' | sudo tee /etc/rsyslog.d/60-netmonitor.conf"),
            "sudo systemctl restart rsyslog".to_owned(),
        ]
    }

    fn identity_command(&self, marker: &str) -> String {
        unix_identity_command(marker)
    }

    fn test_command(&self, message: &str) -> String {
        logger_test_command(message)
    }
}

impl SyslogConfigurationAdapter for UbiquitiSyslogAdapter {
    fn label(&self) -> &'static str {
        "Ubiquiti EdgeOS"
    }

    fn note(&self) -> &'static str {
        "No UniFi, o mesmo ajuste fica em Configurações → Sistema → Registro remoto, apontando para o mesmo endereço e porta."
    }

    fn commands(&self, target: &str, port: u16) -> Vec<String> {
        vec![
            "configure".to_owned(),
            format!("set system syslog host {target} facility all level info"),
            format!("set system syslog host {target} port {port}"),
            "commit".to_owned(),
            "save".to_owned(),
            "exit".to_owned(),
        ]
    }

    fn identity_command(&self, marker: &str) -> String {
        unix_identity_command(marker)
    }

    fn test_command(&self, message: &str) -> String {
        logger_test_command(message)
    }
}

fn unix_identity_command(marker: &str) -> String {
    format!(r#"printf '{marker}%s\n' "$(hostname 2>/dev/null)""#)
}

fn logger_test_command(message: &str) -> String {
    format!(r#"logger -p daemon.err "{message}""#)
}
