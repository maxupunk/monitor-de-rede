//! Instalação do agente como serviço systemd.
//!
//! O script é público e **não carrega segredo**: a URL da central e o código
//! de enrollment chegam pelo ambiente de quem o executa (o comando exibido
//! na tela, ou o script da VPN). O binário vem da própria central, pela
//! mesma conexão que o agente vai usar.

/// Unit systemd versionada no repositório (`assets/agent`).
pub const SYSTEMD_UNIT: &str = include_str!("../../../assets/agent/netmonitor-agent.service");

/// Arquiteturas publicadas em `AGENT_DIST_DIR`.
pub const SUPPORTED_ARCHES: [&str; 2] = ["x86_64", "aarch64"];

/// Pasta com os binários `netmonitor-agent-<arch>` servidos pela central.
#[must_use]
pub fn dist_dir() -> std::path::PathBuf {
    std::env::var("AGENT_DIST_DIR").map_or_else(|_| "/app/agent-dist".into(), Into::into)
}

/// Caminho do binário de uma arquitetura suportada.
#[must_use]
pub fn binary_path(arch: &str) -> Option<std::path::PathBuf> {
    SUPPORTED_ARCHES
        .contains(&arch)
        .then(|| dist_dir().join(format!("netmonitor-agent-{arch}")))
}

/// Script `install.sh`.
#[must_use]
pub fn script() -> String {
    format!(
        r#"#!/bin/sh
# Instala o agente NetMonitor como serviço systemd.
# Uso: curl -fsSL <central>/api/agents/install.sh | sudo AGENT_SERVER_URL=<central> AGENT_ENROLL_CODE=<código> sh
set -eu

: "${{AGENT_SERVER_URL:?defina AGENT_SERVER_URL com o endereço da central}}"
AGENT_ALLOW="${{AGENT_ALLOW:-read,lifecycle,monitor,discovery}}"

if [ "$(id -u)" -ne 0 ]; then
  echo "Execute como root (sudo)." >&2
  exit 1
fi
if ! command -v systemctl >/dev/null 2>&1; then
  echo "systemd não encontrado; use a instalação em container." >&2
  exit 1
fi

case "$(uname -m)" in
  x86_64|amd64) ARCH=x86_64 ;;
  aarch64|arm64) ARCH=aarch64 ;;
  *) echo "Arquitetura não suportada: $(uname -m)" >&2; exit 1 ;;
esac

echo "Baixando o agente ($ARCH) da central..."
curl -fsSL "$AGENT_SERVER_URL/api/agents/download/$ARCH" -o /usr/local/bin/netmonitor-agent.new
chmod 0755 /usr/local/bin/netmonitor-agent.new
mv /usr/local/bin/netmonitor-agent.new /usr/local/bin/netmonitor-agent

install -d -m 0755 /etc/netmonitor-agent
umask 077
{{
  echo "AGENT_SERVER_URL=$AGENT_SERVER_URL"
  echo "AGENT_ALLOW=$AGENT_ALLOW"
  if [ -n "${{AGENT_ENROLL_CODE:-}}" ]; then echo "AGENT_ENROLL_CODE=$AGENT_ENROLL_CODE"; fi
}} > /etc/netmonitor-agent/agent.env

cat > /etc/systemd/system/netmonitor-agent.service <<'UNIT'
{unit}UNIT

# ICMP sem privilégio (ADR 003) para os monitores de ping deste site.
if [ -w /proc/sys/net/ipv4/ping_group_range ]; then
  echo "net.ipv4.ping_group_range = 0 2147483647" > /etc/sysctl.d/60-netmonitor-agent.conf
  sysctl -q -p /etc/sysctl.d/60-netmonitor-agent.conf || true
fi

systemctl daemon-reload
systemctl enable --now netmonitor-agent
systemctl restart netmonitor-agent
echo "Agente NetMonitor instalado. Acompanhe com: journalctl -u netmonitor-agent -f"
"#,
        unit = SYSTEMD_UNIT
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_de_instalacao() {
        insta::assert_snapshot!(script());
    }

    #[test]
    fn so_arquiteturas_publicadas_tem_binario() {
        assert!(binary_path("x86_64").is_some());
        assert!(binary_path("aarch64").is_some());
        assert!(binary_path("../etc/passwd").is_none());
        assert!(binary_path("mips").is_none());
    }
}
