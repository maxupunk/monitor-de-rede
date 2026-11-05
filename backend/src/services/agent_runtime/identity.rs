//! Quem é este host: o conteúdo do `Hello`.

use std::path::Path;

use netmonitor_core::agents::host::HostDocker;

use crate::services::agents::{
    policy::Policy,
    protocol::{Hello, PROTOCOL_VERSION},
};

/// `PRETTY_NAME` do `os-release`, quando existe.
#[must_use]
pub fn parse_os_release(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|value| value.trim().trim_matches('"').to_string())
        .filter(|value| !value.is_empty())
}

fn host_root() -> String {
    std::env::var("HOST_ROOT").unwrap_or_else(|_| "/".into())
}

fn read_host_file(relative: &str) -> Option<String> {
    std::fs::read_to_string(Path::new(&host_root()).join(relative)).ok()
}

fn hostname() -> String {
    read_host_file("etc/hostname")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "desconhecido".into())
}

/// Monta o `Hello` com o estado atual do host.
/// O Docker e o compose vêm do Docker do host (Servidores).
pub async fn hello(policy: &Policy, host_docker: &dyn HostDocker) -> Hello {
    let (docker, compose) = host_docker.capabilities().await;
    Hello {
        protocol: PROTOCOL_VERSION,
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        hostname: hostname(),
        os: read_host_file("etc/os-release")
            .as_deref()
            .and_then(parse_os_release)
            .unwrap_or_else(|| std::env::consts::OS.to_string()),
        arch: std::env::consts::ARCH.to_string(),
        in_container: Path::new("/.dockerenv").exists(),
        policy: policy.permissions(),
        docker,
        compose,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_do_sistema_vem_do_os_release() {
        let text = "NAME=\"Ubuntu\"\nPRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\nID=ubuntu\n";
        assert_eq!(
            parse_os_release(text).as_deref(),
            Some("Ubuntu 24.04.1 LTS")
        );
        assert_eq!(parse_os_release("ID=alpine"), None);
    }
}
