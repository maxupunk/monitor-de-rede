//! Quem é este host: o conteúdo do `Hello`.

use std::path::Path;

use crate::services::{
    agents::{
        policy::Policy,
        protocol::{Capability, Hello, PROTOCOL_VERSION},
    },
    docker::{
        compose,
        source::{DockerEngine, LocalEngine},
        DockerError,
    },
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

async fn docker_capability() -> Capability {
    match LocalEngine.status_raw().await {
        Ok(raw) => Capability {
            available: true,
            version: raw
                .version
                .get("Version")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string),
            reason: None,
        },
        Err(DockerError::Disabled) => Capability {
            available: false,
            version: None,
            reason: Some("DOCKER_ENABLED=false neste host".into()),
        },
        Err(_) => Capability {
            available: false,
            version: None,
            reason: Some("Docker Engine inacessível pelo socket".into()),
        },
    }
}

async fn compose_capability() -> Capability {
    match compose::availability().await {
        Ok(version) => Capability {
            available: true,
            version: Some(version),
            reason: None,
        },
        Err(reason) => Capability {
            available: false,
            version: None,
            reason: Some(reason),
        },
    }
}

/// Monta o `Hello` com o estado atual do host.
pub async fn hello(policy: &Policy) -> Hello {
    let (docker, compose) = tokio::join!(docker_capability(), compose_capability());
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
