//! O usuário SNMPv3 que o script da VPN cria no equipamento.
//!
//! A credencial vem do perfil SNMP do dispositivo (`snmp_profiles`). Cada
//! plataforma aceita um subconjunto dos protocolos do USM: o que ela não
//! aceita, ou um valor que não cabe com segurança na linha de configuração,
//! vira um aviso no script — nunca uma credencial trocada em silêncio, que
//! deixaria a coleta falhando sem explicação.

use netmonitor_core::devices::snmp_credentials::{SnmpCredentials, SnmpVersion};

use crate::shell_escape::escape_routeros;

/// Credencial v3 já separada do resto do contexto.
#[derive(Debug, Clone, Copy)]
pub struct SnmpV3User<'a> {
    pub username: &'a str,
    pub auth: Option<(&'a str, &'a str)>,
    pub privacy: Option<(&'a str, &'a str)>,
}

impl<'a> SnmpV3User<'a> {
    /// `None` quando a credencial não é v3.
    #[must_use]
    pub fn from_credentials(credentials: &'a SnmpCredentials) -> Option<Self> {
        if credentials.version != SnmpVersion::V3 {
            return None;
        }
        let pair = |protocol: &'a Option<String>, key: &'a Option<String>| {
            protocol.as_deref().zip(key.as_deref())
        };
        Some(Self {
            username: credentials.username.as_deref().unwrap_or_default(),
            auth: pair(&credentials.auth_protocol, &credentials.auth_key),
            privacy: pair(&credentials.priv_protocol, &credentials.priv_key),
        })
    }

    /// Nível de segurança no vocabulário do `rouser` do net-snmp.
    fn net_snmp_level(&self) -> &'static str {
        match (self.auth, self.privacy) {
            (Some(_), Some(_)) => "priv",
            (Some(_), None) => "auth",
            _ => "noauth",
        }
    }

    /// `createUser` e `rouser` para o `snmpd.conf` do net-snmp (Linux e
    /// OpenWrt). O net-snmp padrão só cifra com DES ou AES-128.
    ///
    /// # Errors
    ///
    /// O motivo, em português, quando o equipamento não aceitaria a credencial.
    pub fn net_snmp_directives(&self) -> Result<[String; 2], String> {
        let username = config_safe(self.username, "usuário")?;
        let mut create = format!("createUser {username}");
        if let Some((protocol, key)) = self.auth {
            let token = match protocol {
                "md5" => "MD5",
                "sha" => "SHA",
                "sha224" => "SHA-224",
                "sha256" => "SHA-256",
                "sha384" => "SHA-384",
                "sha512" => "SHA-512",
                other => return Err(unsupported("autenticação", other, "MD5 ou SHA")),
            };
            create.push_str(&format!(" {token} \"{}\"", config_safe(key, "senha")?));
            if let Some((protocol, key)) = self.privacy {
                let token = match protocol {
                    "des" => "DES",
                    "aes128" => "AES",
                    other => {
                        return Err(unsupported("privacidade", other, "DES ou AES-128"));
                    }
                };
                create.push_str(&format!(" {token} \"{}\"", config_safe(key, "senha")?));
            }
        }
        Ok([
            create,
            format!("rouser {username} {}", self.net_snmp_level()),
        ])
    }

    /// O comando do RouterOS v7, que guarda o usuário v3 como uma entrada de
    /// `/snmp/community` restrita à faixa da VPN. O RouterOS só autentica com
    /// MD5/SHA1 e só cifra com DES/AES-128.
    ///
    /// # Errors
    ///
    /// O motivo, em português, quando o RouterOS não aceitaria a credencial.
    pub fn routeros_command(&self, vpn_cidr: &str) -> Result<String, String> {
        let username = config_safe(self.username, "usuário")?;
        let mut command = format!(
            "/snmp/community/add name=\"{}\" addresses={} read-access=yes write-access=no",
            escape_routeros(username),
            escape_routeros(vpn_cidr)
        );
        let security = match (self.auth, self.privacy) {
            (Some(_), Some(_)) => "private",
            (Some(_), None) => "authorized",
            _ => "none",
        };
        command.push_str(&format!(" security={security}"));
        if let Some((protocol, key)) = self.auth {
            let token = match protocol {
                "md5" => "MD5",
                "sha" => "SHA1",
                other => return Err(unsupported("autenticação", other, "MD5 ou SHA")),
            };
            command.push_str(&format!(
                " authentication-protocol={token} authentication-password=\"{}\"",
                escape_routeros(config_safe(key, "senha")?)
            ));
        }
        if let Some((protocol, key)) = self.privacy {
            let token = match protocol {
                "des" => "DES",
                "aes128" => "AES",
                other => return Err(unsupported("privacidade", other, "DES ou AES-128")),
            };
            command.push_str(&format!(
                " encryption-protocol={token} encryption-password=\"{}\"",
                escape_routeros(config_safe(key, "senha")?)
            ));
        }
        Ok(command)
    }
}

/// Apaga os usuários v3 que o snmpd persistiu numa execução anterior. Com o
/// snmpd parado (ele regrava o arquivo ao sair): sem isso, o usuário guardado
/// com a senha antiga venceria o `createUser` novo e a troca de senha no
/// perfil nunca chegaria ao equipamento. Cobre Debian, RHEL e OpenWrt.
#[must_use]
pub fn purge_persisted_users() -> String {
    "for f in /var/lib/snmp/snmpd.conf /var/lib/net-snmp/snmpd.conf /usr/lib/snmp/snmpd.conf;      do [ -f \"$f\" ] && sed -i '/^usmUser /d' \"$f\"; done"
        .to_string()
}

/// O que o equipamento recebe entre aspas duplas numa linha de configuração.
/// Aspas, barra invertida, crase, cifrão, espaço e controles ficam de fora:
/// trocá-los mudaria a senha, e escapá-los dependeria do leitor de cada
/// plataforma.
fn config_safe<'v>(value: &'v str, what: &str) -> Result<&'v str, String> {
    let bad = |c: char| c.is_control() || c.is_whitespace() || "\"'\\`$".contains(c);
    if value.is_empty() || value.chars().any(bad) {
        return Err(format!(
            "o {what} SNMPv3 tem espaço, aspas, crase, cifrão ou barra invertida — \
             troque-o no perfil para o script poder gravá-lo"
        ));
    }
    Ok(value)
}

fn unsupported(kind: &str, protocol: &str, accepted: &str) -> String {
    format!("o equipamento não aceita {kind} {protocol} — use {accepted} no perfil SNMP")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v3(auth: Option<(&str, &str)>, privacy: Option<(&str, &str)>) -> SnmpCredentials {
        SnmpCredentials {
            username: Some("netmonitor".into()),
            auth_protocol: auth.map(|(p, _)| p.to_string()),
            auth_key: auth.map(|(_, k)| k.to_string()),
            priv_protocol: privacy.map(|(p, _)| p.to_string()),
            priv_key: privacy.map(|(_, k)| k.to_string()),
            ..SnmpCredentials::community(SnmpVersion::V3, "")
        }
    }

    #[test]
    fn comunidade_nao_vira_usuario_v3() {
        let v2 = SnmpCredentials::community(SnmpVersion::V2c, "netmon");
        assert!(SnmpV3User::from_credentials(&v2).is_none());
    }

    #[test]
    fn net_snmp_cria_o_usuario_com_o_nivel_certo() {
        let priv_ = v3(Some(("sha", "senha-auth")), Some(("aes128", "senha-priv")));
        let user = SnmpV3User::from_credentials(&priv_).unwrap();
        assert_eq!(
            user.net_snmp_directives().unwrap(),
            [
                "createUser netmonitor SHA \"senha-auth\" AES \"senha-priv\"".to_string(),
                "rouser netmonitor priv".to_string(),
            ]
        );
        let auth = v3(Some(("sha256", "senha-auth")), None);
        let user = SnmpV3User::from_credentials(&auth).unwrap();
        assert_eq!(
            user.net_snmp_directives().unwrap(),
            [
                "createUser netmonitor SHA-256 \"senha-auth\"".to_string(),
                "rouser netmonitor auth".to_string(),
            ]
        );
    }

    #[test]
    fn routeros_grava_o_usuario_restrito_a_vpn() {
        let priv_ = v3(Some(("sha", "senha-auth")), Some(("des", "senha-priv")));
        let user = SnmpV3User::from_credentials(&priv_).unwrap();
        assert_eq!(
            user.routeros_command("10.8.0.0/24").unwrap(),
            "/snmp/community/add name=\"netmonitor\" addresses=10.8.0.0/24 read-access=yes \
             write-access=no security=private authentication-protocol=SHA1 \
             authentication-password=\"senha-auth\" encryption-protocol=DES \
             encryption-password=\"senha-priv\""
        );
    }

    #[test]
    fn protocolo_que_a_plataforma_nao_tem_vira_aviso() {
        let sha512 = v3(Some(("sha512", "senha-auth")), None);
        let user = SnmpV3User::from_credentials(&sha512).unwrap();
        assert!(user
            .routeros_command("10.8.0.0/24")
            .unwrap_err()
            .contains("MD5 ou SHA"));
        assert!(user.net_snmp_directives().is_ok());

        let aes256 = v3(Some(("sha", "senha-auth")), Some(("aes256", "senha-priv")));
        let user = SnmpV3User::from_credentials(&aes256).unwrap();
        assert!(user.net_snmp_directives().unwrap_err().contains("AES-128"));
    }

    #[test]
    fn senha_que_nao_cabe_na_linha_nao_e_trocada_em_silencio() {
        for ruim in [
            "com espaco x",
            "aspas\"dupla",
            "barra\\inv",
            "cifrao$HOME",
            "q\nbra",
        ] {
            let credencial = v3(Some(("sha", ruim)), None);
            let user = SnmpV3User::from_credentials(&credencial).unwrap();
            assert!(user.net_snmp_directives().is_err(), "{ruim:?}");
            assert!(user.routeros_command("10.8.0.0/24").is_err(), "{ruim:?}");
        }
    }
}
