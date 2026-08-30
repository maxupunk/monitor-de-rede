//! Parse de uma linha de syslog, com as correções medidas no SPIKE-06.
//!
//! Base: `syslog_loose`, o parser permissivo do Vector. Ele **não devolve
//! `Result`** — linha que não casa com RFC 3164 nem 5424 volta com os campos
//! vazios e a entrada inteira em `msg`. É o contrato certo para um receptor que
//! aceita o que o roteador mandar; nada é descartado por não ser RFC.
//!
//! Por cima dele, duas correções próprias — as duas descobertas na
//! [ADR 008](../../../../docs/adr/008-syslog-parser.md), as duas indispensáveis
//! para RouterOS:
//!
//! 1. **Resgate do `<pri>`** ([`resgata_pri`]): sem `bsd-syslog=yes` o RouterOS
//!    manda formato próprio, sem timestamp — o parser joga tudo em `msg` e a
//!    severidade se perde dentro do texto.
//! 2. **Severidade pelos tópicos**: o adapter RouterOS corrige o `<pri>` fixo
//!    da *action*. O parser apenas aplica os enriquecimentos registrados.

use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Utc};
use syslog_loose::{
    decompose_pri, IncompleteDate, Message, ProcId, SyslogFacility, SyslogSeverity, Variant,
};

use crate::services::devices::adapters::registry;

/// Uma linha já no vocabulário da tabela `device_logs`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedLog {
    pub facility: Option<i16>,
    pub severity: Option<i16>,
    pub device_time: Option<DateTime<FixedOffset>>,
    pub hostname: Option<String>,
    pub app_name: Option<String>,
    pub pid: Option<i32>,
    pub topics: Option<String>,
    pub message: String,
}

/// Parseia uma linha recebida em `recebido_em`.
///
/// `recebido_em` não é enfeite: é a âncora do resolvedor de ano do RFC 3164 —
/// e é a razão de `received_at` ser a verdade do sistema, não `device_time`.
#[must_use]
pub fn parse(bruto: &str, recebido_em: DateTime<Utc>) -> ParsedLog {
    let bruto = bruto.trim_end_matches(['\r', '\n', '\0']);
    let resolvedor = |incompleta: IncompleteDate| ano_mais_proximo(incompleta, recebido_em);
    let mensagem = syslog_loose::parse_message_with_year(bruto, resolvedor, Variant::Either);

    if mensagem.severity.is_none() {
        if let Some((facility, severity, resto)) = resgata_pri(bruto) {
            let mut linha = converte(syslog_loose::parse_message_with_year(
                resto,
                resolvedor,
                Variant::Either,
            ));
            linha.facility = facility.map(|valor| valor as i16);
            linha.severity = linha.severity.or_else(|| severity.map(|v| v as i16));
            // Sem timestamp na linha crua, o reparse devolve a linha toda menos
            // o pri como mensagem: os tópicos ficam no começo dela.
            if linha.topics.is_none() {
                if let Some((tag, resto)) = linha.message.split_once(' ') {
                    if let Some(topicos) = registry::syslog_topics(Some(tag)) {
                        linha.severity = registry::syslog_severity(&topicos).or(linha.severity);
                        linha.topics = Some(topicos);
                        linha.message = resto.to_owned();
                    }
                }
            }
            return linha;
        }
    }

    converte(mensagem)
}

fn converte(mensagem: Message<&str>) -> ParsedLog {
    let topics = registry::syslog_topics(mensagem.appname);
    let severity = topics
        .as_deref()
        .and_then(registry::syslog_severity)
        .or_else(|| mensagem.severity.map(|valor| valor as i16));
    ParsedLog {
        facility: mensagem.facility.map(|valor| valor as i16),
        severity,
        device_time: mensagem.timestamp,
        hostname: mensagem.hostname.map(str::to_owned),
        // Tópico do RouterOS não é nome de aplicação: ou é um, ou é outro.
        app_name: if topics.is_some() {
            None
        } else {
            mensagem.appname.map(str::to_owned)
        },
        pid: match mensagem.procid {
            Some(ProcId::PID(pid)) => Some(pid),
            _ => None,
        },
        topics,
        message: mensagem.msg.to_owned(),
    }
}

/// Extrai `<n>` do início da linha. `191` é o maior pri válido (23 × 8 + 7).
fn resgata_pri(bruto: &str) -> Option<(Option<SyslogFacility>, Option<SyslogSeverity>, &str)> {
    let resto = bruto.strip_prefix('<')?;
    let (numero, resto) = resto.split_once('>')?;
    let pri: u8 = numero.parse().ok()?;
    let (facility, severity) = decompose_pri(pri);
    Some((facility, severity, resto))
}

/// Escolhe o ano que deixa a data **mais perto** de `referencia`.
///
/// O RFC 3164 não manda o ano. Assumir o corrente erra na virada: uma mensagem
/// de 31/dez 23:59 recebida em 01/jan 00:00 iria parar doze meses no futuro e
/// sumiria de qualquer filtro por período. Testar os três anos candidatos e
/// ficar com o mais próximo resolve as duas direções.
///
/// Data inválida no ano candidato (29/fev fora de bissexto) é descartada pelo
/// `single()`; se nenhuma valer, cai no ano da referência.
#[must_use]
pub fn ano_mais_proximo(incompleta: IncompleteDate, referencia: DateTime<Utc>) -> i32 {
    let (mes, dia, hora, minuto, segundo) = incompleta;
    let base = referencia.year();
    [base - 1, base, base + 1]
        .into_iter()
        .filter_map(|ano| {
            Utc.with_ymd_and_hms(ano, mes, dia, hora, minuto, segundo)
                .single()
                .map(|instante| (ano, (instante - referencia).num_seconds().abs()))
        })
        .min_by_key(|(_, distancia)| *distancia)
        .map_or(base, |(ano, _)| ano)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agora() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 15, 10, 30, 0).unwrap()
    }

    #[test]
    fn routeros_com_bsd_syslog_rende_topicos_hostname_e_horario() {
        let linha = parse(
            "<134>Aug 15 10:23:45 MikroTik-CCR system,info,account user admin logged in via winbox",
            agora(),
        );
        assert_eq!(linha.topics.as_deref(), Some("system,info,account"));
        assert_eq!(linha.hostname.as_deref(), Some("MikroTik-CCR"));
        assert_eq!(linha.message, "user admin logged in via winbox");
        assert_eq!(linha.severity, Some(6));
        assert_eq!(linha.facility, Some(16));
        assert!(linha.device_time.is_some());
        // Tópico não é nome de aplicação.
        assert_eq!(linha.app_name, None);
    }

    #[test]
    fn a_severidade_vem_do_topico_e_nao_do_pri() {
        // `<131>` diz err (3); os tópicos dizem `error,critical`. Vence o mais
        // grave. Sem isto, todo log de um parque MikroTik seria "info" e o
        // filtro por severidade não separaria nada.
        let linha = parse(
            "<131>Aug 15 10:24:01 MikroTik-CCR system,error,critical login failure for user admin",
            agora(),
        );
        assert_eq!(linha.severity, Some(2), "critical");
    }

    #[test]
    fn routeros_sem_bsd_syslog_ainda_rende_severidade_e_topicos() {
        // Formato próprio: `<pri>` colado nos tópicos, sem timestamp e sem
        // hostname. O `syslog_loose` sozinho jogaria a linha inteira em `msg`.
        let linha = parse(
            "<134>system,info,account user admin logged in from 192.168.88.50 via winbox",
            agora(),
        );
        assert_eq!(linha.facility, Some(16));
        assert_eq!(linha.severity, Some(6));
        assert_eq!(linha.topics.as_deref(), Some("system,info,account"));
        assert_eq!(
            linha.message,
            "user admin logged in from 192.168.88.50 via winbox"
        );
        // O que se perde sem a flag — e só isto.
        assert_eq!(linha.device_time, None);
        assert_eq!(linha.hostname, None);
    }

    #[test]
    fn openwrt_e_linux_rendem_app_e_pid() {
        let linha = parse(
            "<30>Aug 15 10:30:45 OpenWrt dnsmasq-dhcp[1834]: DHCPACK(br-lan) 192.168.1.140",
            agora(),
        );
        assert_eq!(linha.app_name.as_deref(), Some("dnsmasq-dhcp"));
        assert_eq!(linha.pid, Some(1834));
        assert_eq!(linha.topics, None, "vírgula nenhuma, logo não é RouterOS");

        let rfc5424 = parse(
            "<165>1 2026-08-15T10:31:02.123456Z servidor sshd 4711 ID47 - Accepted publickey",
            agora(),
        );
        assert_eq!(rfc5424.app_name.as_deref(), Some("sshd"));
        assert_eq!(rfc5424.pid, Some(4711));
        assert_eq!(rfc5424.message, "Accepted publickey");
    }

    #[test]
    fn linha_sem_formato_vira_mensagem_inteira_e_nao_e_descartada() {
        let linha = parse("isto não é syslog de coisa alguma", agora());
        assert_eq!(linha.message, "isto não é syslog de coisa alguma");
        assert_eq!(linha.severity, None);
        assert_eq!(linha.facility, None);
    }

    #[test]
    fn a_quebra_de_linha_do_tcp_nao_entra_na_mensagem() {
        let linha = parse("<134>Aug 15 10:23:45 host app: mensagem\r\n", agora());
        assert_eq!(linha.message, "mensagem");
    }

    #[test]
    fn o_ano_ausente_do_rfc3164_escolhe_a_data_mais_proxima() {
        // Virada para frente: 31/dez recebido já em janeiro é do ano anterior.
        let virada = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 30).unwrap();
        assert_eq!(ano_mais_proximo((12, 31, 23, 59, 50), virada), 2026);
        // Virada para trás: relógio do roteador adiantado.
        let vespera = Utc.with_ymd_and_hms(2026, 12, 31, 23, 59, 30).unwrap();
        assert_eq!(ano_mais_proximo((1, 1, 0, 0, 10), vespera), 2027);
        // Caso comum: mesmo ano.
        assert_eq!(ano_mais_proximo((8, 15, 10, 0, 0), agora()), 2026);
    }

    #[test]
    fn o_29_de_fevereiro_nao_quebra_o_resolvedor() {
        // 2026 e 2027 não são bissextos; 2028 é. O candidato inválido é
        // descartado em vez de derrubar o parse.
        let referencia = Utc.with_ymd_and_hms(2027, 6, 1, 0, 0, 0).unwrap();
        assert_eq!(ano_mais_proximo((2, 29, 12, 0, 0), referencia), 2028);
    }

    #[test]
    fn topico_so_e_topico_quando_parece_com_um() {
        assert_eq!(
            registry::syslog_topics(Some("system,info")).as_deref(),
            Some("system,info")
        );
        assert_eq!(registry::syslog_topics(Some("sshd")), None);
        // Segmento vazio e caractere fora do alfabeto derrubam o palpite.
        assert_eq!(registry::syslog_topics(Some("system,,info")), None);
        assert_eq!(registry::syslog_topics(Some("a b,c")), None);
        assert_eq!(registry::syslog_topics(None), None);
    }
}
