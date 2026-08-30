//! Contratos dos adapters de plataforma de dispositivo.
//!
//! O núcleo conhece somente estes contratos. Identificação, meios de acesso e
//! configuração específica ficam nas implementações concretas, de modo que um
//! novo sistema seja adicionado sem alterar os consumidores (OCP/DIP).

/// Metadados estáveis de uma plataforma expostos pela API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevicePlatform {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    /// Plataformas genéricas só vencem a detecção depois das específicas.
    pub generic: bool,
}

/// Meios de acesso cuja disponibilidade depende da plataforma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceAccessMethod {
    MacTelnet,
}

/// Adapter de configuração e dialeto Syslog de uma plataforma.
pub trait SyslogConfigurationAdapter: Send + Sync {
    fn label(&self) -> &'static str;
    fn note(&self) -> &'static str;
    fn commands(&self, server_address: &str, port: u16) -> Vec<String>;
    fn identity_command(&self, marker: &str) -> String;
    fn test_command(&self, message: &str) -> String;

    /// Reconhece metadados próprios no campo `app-name`/tag.
    fn topics(&self, _app_name: &str) -> Option<String> {
        None
    }

    /// Converte metadados próprios na severidade canônica do Syslog.
    fn severity(&self, _topics: &str) -> Option<i16> {
        None
    }
}

/// Adapter principal de uma família de dispositivos.
pub trait DeviceAdapter: Send + Sync {
    fn platform(&self) -> &'static DevicePlatform;

    /// Termos específicos usados em `sysDescr`, fabricante, modelo e nome.
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Prefixos empresariais de `sysObjectId`.
    fn sys_object_ids(&self) -> &'static [&'static str] {
        &[]
    }

    /// Marcas específicas no banner do servidor SSH.
    fn ssh_banners(&self) -> &'static [&'static str] {
        &[]
    }

    fn supports_access(&self, _method: DeviceAccessMethod) -> bool {
        false
    }

    /// Adapter especializado para provisionamento de logs, quando suportado.
    fn syslog(&self) -> Option<&'static dyn SyslogConfigurationAdapter> {
        None
    }

    /// Chave do adapter de geração WireGuard, quando suportado.
    fn vpn_profile(&self) -> Option<&'static str> {
        None
    }

    /// Papel sugerido durante discovery. `None` deixa a heurística genérica
    /// continuar; a plataforma só decide quando possui conhecimento específico.
    fn device_type_hint(&self, _evidence: &str) -> Option<&'static str> {
        None
    }

    /// Tipo inicial usado quando a plataforma cria um dispositivo no inventário.
    fn default_device_type(&self) -> &'static str {
        "host"
    }

    /// Impede que um `sysDescr` antigo seja apresentado como fabricante.
    fn is_system_description(&self, _value: &str) -> bool {
        false
    }
}
