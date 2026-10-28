//! RouterOS: usuário `netmonitor`, num grupo próprio, com a chave importada.
//!
//! O grupo leva só as políticas que os plugins builtin usam — `ssh` para
//! entrar, `read`/`write` para ler e configurar, `test` para o
//! `bandwidth-test`, `policy` para o usuário temporário do teste de enlace e
//! `sensitive` para conferir a senha de uma rede. Fica de fora o resto
//! (`winbox`, `web`, `api`, `rest-api`, `ftp`, `telnet`, `password`, `reboot`,
//! `local`…): a conta não serve para entrar por outro caminho.
//!
//! Com chave cadastrada, o padrão do RouterOS (`/ip ssh
//! password-authentication=yes-if-no-key`) recusa senha no SSH para o
//! usuário. O NetMonitor não mexe nessa configuração global.

use super::{keys::GeneratedKey, Platform, StepOutput};
use crate::services::shared::errors::{AppError, AppResult};

pub const USERNAME: &str = "netmonitor";
const GROUP: &str = "netmonitor";
const POLICY: &str = "ssh,read,write,test,policy,sensitive";
const KEY_FILE: &str = "netmonitor.pub";
/// Chave de usuário ed25519 chegou no RouterOS 7.12.
const MIN_VERSION: (u32, u32) = (7, 12);

pub struct Mikrotik;

/// `7.15.2 (stable)` → `(7, 15)`.
fn parse_version(output: &str) -> Option<(u32, u32)> {
    let version = output.split_whitespace().next()?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts
        .next()
        .map(|minor| {
            minor
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
        })
        .and_then(|minor| minor.parse().ok())
        .unwrap_or(0);
    Some((major, minor))
}

impl Platform for Mikrotik {
    fn username(&self) -> &'static str {
        USERNAME
    }

    fn detect_command(&self) -> &'static str {
        ":put [/system resource get version]"
    }

    fn check_detected(&self, output: &StepOutput) -> AppResult<()> {
        let Some(version) = parse_version(output.stdout.trim()) else {
            return Err(AppError::business_rule(
                "O equipamento não respondeu como um RouterOS.",
            ));
        };
        if version < MIN_VERSION {
            return Err(AppError::business_rule(format!(
                "O acesso dedicado precisa do RouterOS {}.{} ou mais novo (chave ed25519); \
                 este está na {}.{}. Atualize o RouterOS ou continue com a senha.",
                MIN_VERSION.0, MIN_VERSION.1, version.0, version.1
            )));
        }
        Ok(())
    }

    fn install_commands(&self, key: &GeneratedKey, password: &str) -> Vec<String> {
        vec![
            format!(
                ":if ([:len [/user group find name=\"{GROUP}\"]] = 0) do={{/user group add \
                 name=\"{GROUP}\" policy=\"{POLICY}\" comment=\"NetMonitor: acesso dedicado\"}} \
                 else={{/user group set [find name=\"{GROUP}\"] policy=\"{POLICY}\"}}"
            ),
            format!(
                ":if ([:len [/user find name=\"{USERNAME}\"]] = 0) do={{/user add \
                 name=\"{USERNAME}\" group=\"{GROUP}\" password=\"{password}\" \
                 comment=\"NetMonitor: acesso dedicado por chave SSH\"}} \
                 else={{/user set [find name=\"{USERNAME}\"] group=\"{GROUP}\" disabled=no}}"
            ),
            // Uma chave só por vez: a nova substitui a anterior. A rotação
            // desfaz trocando de volta, pela sessão antiga ainda aberta.
            format!("/user ssh-keys remove [find user=\"{USERNAME}\"]"),
            format!(
                "/file add name=\"{KEY_FILE}\" contents=\"{}\"",
                key.public_openssh
            ),
            format!("/user ssh-keys import public-key-file=\"{KEY_FILE}\" user=\"{USERNAME}\""),
        ]
    }

    fn retire_commands(&self, _comment: &str) -> Vec<String> {
        // A instalação já trocou a chave inteira.
        Vec::new()
    }

    fn revoke_commands(&self, _comment: &str) -> Vec<String> {
        vec![
            format!("/user ssh-keys remove [find user=\"{USERNAME}\"]"),
            format!("/user remove [find name=\"{USERNAME}\"]"),
            format!("/user group remove [find name=\"{GROUP}\"]"),
        ]
    }

    fn failure(&self, output: &StepOutput) -> Option<String> {
        // O RouterOS responde erro em texto, quase sempre com código 0.
        const MARKS: [&str; 8] = [
            "failure:",
            "syntax error",
            "expected ",
            "bad command name",
            "no such item",
            "input does not match",
            "not enough permissions",
            "ambiguous value",
        ];
        let text = format!("{}\n{}", output.stdout, output.stderr);
        let lower = text.to_ascii_lowercase();
        if MARKS.iter().any(|mark| lower.contains(mark)) || output.exit_code.is_some_and(|c| c != 0)
        {
            Some(text.trim().to_owned())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(stdout: &str) -> StepOutput {
        StepOutput {
            stdout: stdout.into(),
            stderr: String::new(),
            exit_code: Some(0),
            host_key: None,
        }
    }

    #[test]
    fn versao_do_routeros() {
        assert_eq!(parse_version("7.15.2 (stable)"), Some((7, 15)));
        assert_eq!(parse_version("7.12rc1 (testing)"), Some((7, 12)));
        assert_eq!(parse_version("6.49.10 (long-term)"), Some((6, 49)));
        assert_eq!(parse_version("bad command name cat"), None);
        assert!(Mikrotik.check_detected(&out("7.12 (stable)")).is_ok());
        let antigo = Mikrotik
            .check_detected(&out("7.9.2 (stable)"))
            .unwrap_err()
            .to_string();
        assert!(antigo.contains("7.12"), "{antigo}");
        assert!(Mikrotik
            .check_detected(&out("DISTRIB_ID='OpenWrt'"))
            .is_err());
    }

    #[test]
    fn erro_do_routeros_vem_no_texto() {
        assert!(Mikrotik.failure(&out("")).is_none());
        assert!(Mikrotik
            .failure(&out("failure: user with the same name already exists"))
            .is_some());
        assert!(Mikrotik
            .failure(&out("syntax error (line 1 column 5)"))
            .is_some());
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
            "mikrotik_instala",
            Mikrotik
                .install_commands(&key, "SenhaDescartavel")
                .join("\n")
        );
        insta::assert_snapshot!(
            "mikrotik_revoga",
            Mikrotik.revoke_commands(&key.comment).join("\n")
        );
        assert!(Mikrotik.retire_commands(&key.comment).is_empty());
    }
}
