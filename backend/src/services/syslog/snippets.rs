//! Comandos prontos de configuração, por fabricante.
//!
//! Existem porque o passo que mais falha na implantação não é técnico: é o
//! operador digitar o IP errado, esquecer `bsd-syslog=yes` ou apontar para a
//! 5514 em vez da 514. O snippet com o endereço já preenchido elimina os três.
//!
//! **A porta do snippet é sempre a 514**, nunca a 5514: é a porta publicada
//! pelo `docker-compose.yml`, e é ela que o roteador enxerga. A 5514 é detalhe
//! interno do container — quem a colocar no roteador não recebe nada.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Porta que o roteador deve usar. Ver a nota do módulo.
pub const PUBLISHED_PORT: u16 = 514;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SetupSnippet {
    /// Chave estável, para a tela escolher ícone e ordenar.
    pub vendor: String,
    pub label: String,
    /// O que o operador precisa saber antes de colar.
    pub note: String,
    pub commands: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SetupGuide {
    /// Endereço que o operador deve apontar. Vazio quando o servidor não
    /// conseguiu descobrir o próprio IP — a tela então pede que ele digite.
    pub server_address: String,
    #[ts(type = "number")]
    pub port: u16,
    pub snippets: Vec<SetupSnippet>,
}

/// Monta o guia com o endereço do servidor preenchido.
#[must_use]
pub fn build(server_address: &str) -> SetupGuide {
    let alvo = if server_address.trim().is_empty() {
        "<IP-DO-SERVIDOR>"
    } else {
        server_address.trim()
    };

    SetupGuide {
        server_address: alvo.to_owned(),
        port: PUBLISHED_PORT,
        snippets: vec![
            SetupSnippet {
                vendor: "routeros".into(),
                label: "MikroTik RouterOS".into(),
                note: "`bsd-syslog=yes` é recomendado: sem ele o RouterOS envia um formato \
                       próprio, sem data e sem nome do equipamento. A severidade e os tópicos \
                       continuam sendo lidos de qualquer forma. Se o roteador tiver mais de um \
                       IP, acrescente `src-address=` com o endereço cadastrado aqui."
                    .into(),
                commands: format!(
                    "/system logging action add name=netmonitor target=remote \\\n    \
                     remote={alvo} remote-port={PUBLISHED_PORT} bsd-syslog=yes\n\
                     /system logging add topics=system action=netmonitor\n\
                     /system logging add topics=error action=netmonitor\n\
                     /system logging add topics=critical action=netmonitor\n\
                     /system logging add topics=interface action=netmonitor"
                ),
            },
            SetupSnippet {
                vendor: "openwrt".into(),
                label: "OpenWRT".into(),
                note: "O `log_port` é a porta publicada, não a interna. Depois de reiniciar o \
                       serviço, os registros aparecem em poucos segundos."
                    .into(),
                commands: format!(
                    "uci set system.@system[0].log_ip='{alvo}'\n\
                     uci set system.@system[0].log_port='{PUBLISHED_PORT}'\n\
                     uci set system.@system[0].log_proto='udp'\n\
                     uci commit system && /etc/init.d/log restart"
                ),
            },
            SetupSnippet {
                vendor: "linux".into(),
                label: "Linux (rsyslog)".into(),
                note: "Um único `@` usa UDP; `@@` usa TCP. As duas formas são aceitas — o \
                       servidor escuta nos dois protocolos."
                    .into(),
                commands: format!(
                    "echo '*.* @{alvo}:{PUBLISHED_PORT}' | sudo tee /etc/rsyslog.d/60-netmonitor.conf\n\
                     sudo systemctl restart rsyslog"
                ),
            },
            SetupSnippet {
                vendor: "ubiquiti".into(),
                label: "Ubiquiti EdgeOS".into(),
                note: "No UniFi, o mesmo ajuste fica em Configurações → Sistema → Registro \
                       remoto, apontando para o mesmo endereço e porta."
                    .into(),
                commands: format!(
                    "configure\n\
                     set system syslog host {alvo} facility all level info\n\
                     set system syslog host {alvo} port {PUBLISHED_PORT}\n\
                     commit; save; exit"
                ),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_endereco_entra_em_todos_os_snippets() {
        let guia = build("192.168.1.10");
        assert_eq!(guia.server_address, "192.168.1.10");
        assert_eq!(guia.snippets.len(), 4);
        for snippet in &guia.snippets {
            assert!(
                snippet.commands.contains("192.168.1.10"),
                "{} não recebeu o endereço",
                snippet.vendor
            );
        }
    }

    #[test]
    fn a_porta_do_snippet_e_a_publicada_e_nunca_a_interna() {
        // 5514 é a porta de dentro do container; quem a puser no roteador não
        // recebe nada.
        let guia = build("10.0.0.5");
        assert_eq!(guia.port, 514);
        for snippet in &guia.snippets {
            assert!(
                !snippet.commands.contains("5514"),
                "{} vazou a porta interna",
                snippet.vendor
            );
        }
    }

    #[test]
    fn endereco_desconhecido_vira_marcador_e_nao_string_vazia() {
        // Um comando com `remote=` vazio seria colado do jeito que está e
        // falharia no roteador sem explicar por quê.
        for entrada in ["", "   "] {
            let guia = build(entrada);
            assert_eq!(guia.server_address, "<IP-DO-SERVIDOR>");
            assert!(guia.snippets[0].commands.contains("<IP-DO-SERVIDOR>"));
        }
    }

    #[test]
    fn o_routeros_insiste_no_bsd_syslog() {
        let guia = build("10.0.0.5");
        let routeros = guia
            .snippets
            .iter()
            .find(|s| s.vendor == "routeros")
            .expect("routeros");
        assert!(routeros.commands.contains("bsd-syslog=yes"));
        assert!(routeros.note.contains("src-address"));
    }
}
