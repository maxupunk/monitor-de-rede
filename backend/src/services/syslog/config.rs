//! Configuração do servidor de syslog, lida do ambiente.
//!
//! Mesma regra do `data_pruner`: valor ausente ou inválido cai no padrão, e
//! `0` **não** desliga um limite. Desligar a proteção por erro de digitação no
//! compose enche o disco em silêncio, que é pior do que ignorar a configuração.

use std::time::Duration;

/// Porta padrão de escuta. **Não é 514**: o processo roda com `CapEff` zerado
/// (`setpriv --inh-caps=-all` no entrypoint) e não consegue abrir porta
/// privilegiada. Quem faz a ponte é o `docker-compose.yml`, publicando
/// `514:5514`.
pub const DEFAULT_PORT: u16 = 5514;

/// Teto de uma mensagem. O RFC 5426 exige que o receptor aguente 480 bytes e
/// recomenda 2048; 8 KiB cobre linha de firewall do RouterOS com folga sem
/// abrir espaço para um remetente hostil reservar memória à vontade.
pub const DEFAULT_MAX_MSG_BYTES: usize = 8192;

/// Fôlego da fila entre o listener e o escritor: ~50 s de pico a 200 msg/s,
/// ~3 MB de memória. Grande o bastante para atravessar uma pausa de disco,
/// pequeno o bastante para a perda ser visível antes de virar problema.
pub const DEFAULT_QUEUE_CAPACITY: usize = 10_000;

/// Teto por fonte, em mensagens por segundo, com rajada de 4×.
///
/// Não são os 200 do pico global: a 200/s **uma** fonte defeituosa consumiria
/// sozinha todo o orçamento. A rajada absorve o despejo de buffer de um
/// roteador que acabou de reiniciar.
pub const DEFAULT_RATE_LIMIT_PER_SOURCE: u32 = 50;

/// Gatilhos do escritor em lote — o que vier primeiro.
pub const DEFAULT_BATCH_SIZE: usize = 500;
pub const DEFAULT_BATCH_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyslogConfig {
    pub enabled: bool,
    pub udp_port: u16,
    pub tcp_port: u16,
    pub max_msg_bytes: usize,
    pub queue_capacity: usize,
    pub rate_limit_per_source: u32,
    pub accept_unknown_sources: bool,
    pub store_raw: bool,
}

impl Default for SyslogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            udp_port: DEFAULT_PORT,
            tcp_port: DEFAULT_PORT,
            max_msg_bytes: DEFAULT_MAX_MSG_BYTES,
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
            rate_limit_per_source: DEFAULT_RATE_LIMIT_PER_SOURCE,
            accept_unknown_sources: false,
            store_raw: false,
        }
    }
}

impl SyslogConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let padrao = Self::default();
        Self {
            enabled: flag("SYSLOG_ENABLED", padrao.enabled),
            udp_port: numero("SYSLOG_UDP_PORT", padrao.udp_port),
            tcp_port: numero("SYSLOG_TCP_PORT", padrao.tcp_port),
            max_msg_bytes: numero("SYSLOG_MAX_MSG_BYTES", padrao.max_msg_bytes),
            queue_capacity: numero("SYSLOG_QUEUE_CAPACITY", padrao.queue_capacity),
            rate_limit_per_source: numero(
                "SYSLOG_RATE_LIMIT_PER_SOURCE",
                padrao.rate_limit_per_source,
            ),
            accept_unknown_sources: flag(
                "SYSLOG_ACCEPT_UNKNOWN_SOURCES",
                padrao.accept_unknown_sources,
            ),
            store_raw: flag("SYSLOG_STORE_RAW", padrao.store_raw),
        }
    }
}

/// Lê um booleano do ambiente. Só as negações conhecidas desligam algo ligado —
/// o mesmo critério de `SCHEDULER_ENABLED`, e pelo mesmo motivo: um erro de
/// digitação não pode parar a ingestão em silêncio.
fn flag(variavel: &str, padrao: bool) -> bool {
    std::env::var(variavel).map_or(padrao, |valor| {
        match valor.trim().to_ascii_lowercase().as_str() {
            "false" | "0" | "no" | "off" => false,
            "true" | "1" | "yes" | "on" => true,
            _ => padrao,
        }
    })
}

/// Lê um número positivo. Zero e lixo caem no padrão.
fn numero<T>(variavel: &str, padrao: T) -> T
where
    T: std::str::FromStr + PartialOrd + Default,
{
    std::env::var(variavel)
        .ok()
        .and_then(|valor| valor.trim().parse::<T>().ok())
        .filter(|valor| *valor > T::default())
        .unwrap_or(padrao)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn o_padrao_escuta_na_5514_e_recusa_fonte_desconhecida() {
        for variavel in [
            "SYSLOG_ENABLED",
            "SYSLOG_UDP_PORT",
            "SYSLOG_TCP_PORT",
            "SYSLOG_MAX_MSG_BYTES",
            "SYSLOG_QUEUE_CAPACITY",
            "SYSLOG_RATE_LIMIT_PER_SOURCE",
            "SYSLOG_ACCEPT_UNKNOWN_SOURCES",
            "SYSLOG_STORE_RAW",
        ] {
            std::env::remove_var(variavel);
        }
        let config = SyslogConfig::from_env();
        assert!(config.enabled);
        assert_eq!(config.udp_port, 5514, "514 exigiria capability");
        assert_eq!(config.tcp_port, 5514);
        assert!(!config.accept_unknown_sources);
        assert!(!config.store_raw, "payload cru dobra o disco");
    }

    #[test]
    #[serial]
    fn valor_invalido_ou_zerado_nao_desliga_o_limite() {
        for invalido in ["0", "-1", "sempre", ""] {
            std::env::set_var("SYSLOG_QUEUE_CAPACITY", invalido);
            assert_eq!(
                SyslogConfig::from_env().queue_capacity,
                DEFAULT_QUEUE_CAPACITY,
                "aceitou {invalido:?}"
            );
        }
        std::env::remove_var("SYSLOG_QUEUE_CAPACITY");
    }

    #[test]
    #[serial]
    fn so_uma_negacao_conhecida_desliga_a_ingestao() {
        std::env::set_var("SYSLOG_ENABLED", "false");
        assert!(!SyslogConfig::from_env().enabled);
        // Erro de digitação mantém a ingestão de pé.
        std::env::set_var("SYSLOG_ENABLED", "sim");
        assert!(SyslogConfig::from_env().enabled);
        std::env::remove_var("SYSLOG_ENABLED");
    }
}
