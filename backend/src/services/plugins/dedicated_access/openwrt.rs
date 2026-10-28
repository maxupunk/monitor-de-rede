//! OpenWrt: a chave do NetMonitor no `authorized_keys` do Dropbear (root).
//!
//! `uci`, `ubus` e o Wi-Fi só funcionam como root, e o OpenWrt não traz
//! `sudo`: um usuário próprio não serviria aos plugins. O ganho está em outro
//! lugar — a senha do root nunca é guardada, trocá-la não quebra o
//! monitoramento, e a chave é exclusiva deste equipamento e revogável.
//!
//! A linha leva `no-port-forwarding,no-agent-forwarding,no-X11-forwarding`: a
//! chave entra e roda comando, não vira túnel para o resto da rede. `no-pty`
//! fica de fora porque a ativação do syslog abre um shell com PTY.

use super::{keys::GeneratedKey, Platform, StepOutput};
use crate::services::shared::errors::{AppError, AppResult};

pub const USERNAME: &str = "root";
const AUTHORIZED_KEYS: &str = "/etc/dropbear/authorized_keys";
const OPTIONS: &str = "no-port-forwarding,no-agent-forwarding,no-X11-forwarding";

pub struct Openwrt;

/// O comentário só tem `netmonitor-` e hexadecimal (ver `keys`), e é conferido
/// aqui de novo: ele entra num `sed` e num `grep`.
fn safe_comment(comment: &str) -> Option<&str> {
    comment
        .strip_prefix(super::keys::COMMENT_PREFIX)
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|_| comment)
}

fn retire(comment: &str) -> Vec<String> {
    safe_comment(comment)
        .map(|comment| vec![format!("sed -i '/ {comment}$/d' {AUTHORIZED_KEYS}")])
        .unwrap_or_default()
}

impl Platform for Openwrt {
    fn username(&self) -> &'static str {
        USERNAME
    }

    fn detect_command(&self) -> &'static str {
        "cat /etc/openwrt_release; command -v dropbear"
    }

    fn check_detected(&self, output: &StepOutput) -> AppResult<()> {
        if !output.stdout.contains("DISTRIB_ID") {
            return Err(AppError::business_rule(
                "O equipamento não respondeu como um OpenWrt.",
            ));
        }
        if !output.stdout.contains("dropbear") {
            return Err(AppError::business_rule(
                "Este OpenWrt não usa o Dropbear como servidor SSH; o acesso dedicado ainda não \
                 cobre o OpenSSH. Continue com a senha.",
            ));
        }
        Ok(())
    }

    fn install_commands(&self, key: &GeneratedKey, _password: &str) -> Vec<String> {
        let Some(comment) = safe_comment(&key.comment) else {
            return Vec::new();
        };
        vec![format!(
            "umask 077; mkdir -p /etc/dropbear; touch {AUTHORIZED_KEYS}; \
             grep -qF ' {comment}' {AUTHORIZED_KEYS} || \
             echo '{OPTIONS} {}' >> {AUTHORIZED_KEYS}; chmod 600 {AUTHORIZED_KEYS}",
            key.public_openssh
        )]
    }

    fn retire_commands(&self, comment: &str) -> Vec<String> {
        retire(comment)
    }

    fn revoke_commands(&self, comment: &str) -> Vec<String> {
        retire(comment)
    }

    fn failure(&self, output: &StepOutput) -> Option<String> {
        output.exit_code.filter(|code| *code != 0).map(|code| {
            let text = format!("{}\n{}", output.stdout, output.stderr);
            format!("código {code}: {}", text.trim())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(stdout: &str, exit_code: i32) -> StepOutput {
        StepOutput {
            stdout: stdout.into(),
            stderr: String::new(),
            exit_code: Some(exit_code),
            host_key: None,
        }
    }

    #[test]
    fn deteccao_do_openwrt_com_dropbear() {
        let release = "DISTRIB_ID='OpenWrt'\nDISTRIB_RELEASE='23.05.3'\n/usr/sbin/dropbear\n";
        assert!(Openwrt.check_detected(&out(release, 0)).is_ok());
        assert!(Openwrt
            .check_detected(&out("DISTRIB_ID='OpenWrt'\n", 1))
            .unwrap_err()
            .to_string()
            .contains("Dropbear"));
        assert!(Openwrt
            .check_detected(&out("bad command name cat", 0))
            .is_err());
    }

    #[test]
    fn comentario_fora_do_formato_nao_vira_comando() {
        assert!(retire("netmonitor-abc'; rm -rf /").is_empty());
        assert!(retire("outra-coisa").is_empty());
        assert_eq!(retire("netmonitor-0a1b").len(), 1);
    }

    #[test]
    fn comandos_de_instalacao_e_revogacao() {
        let key = GeneratedKey {
            private_openssh: String::new(),
            public_openssh: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITESTE netmonitor-0123456789ab"
                .into(),
            fingerprint: "SHA256:teste".into(),
            comment: "netmonitor-0123456789ab".into(),
        };
        insta::assert_snapshot!(
            "openwrt_instala",
            Openwrt.install_commands(&key, "").join("\n")
        );
        insta::assert_snapshot!(
            "openwrt_revoga",
            Openwrt.revoke_commands(&key.comment).join("\n")
        );
        assert!(Openwrt.failure(&out("", 0)).is_none());
        assert!(Openwrt.failure(&out("sem permissão", 1)).is_some());
    }
}
