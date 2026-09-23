//! Por qual endereço desta central o agente vai se conectar.
//!
//! A escolha é a mesma lista de "Endereços deste servidor" que o guia de
//! syslog e a VPN usam (`services::server_addresses`): rede local, túnel VPN,
//! internet ou um endereço definido pelo operador. Aqui ela vira a URL que
//! entra nos comandos de instalação — com a porta certa para cada caminho.
//!
//! **Porta.** Pelo túnel o agente chega direto no container (a `wg0` é dele),
//! então vale a porta real da API (`APP_PORT`). Pela rede local ou pela
//! internet ele passa pelo mapeamento do Docker, então vale a porta publicada
//! (`APP_EXTERNAL_PORT`, que no modo host é a própria `APP_PORT`).

use loco_rs::app::AppContext;

use crate::{
    models::devices,
    services::{
        server_addresses::{self, AddressKind, ServerAddress},
        shared::errors::{AppError, AppResult},
    },
    views::agents::AgentInstallCommands,
};

use super::service::install_commands;

const DEFAULT_PORT: u16 = 3333;

fn env_port(key: &str) -> Option<u16> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|port| *port > 0)
}

/// Portas `(túnel, publicada)` desta instalação.
#[must_use]
pub fn ports(ctx: &AppContext) -> (u16, u16) {
    let tunnel =
        env_port("APP_PORT").unwrap_or(ctx.config.server.port.try_into().unwrap_or(DEFAULT_PORT));
    let published = env_port("APP_EXTERNAL_PORT").unwrap_or(tunnel);
    (tunnel, published)
}

/// Só caracteres de host/URL: o valor entra num comando de shell que o
/// operador cola no servidor, e um endereço personalizado é texto livre.
fn is_safe(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_:/[]%".contains(c))
}

/// URL da central para um endereço da lista.
///
/// Aceita o que o operador costuma digitar: IP, nome (DDNS), `host:porta`,
/// IPv6 com ou sem colchetes, ou a URL completa (`https://…`), que vale como
/// está. `None` para valor vazio ou com caracteres que não pertencem a um host.
#[must_use]
pub fn server_url(
    kind: AddressKind,
    value: &str,
    (tunnel, published): (u16, u16),
) -> Option<String> {
    let value = value.trim().trim_end_matches('/');
    if !is_safe(value) {
        return None;
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return Some(value.to_string());
    }
    let port = if kind == AddressKind::Vpn {
        tunnel
    } else {
        published
    };
    let colons = value.matches(':').count();
    let authority = match colons {
        // `10.0.0.5` ou `monitor.exemplo.com`
        0 => format!("{value}:{port}"),
        // `10.0.0.5:8080` — a porta já veio junto
        1 => value.to_string(),
        // IPv6: com colchetes pode trazer porta; sem, recebe a padrão.
        _ if value.starts_with('[') && value.contains("]:") => value.to_string(),
        _ if value.starts_with('[') => format!("{value}:{port}"),
        _ => format!("[{value}]:{port}"),
    };
    Some(format!("http://{authority}"))
}

/// O código entra no comando colado no servidor: só o formato emitido pela
/// central (`nma_` + 64 hex) é aceito.
///
/// # Errors
///
/// `validation` para qualquer outro formato.
pub fn validate_code(code: &str) -> AppResult<&str> {
    let code = code.trim();
    let valid = code
        .strip_prefix("nma_")
        .is_some_and(|hex| hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()));
    if valid {
        Ok(code)
    } else {
        Err(AppError::validation("Código de instalação inválido"))
    }
}

/// Endereço padrão: o que o dispositivo vinculado usaria para chegar aqui
/// (mesma sugestão do provisionamento de syslog), senão o preferido do
/// operador, senão o primeiro com valor.
async fn default_address<'a>(
    ctx: &AppContext,
    addresses: &'a [ServerAddress],
    device: Option<&devices::Model>,
) -> AppResult<Option<&'a ServerAddress>> {
    let usable = || addresses.iter().filter(|address| address.value.is_some());
    if let Some(device) = device {
        if let Some((value, _)) = server_addresses::suggest_for(&ctx.db, device, addresses).await? {
            if let Some(found) = usable().find(|address| address.value.as_deref() == Some(&value)) {
                return Ok(Some(found));
            }
        }
    }
    let preferred = server_addresses::stored(&ctx.db).await?.preferred_id;
    Ok(usable()
        .find(|address| Some(&address.id) == preferred.as_ref())
        .or_else(|| usable().next()))
}

/// Comandos de instalação para o endereço escolhido (ou o padrão).
///
/// `AGENT_SERVER_URL` na central, quando definida, vence qualquer escolha: é
/// a forma de fixar um endereço que a lista não conhece (proxy, túnel próprio).
///
/// # Errors
///
/// Endereço inexistente, sem valor ou impróprio para URL; erro do banco.
pub async fn commands(
    ctx: &AppContext,
    code: &str,
    address_id: Option<&str>,
    device: Option<&devices::Model>,
    request_origin: &str,
) -> AppResult<AgentInstallCommands> {
    let code = validate_code(code)?;
    if let Ok(forced) = std::env::var("AGENT_SERVER_URL") {
        let forced = forced.trim().trim_end_matches('/').to_string();
        if !forced.is_empty() {
            return Ok(render(None, forced, code));
        }
    }
    let addresses = server_addresses::list(&ctx.db, &server_addresses::nat_detector(ctx)).await?;
    let chosen = match address_id {
        Some(id) => Some(
            addresses
                .iter()
                .find(|address| address.id == id && address.value.is_some())
                .ok_or_else(|| AppError::validation("Endereço deste servidor não encontrado"))?,
        ),
        None => default_address(ctx, &addresses, device).await?,
    };
    let Some(chosen) = chosen else {
        // Nenhum endereço cadastrado: a origem de quem abriu a tela é o
        // melhor palpite que resta, e a tela avisa que é um palpite.
        return Ok(render(
            None,
            request_origin.trim_end_matches('/').to_string(),
            code,
        ));
    };
    let url = server_url(
        chosen.kind,
        chosen.value.as_deref().unwrap_or_default(),
        ports(ctx),
    )
    .ok_or_else(|| {
        AppError::validation("O endereço escolhido não é um host ou URL válido para o agente")
    })?;
    Ok(render(Some(chosen.id.clone()), url, code))
}

fn render(address_id: Option<String>, server_url: String, code: &str) -> AgentInstallCommands {
    let (docker_command, systemd_command) = install_commands(&server_url, code);
    AgentInstallCommands {
        address_id,
        server_url,
        docker_command,
        systemd_command,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORTS: (u16, u16) = (3333, 8080);

    #[test]
    fn porta_do_tunel_para_vpn_e_publicada_para_o_resto() {
        assert_eq!(
            server_url(AddressKind::Vpn, "10.8.0.1", PORTS).as_deref(),
            Some("http://10.8.0.1:3333")
        );
        assert_eq!(
            server_url(AddressKind::Lan, "192.168.0.10", PORTS).as_deref(),
            Some("http://192.168.0.10:8080")
        );
        assert_eq!(
            server_url(AddressKind::Public, "monitor.exemplo.com", PORTS).as_deref(),
            Some("http://monitor.exemplo.com:8080")
        );
    }

    #[test]
    fn respeita_porta_url_completa_e_ipv6() {
        assert_eq!(
            server_url(AddressKind::Custom, "200.1.2.3:9000", PORTS).as_deref(),
            Some("http://200.1.2.3:9000")
        );
        assert_eq!(
            server_url(AddressKind::Custom, "https://monitor.exemplo.com/", PORTS).as_deref(),
            Some("https://monitor.exemplo.com")
        );
        assert_eq!(
            server_url(AddressKind::Lan, "fd00::5", PORTS).as_deref(),
            Some("http://[fd00::5]:8080")
        );
        assert_eq!(
            server_url(AddressKind::Lan, "[fd00::5]:9000", PORTS).as_deref(),
            Some("http://[fd00::5]:9000")
        );
    }

    #[test]
    fn valor_que_nao_e_host_nao_vira_comando() {
        for value in ["", "10.0.0.1; rm -rf /", "host name", "$(id)", "a`b`"] {
            assert_eq!(
                server_url(AddressKind::Custom, value, PORTS),
                None,
                "{value}"
            );
        }
    }

    #[test]
    fn so_o_formato_de_codigo_emitido_e_aceito() {
        let code = format!("nma_{}", "a".repeat(64));
        assert!(validate_code(&code).is_ok());
        assert!(validate_code("nma_curto").is_err());
        assert!(validate_code(&format!("nma_{}", "z".repeat(64))).is_err());
        assert!(validate_code(&format!("x{code}")).is_err());
    }
}
