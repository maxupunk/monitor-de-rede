//! Cliente ICMP único do processo (`surge-ping`, socket DGRAM sem privilégio —
//! ADR 003) e a célula que o reabre enquanto o ambiente não deixa.

use std::{
    net::IpAddr,
    sync::{Arc, Mutex, PoisonError},
};

use loco_rs::app::AppContext;
use socket2::Type;
use surge_ping::{Client, Config, ICMP};

use crate::shared::errors::{AppError, AppResult};

/// Cliente único por processo. O socket compartilhado evita abrir um raw/DGRAM
/// socket para cada monitor e o `PingIdentifier` faz a multiplexação segura.
#[derive(Clone)]
pub struct PingClient {
    ipv4: Arc<Client>,
    ipv6: Option<Arc<Client>>,
}

impl PingClient {
    /// Cria o cliente com DGRAM, a modalidade que funciona sem `CAP_NET_RAW`.
    pub fn create() -> std::io::Result<Self> {
        let ipv4_config = Config::builder()
            .kind(ICMP::V4)
            .sock_type_hint(Type::DGRAM)
            .build();
        let ipv6_config = Config::builder()
            .kind(ICMP::V6)
            .sock_type_hint(Type::DGRAM)
            .build();
        let ipv4 = Client::new(&ipv4_config)?;
        let ipv6 = Client::new(&ipv6_config)
            .map(Arc::new)
            .map_err(|error| {
                tracing::warn!(%error, "socket ICMPv6 DGRAM indisponível; ICMPv4 permanece ativo");
                error
            })
            .ok();
        Ok(Self {
            ipv4: Arc::new(ipv4),
            ipv6,
        })
    }

    /// Cliente do processo, aberto agora se o boot não conseguiu.
    pub fn from_context(ctx: &AppContext) -> AppResult<Self> {
        ctx.shared_store
            .get::<PingClientCell>()
            .unwrap_or_default()
            .client()
    }

    #[must_use]
    pub fn for_ip(&self, ip: IpAddr) -> Option<Arc<Client>> {
        match ip {
            IpAddr::V4(_) => Some(Arc::clone(&self.ipv4)),
            IpAddr::V6(_) => self.ipv6.clone(),
        }
    }
}

/// Guarda o [`PingClient`] do processo e **tenta de novo** enquanto ele não abre.
///
/// O socket falha por ambiente, não por bug: `net.ipv4.ping_group_range`
/// fechado devolve `EPERM`/`EACCES` (ADR 003). Abrir só no boot transformava
/// isso em falha permanente — corrigido o sysctl no host, os monitores e a
/// descoberta continuavam quebrados até alguém reiniciar o container. Aqui cada
/// uso que encontra a célula vazia tenta abrir: são duas chamadas de sistema que
/// falham na hora, e o log só registra as transições (caiu / voltou).
#[derive(Clone, Default)]
pub struct PingClientCell(Arc<Mutex<PingCellState>>);

#[derive(Default)]
struct PingCellState {
    client: Option<PingClient>,
    failing: bool,
}

impl PingClientCell {
    /// Célula já com a primeira tentativa feita — o aviso sai no boot.
    #[must_use]
    pub fn open() -> Self {
        let cell = Self::default();
        let _ = cell.client();
        cell
    }

    pub fn client(&self) -> AppResult<PingClient> {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(client) = &state.client {
            return Ok(client.clone());
        }
        match PingClient::create() {
            Ok(client) => {
                if state.failing {
                    tracing::info!("socket ICMP DGRAM disponível; ping voltou a funcionar");
                }
                state.failing = false;
                state.client = Some(client.clone());
                Ok(client)
            }
            Err(error) => {
                if !state.failing {
                    tracing::warn!(
                        %error,
                        "socket ICMP DGRAM indisponível; ping e varredura ICMP falham até o \
                         net.ipv4.ping_group_range ser liberado (ADR 003)"
                    );
                }
                state.failing = true;
                Err(icmp_unavailable(&error))
            }
        }
    }
}

/// Mensagem que aparece no resultado do monitor e na tela de descoberta: diz o
/// que fazer, não só o que falhou.
fn icmp_unavailable(error: &std::io::Error) -> AppError {
    AppError::ServiceUnavailable(format!(
        "Ping indisponível: o sistema recusou o socket ICMP sem privilégio ({error}). \
         Libere net.ipv4.ping_group_range no host — com a rede do host, o serviço \
         icmp-sysctl do docker-compose.host.yml faz isso; à mão: \
         sysctl -w net.ipv4.ping_group_range=\"0 2147483647\". Não é preciso reiniciar."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falta_de_icmp_diz_como_liberar_o_sysctl() {
        let error = std::io::Error::from_raw_os_error(1);
        let message = icmp_unavailable(&error).to_string();
        assert!(message.starts_with("Ping indisponível"), "{message}");
        assert!(message.contains("net.ipv4.ping_group_range"), "{message}");
        assert!(message.contains("icmp-sysctl"), "{message}");
    }

    #[tokio::test]
    async fn celula_guarda_o_cliente_depois_que_abre() {
        let cell = PingClientCell::default();
        // Sem o sysctl liberado o socket não abre aqui também — o que se prova
        // é que, aberto uma vez, o mesmo cliente é devolvido sem reabrir.
        if let Ok(first) = cell.client() {
            let second = cell.client().expect("cliente guardado");
            assert!(Arc::ptr_eq(&first.ipv4, &second.ipv4));
        }
    }
}
