//! Onde este processo está rodando — para explicar falhas, não para mudar
//! comportamento.

use std::net::IpAddr;
use std::sync::OnceLock;

/// Nome que o Docker resolve para a máquina hospedeira (Docker Desktop sempre;
/// no Linux pelo `extra_hosts: host-gateway` do compose).
pub const DOCKER_HOST_ALIAS: &str = "host.docker.internal";

/// O processo roda dentro de um container?
#[must_use]
pub fn in_container() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        std::path::Path::new("/.dockerenv").exists()
            || std::fs::read_to_string("/proc/1/cgroup")
                .is_ok_and(|text| text.contains("docker") || text.contains("containerd"))
    })
}

/// O endereço aponta para a própria máquina (`localhost`, `127.x`, `::1`)?
#[must_use]
pub fn is_loopback_host(host: &str) -> bool {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Explicação para "conexão recusada" num endereço local, quando o servidor
/// está num container: ali `127.0.0.1` é o próprio container, não a máquina.
#[must_use]
pub fn loopback_hint(host: &str) -> Option<String> {
    loopback_hint_for(host, in_container())
}

fn loopback_hint_for(host: &str, container: bool) -> Option<String> {
    (container && is_loopback_host(host)).then(|| {
        format!(
            "Este servidor roda num container, e na rede padrão do Docker \"{}\" é o próprio \
             container, não a sua máquina. Para um serviço da máquina hospedeira use \
             \"{DOCKER_HOST_ALIAS}\"; para outro computador, o IP dele na rede",
            host.trim()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_os_enderecos_locais() {
        for host in [
            "127.0.0.1",
            "127.0.1.1",
            "localhost",
            "LOCALHOST",
            "::1",
            "[::1]",
            " 127.0.0.1 ",
        ] {
            assert!(is_loopback_host(host), "{host}");
        }
        for host in ["10.0.0.5", "host.docker.internal", "db.local", "0.0.0.0"] {
            assert!(!is_loopback_host(host), "{host}");
        }
    }

    #[test]
    fn so_explica_quando_o_servidor_esta_num_container() {
        let hint = loopback_hint_for("127.0.0.1", true).unwrap();
        assert!(hint.contains(DOCKER_HOST_ALIAS));
        assert!(loopback_hint_for("127.0.0.1", false).is_none());
        assert!(loopback_hint_for("10.0.0.5", true).is_none());
    }
}
