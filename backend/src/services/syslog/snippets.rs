//! Comandos prontos de configuração, por fabricante.
//!
//! Existem porque o passo que mais falha na implantação não é técnico: é o
//! operador digitar o IP errado, esquecer `bsd-syslog=yes` ou apontar para a
//! 5514 em vez da 514. O snippet com o endereço já preenchido elimina os três.
//!
//! **A porta do snippet é a publicada, não a de escuta.** O padrão é 514 — a
//! que o `docker-compose.yml` publica e a que todo firmware assume. A 5514 é
//! detalhe interno do container, e quem a colocar no roteador não recebe nada.
//! Quem trocar o mapeamento (ou rodar em `network_mode: host`, onde não há
//! mapeamento nenhum e a porta real passa a ser a 5514) declara a porta certa
//! em `SYSLOG_EXTERNAL_PORT` — senão o comando gerado aponta para o lugar
//! errado com toda a convicção.
//!
//! **A receita é uma lista de comandos, não um bloco de texto.** É a mesma
//! fonte que alimenta o copiar-e-colar da tela e a ativação automática por
//! SSH/Telnet do [`super::provision`]: duas listas iriam divergir na primeira
//! correção, e a divergência só apareceria no equipamento de alguém.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Porta que o roteador deve usar, quando nada é declarado. Ver a nota do
/// módulo.
pub const DEFAULT_PUBLISHED_PORT: u16 = 514;

/// Variável que declara a porta realmente publicada — a mesma do compose.
pub const ENV_PUBLISHED_PORT: &str = "SYSLOG_EXTERNAL_PORT";

/// A porta que o roteador deve usar para alcançar este servidor.
///
/// Valor inválido ou zerado cai no padrão, como no resto da configuração de
/// syslog: um erro de digitação no compose não pode gerar um comando que
/// aponta para a porta 0.
#[must_use]
pub fn published_port() -> u16 {
    std::env::var(ENV_PUBLISHED_PORT)
        .ok()
        .and_then(|valor| valor.trim().parse::<u16>().ok())
        .filter(|porta| *porta > 0)
        .unwrap_or(DEFAULT_PUBLISHED_PORT)
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../frontend/src/bindings/")]
pub struct SetupSnippet {
    /// Chave do catálogo de sistemas, para a tela escolher ícone e ordenar.
    pub system: String,
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

/// Marcador que entra no lugar do endereço quando ele não é conhecido.
///
/// Não é string vazia de propósito: um `remote=` vazio seria colado do jeito
/// que está e falharia no roteador sem explicar por quê.
const ALVO_DESCONHECIDO: &str = "<IP-DO-SERVIDOR>";

/// Os sistemas com receita, na ordem em que a tela os mostra.
///
/// As chaves são as do catálogo de [`crate::services::devices::systems`] — não
/// um vocabulário próprio. Um teste lá garante que os dois lados não se
/// separem: uma receita órfã seria oferecida numa tela e recusada na outra.
#[must_use]
pub fn systems() -> Vec<&'static str> {
    RECEITAS.iter().map(|receita| receita.sistema).collect()
}

/// Os comandos de um sistema, um por linha, prontos para digitar.
///
/// É o que o [`super::provision`] envia pela sessão SSH ou Telnet. `None`
/// quando o sistema não tem receita — o chamador transforma isso em erro de
/// validação com o nome recebido, que é mais útil do que uma lista vazia.
#[must_use]
pub fn commands_for(sistema: &str, server_address: &str, port: u16) -> Option<Vec<String>> {
    let alvo = normaliza(server_address);
    RECEITAS
        .iter()
        .find(|receita| receita.sistema.eq_ignore_ascii_case(sistema.trim()))
        .map(|receita| (receita.comandos)(&alvo, port))
}

/// Monta o guia com o endereço do servidor preenchido.
#[must_use]
pub fn build(server_address: &str) -> SetupGuide {
    let alvo = normaliza(server_address);
    let porta = published_port();

    SetupGuide {
        server_address: alvo.clone(),
        port: porta,
        snippets: RECEITAS
            .iter()
            .map(|receita| SetupSnippet {
                system: receita.sistema.to_owned(),
                label: receita.label.to_owned(),
                note: receita.note.to_owned(),
                commands: (receita.comandos)(&alvo, porta).join("\n"),
            })
            .collect(),
    }
}

fn normaliza(server_address: &str) -> String {
    let limpo = server_address.trim();
    if limpo.is_empty() {
        ALVO_DESCONHECIDO.to_owned()
    } else {
        limpo.to_owned()
    }
}

/// Comando que faz o equipamento emitir **uma** linha de log agora.
///
/// Existe para desfazer uma ambiguidade que a ativação automática tinha: as
/// regras enviadas cobrem tópicos que só falam quando algo acontece, então um
/// roteador saudável pode ficar horas em silêncio depois de configurado — e
/// "nada chegou" não distinguia isso de um firewall bloqueando o caminho. Com a
/// linha de teste, silêncio passa a significar caminho bloqueado.
///
/// A severidade escolhida em cada um **casa com uma regra que acabou de ser
/// criada**: no RouterOS o tópico é `error`, que a receita inclui. Emitir em
/// `info` produziria uma linha que o próprio equipamento não encaminharia, e o
/// teste acusaria falha onde não há.
#[must_use]
pub fn test_command(sistema: &str) -> Option<String> {
    RECEITAS
        .iter()
        .find(|receita| receita.sistema.eq_ignore_ascii_case(sistema.trim()))
        .and_then(|receita| receita.teste.map(str::to_owned))
}

/// Uma receita de sistema. Os comandos são função do alvo e da porta porque os
/// dois só se conhecem em tempo de execução.
struct Receita {
    sistema: &'static str,
    label: &'static str,
    note: &'static str,
    comandos: fn(&str, u16) -> Vec<String>,
    /// Ver [`test_command`].
    teste: Option<&'static str>,
}

const RECEITAS: &[Receita] = &[
    Receita {
        sistema: "routeros",
        label: "MikroTik RouterOS",
        note: "`bsd-syslog=yes` é recomendado: sem ele o RouterOS envia um formato próprio, sem \
               data e sem nome do equipamento. A severidade e os tópicos continuam sendo lidos \
               de qualquer forma. Se o roteador tiver mais de um IP, acrescente `src-address=` \
               com o endereço cadastrado aqui.",
        comandos: |alvo, porta| {
            vec![
                format!(
                    "/system logging action add name=netmonitor target=remote remote={alvo} \
                     remote-port={porta} bsd-syslog=yes"
                ),
                "/system logging add topics=system action=netmonitor".to_owned(),
                "/system logging add topics=error action=netmonitor".to_owned(),
                "/system logging add topics=critical action=netmonitor".to_owned(),
                "/system logging add topics=interface action=netmonitor".to_owned(),
            ]
        },
        // `:log error` emite no tópico `error`, que a receita acabou de mandar
        // encaminhar. Emitir em `info` produziria uma linha que o próprio
        // equipamento não encaminha, e o teste acusaria falha onde não há.
        teste: Some(":log error \"netmonitor: teste de envio de log\""),
    },
    Receita {
        sistema: "openwrt",
        label: "OpenWRT",
        note: "O `log_port` é a porta publicada, não a interna. Depois de reiniciar o serviço, \
               os registros aparecem em poucos segundos.",
        comandos: |alvo, porta| {
            vec![
                format!("uci set system.@system[0].log_ip='{alvo}'"),
                format!("uci set system.@system[0].log_port='{porta}'"),
                "uci set system.@system[0].log_proto='udp'".to_owned(),
                "uci commit system && /etc/init.d/log restart".to_owned(),
            ]
        },
        // Atravessa o `logd`, que é quem encaminha para o `log_ip`.
        teste: Some("logger -p daemon.err \"netmonitor: teste de envio de log\""),
    },
    Receita {
        sistema: "linux",
        label: "Linux (rsyslog)",
        note: "Um único `@` usa UDP; `@@` usa TCP. As duas formas são aceitas — o servidor \
               escuta nos dois protocolos. Na ativação automática, o usuário informado precisa \
               poder usar `sudo`.",
        comandos: |alvo, porta| {
            vec![
                format!("echo '*.* @{alvo}:{porta}' | sudo tee /etc/rsyslog.d/60-netmonitor.conf"),
                "sudo systemctl restart rsyslog".to_owned(),
            ]
        },
        teste: Some("logger -p daemon.err \"netmonitor: teste de envio de log\""),
    },
    Receita {
        sistema: "ubiquiti",
        label: "Ubiquiti EdgeOS",
        note: "No UniFi, o mesmo ajuste fica em Configurações → Sistema → Registro remoto, \
               apontando para o mesmo endereço e porta.",
        comandos: |alvo, porta| {
            vec![
                "configure".to_owned(),
                format!("set system syslog host {alvo} facility all level info"),
                format!("set system syslog host {alvo} port {porta}"),
                "commit".to_owned(),
                "save".to_owned(),
                "exit".to_owned(),
            ]
        },
        // Fora do modo `configure`, que os comandos acima já fecharam.
        teste: Some("logger -p daemon.err \"netmonitor: teste de envio de log\""),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn o_endereco_entra_em_todos_os_snippets() {
        std::env::remove_var(ENV_PUBLISHED_PORT);
        let guia = build("192.168.1.10");
        assert_eq!(guia.server_address, "192.168.1.10");
        assert_eq!(guia.snippets.len(), 4);
        for snippet in &guia.snippets {
            assert!(
                snippet.commands.contains("192.168.1.10"),
                "{} não recebeu o endereço",
                snippet.system
            );
        }
    }

    #[test]
    #[serial]
    fn a_porta_do_snippet_e_a_publicada_e_nunca_a_interna() {
        // 5514 é a porta de dentro do container; quem a puser no roteador não
        // recebe nada — a não ser que o compose diga o contrário.
        std::env::remove_var(ENV_PUBLISHED_PORT);
        let guia = build("10.0.0.5");
        assert_eq!(guia.port, 514);
        for snippet in &guia.snippets {
            assert!(
                !snippet.commands.contains("5514"),
                "{} vazou a porta interna",
                snippet.system
            );
        }
    }

    #[test]
    #[serial]
    fn a_porta_publicada_acompanha_o_compose() {
        // Em `network_mode: host` não há mapeamento e a porta real é a 5514.
        // O snippet precisa dizer 5514, ou manda o roteador para o vazio.
        std::env::set_var(ENV_PUBLISHED_PORT, "5514");
        let guia = build("10.0.0.5");
        assert_eq!(guia.port, 5514);
        assert!(guia.snippets[0].commands.contains("remote-port=5514"));
        std::env::remove_var(ENV_PUBLISHED_PORT);
    }

    #[test]
    #[serial]
    fn porta_invalida_no_ambiente_cai_no_padrao() {
        for invalida in ["0", "-1", "porta", ""] {
            std::env::set_var(ENV_PUBLISHED_PORT, invalida);
            assert_eq!(published_port(), 514, "aceitou {invalida:?}");
        }
        std::env::remove_var(ENV_PUBLISHED_PORT);
    }

    #[test]
    #[serial]
    fn endereco_desconhecido_vira_marcador_e_nao_string_vazia() {
        std::env::remove_var(ENV_PUBLISHED_PORT);
        for entrada in ["", "   "] {
            let guia = build(entrada);
            assert_eq!(guia.server_address, ALVO_DESCONHECIDO);
            assert!(guia.snippets[0].commands.contains(ALVO_DESCONHECIDO));
        }
    }

    #[test]
    #[serial]
    fn o_routeros_insiste_no_bsd_syslog() {
        std::env::remove_var(ENV_PUBLISHED_PORT);
        let guia = build("10.0.0.5");
        let routeros = guia
            .snippets
            .iter()
            .find(|s| s.system == "routeros")
            .expect("routeros");
        assert!(routeros.commands.contains("bsd-syslog=yes"));
        assert!(routeros.note.contains("src-address"));
    }

    #[test]
    fn a_receita_da_tela_e_a_receita_do_ssh() {
        // Duas listas divergiriam na primeira correção, e a divergência só
        // apareceria no equipamento de alguém.
        let guia = build("10.0.0.5");
        for snippet in &guia.snippets {
            let comandos = commands_for(&snippet.system, "10.0.0.5", guia.port)
                .expect("receita do fabricante");
            assert_eq!(
                comandos.join("\n"),
                snippet.commands,
                "{} divergiu entre a tela e a ativação automática",
                snippet.system
            );
        }
    }

    #[test]
    fn cada_comando_da_receita_cabe_numa_linha() {
        // A sessão interativa manda uma linha por comando: uma receita com
        // quebra de linha embutida chegaria picotada ao equipamento.
        for sistema in systems() {
            for comando in commands_for(sistema, "10.0.0.5", 514).expect("receita") {
                assert!(
                    !comando.contains('\n') && !comando.contains('\\'),
                    "{sistema} tem comando multilinha: {comando:?}"
                );
            }
        }
    }

    #[test]
    fn fabricante_desconhecido_nao_devolve_receita_vazia() {
        // Lista vazia rodaria "com sucesso" sem configurar nada.
        assert!(commands_for("cisco", "10.0.0.5", 514).is_none());
        // Sistemas do catálogo sem receita caem aqui pelo mesmo caminho.
        assert!(commands_for("windows", "10.0.0.5", 514).is_none());
        assert!(commands_for("other", "10.0.0.5", 514).is_none());
        assert!(commands_for("", "10.0.0.5", 514).is_none());
        // O nome chega da tela, e caixa não pode decidir se funciona.
        assert!(commands_for("RouterOS", "10.0.0.5", 514).is_some());
    }
}
