//! O efeito de um acesso ao equipamento: só lê, ou altera.
//!
//! A declaração do plugin (ou da IA) diz a intenção; a classificação aqui diz
//! o que o comando parece fazer. Vale sempre a **mais restritiva** das duas:
//! uma ação declarada como leitura que tenta `uci commit` é recusada, e um
//! pedido da IA que se diz "leitura" mas roda `opkg install` pede a confirmação
//! de escrita. A heurística pode errar para o lado de "escrita" — isso custa
//! uma confirmação a mais; errar para o outro lado custaria o equipamento.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub enum Effect {
    Read,
    Write,
}

impl Effect {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }

    /// A mais restritiva das duas.
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        std::cmp::max(self, other)
    }
}

/// Comandos de shell que alteram estado. Palavra inteira, para `cat` não casar
/// com `concat` nem `rm` com `firmware`.
const WRITE_COMMANDS: &[&str] = &[
    // pacotes
    r"opkg\s+(install|remove|upgrade|update|flag)",
    r"apk\s+(add|del|upgrade|update)",
    r"(apt|apt-get|yum|dnf)\s+(install|remove|purge|upgrade|update|autoremove)",
    // OpenWrt / UCI
    r"uci\s+(set|add|add_list|del|del_list|delete|rename|reorder|commit|revert|import|batch)",
    r"sysupgrade",
    r"firstboot",
    r"jffs2reset",
    r"mtd\s+(write|erase|-r)",
    // serviços e sistema
    r"/etc/init\.d/\S+\s+(start|stop|restart|reload|enable|disable)",
    r"(systemctl|service)\s+\S*\s*(start|stop|restart|reload|enable|disable|mask)",
    r"(reboot|poweroff|halt|shutdown)",
    r"wifi(\s+(up|down|reload)|\s*$)",
    // arquivos
    r"(rm|mv|cp|dd|chmod|chown|chgrp|ln|mkdir|rmdir|touch|truncate|tee|sed\s+-i|install)",
    // rede e usuários
    r"(ip|ifconfig)\s+\S*\s*(set|add|del|delete|flush|up|down)",
    r"(iptables|ip6tables|nft)\s",
    r"(passwd|useradd|userdel|usermod|adduser|deluser|chpasswd)",
    r"crontab",
    r"(kill|killall|pkill)",
    // RouterOS
    r"/[a-z0-9-]+(\s+[a-z0-9-]+)*?\s+(add|set|remove|enable|disable|reset|import)",
    r"/system\s+(reboot|reset-configuration|shutdown)",
];

fn write_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        WRITE_COMMANDS
            .iter()
            .map(|pattern| {
                // Início de comando: começo da linha ou depois de `;`, `&&`,
                // `||`, `|`, `(` e `sudo`/`xargs`.
                Regex::new(&format!(
                    r"(?:^|[;&|(`]\s*|\$\(\s*|\bsudo\s+|\bxargs\s+)(?:{pattern})\b"
                ))
                .expect("padrão de comando de escrita válido")
            })
            .collect()
    })
}

/// Redirecionamento para arquivo (`> arquivo`, `>> arquivo`), exceto os
/// descartes inofensivos (`2>/dev/null`, `>&2`).
fn redirects_to_file(command: &str) -> bool {
    static REDIRECT: OnceLock<Regex> = OnceLock::new();
    let redirect = REDIRECT.get_or_init(|| {
        Regex::new(r">{1,2}\s*([^\s&|;]+)").expect("regex de redirecionamento válida")
    });
    redirect.captures_iter(command).any(|capture| {
        let target = capture.get(1).map_or("", |m| m.as_str());
        target != "/dev/null" && !target.starts_with('&')
    })
}

/// O efeito provável de um comando de shell (SSH/Telnet).
#[must_use]
pub fn classify_command(command: &str) -> Effect {
    let normalized = command.trim();
    if normalized.is_empty() {
        return Effect::Read;
    }
    let writes = normalized
        .lines()
        .map(str::trim)
        .any(|line| write_patterns().iter().any(|re| re.is_match(line)) || redirects_to_file(line));
    if writes {
        Effect::Write
    } else {
        Effect::Read
    }
}

/// O efeito provável de uma requisição HTTP. `GET`/`HEAD`/`OPTIONS` leem; o
/// resto altera — inclusive o `POST` de login, que para a política de
/// aprovação é tratado como escrita e precisa ser liberado pela ação.
#[must_use]
pub fn classify_http(method: &str) -> Effect {
    match method.trim().to_ascii_uppercase().as_str() {
        "GET" | "HEAD" | "OPTIONS" => Effect::Read,
        _ => Effect::Write,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leitura_comum_nao_e_escrita() {
        for command in [
            "cat /etc/openwrt_release",
            "uci show network",
            "opkg list-installed",
            "ubus call system board",
            "ip addr show",
            "ls -la /tmp 2>/dev/null",
            "/system resource print",
            "grep -c processor /proc/cpuinfo",
            "concatena firmware",
        ] {
            assert_eq!(classify_command(command), Effect::Read, "{command}");
        }
    }

    #[test]
    fn escrita_e_reconhecida_ate_no_meio_da_linha() {
        for command in [
            "opkg update && opkg install luci-app-sqm",
            "uci set network.lan.ipaddr=192.168.1.2; uci commit network",
            "rm -rf /tmp/x",
            "echo 1 > /proc/sys/net/ipv4/ip_forward",
            "sudo systemctl restart nginx",
            "reboot",
            "/etc/init.d/network restart",
            "sysupgrade -n /tmp/fw.bin",
            "/ip address add address=10.0.0.1/24 interface=ether1",
            "cat x | tee /etc/config/x",
            "$(rm -f /etc/passwd)",
        ] {
            assert_eq!(classify_command(command), Effect::Write, "{command}");
        }
    }

    #[test]
    fn http_de_leitura_so_com_metodo_seguro() {
        assert_eq!(classify_http("get"), Effect::Read);
        assert_eq!(classify_http("HEAD"), Effect::Read);
        assert_eq!(classify_http("POST"), Effect::Write);
        assert_eq!(classify_http("DELETE"), Effect::Write);
    }

    #[test]
    fn a_mais_restritiva_vence() {
        assert_eq!(Effect::Read.max(Effect::Write), Effect::Write);
        assert_eq!(Effect::Read.max(Effect::Read), Effect::Read);
    }
}
