//! Alvo de teste ativo precisa ter origem.
//!
//! Sem dado nenhum, um modelo pequeno "propõe" um ping em `10.0.0.1`,
//! `10.0.0.2` e `10.0.0.3` — endereços que ele não tem como conhecer — e o
//! usuário recebe três pedidos de confirmação inúteis (ou três testes contra
//! nada, sem a confirmação). Endereço interno só é testado se veio da
//! conversa (o usuário escreveu, uma consulta devolveu, o administrador pôs no
//! prompt) ou está cadastrado no inventário. Alvo público (`8.8.8.8`,
//! `google.com`) é conhecimento geral e segue livre, como o loopback.

use std::net::IpAddr;

use sea_orm::{ConnectionTrait, EntityTrait};

use crate::{
    models::{devices, monitors},
    services::{ai::drivers::traits::AiMessage, shared::errors::AppResult},
};

/// Sufixos de nome que só resolvem dentro da rede do usuário.
const LOCAL_SUFFIXES: &[&str] = &[
    ".local",
    ".lan",
    ".home",
    ".internal",
    ".intranet",
    ".localdomain",
    ".corp",
    ".home.arpa",
];

/// Chaves da configuração de um monitor que guardam o alvo.
const MONITOR_TARGET_KEYS: &[&str] = &["host", "domain", "url"];

/// Minúsculas, sem espaços nem colchetes de IPv6.
fn normalize(target: &str) -> String {
    target
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_lowercase()
}

/// Endereço que só existe na rede do usuário: o modelo não tem como sabê-lo
/// sozinho. Loopback não entra — testar a própria central é inofensivo.
#[must_use]
pub fn is_internal(target: &str) -> bool {
    let target = normalize(target);
    match target.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            let [first, second, ..] = ip.octets();
            ip.is_private()
                || ip.is_link_local()
                || ip.is_unspecified()
                // 100.64.0.0/10: CGNAT, o endereço que o provedor dá ao cliente.
                || (first == 100 && (64..128).contains(&second))
        }
        Ok(IpAddr::V6(ip)) => {
            let first = ip.segments()[0];
            ip.is_unspecified() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
        Err(_) => {
            target != "localhost"
                && (!target.contains('.')
                    || LOCAL_SUFFIXES.iter().any(|suffix| target.ends_with(suffix)))
        }
    }
}

/// Caractere que continua um IP ou hostname.
fn continues_host(c: char) -> bool {
    c.is_alphanumeric() || c == '.' || c == '-'
}

/// O texto cita o alvo como palavra inteira: `10.0.0.1` não casa dentro de
/// `10.0.0.12`, mas casa em `"ip":"10.0.0.1"`, `10.0.0.1:161` e no fim de uma
/// frase (`... o 10.0.0.1.`).
#[must_use]
pub fn mentions(text: &str, target: &str) -> bool {
    let target = normalize(target);
    if target.is_empty() {
        return false;
    }
    let text = text.to_lowercase();
    text.match_indices(&target).any(|(start, found)| {
        let before = text[..start].chars().next_back();
        let mut after = text[start + found.len()..].chars();
        let starts_clean = before.is_none_or(|c| !continues_host(c));
        let ends_clean = match after.next() {
            None => true,
            // Ponto ou hífen de pontuação: o que vem depois não é mais nome.
            Some('.' | '-') => after.next().is_none_or(|c| !c.is_alphanumeric()),
            Some(c) => !continues_host(c),
        };
        starts_clean && ends_clean
    })
}

/// O alvo apareceu no que a IA leu nesta sessão: pedido do usuário (com o
/// `<contexto>`), resultado de consulta ou o system prompt. O que a própria IA
/// escreveu não conta — é exatamente o que se quer conferir.
fn grounded_in_conversation(conversation: &[AiMessage], target: &str) -> bool {
    conversation
        .iter()
        .filter(|message| message.role != "assistant")
        .filter_map(|message| message.content.as_deref())
        .any(|content| mentions(content, target))
}

/// O alvo é o IP de um dispositivo ou o alvo de um monitor cadastrado (o
/// resultado da consulta pode ter sido podado da conversa).
async fn registered<C: ConnectionTrait>(db: &C, target: &str) -> AppResult<bool> {
    let wanted = normalize(target);
    let same = |value: &str| {
        let value = normalize(value);
        value == wanted || mentions(&value, &wanted)
    };
    let devices = devices::Entity::find().all(db).await?;
    if devices
        .iter()
        .filter_map(|device| device.ip_address.as_deref())
        .any(same)
    {
        return Ok(true);
    }
    let monitors = monitors::Entity::find().all(db).await?;
    Ok(monitors.iter().any(|monitor| {
        MONITOR_TARGET_KEYS
            .iter()
            .filter_map(|key| monitor.configuration.get(key)?.as_str())
            .any(same)
    }))
}

/// Mensagem devolvida à IA quando o alvo foi inventado; `None` quando ele tem
/// origem. Falha do banco não bloqueia: o teste segue o caminho normal.
pub async fn invented_target<C: ConnectionTrait>(
    db: &C,
    conversation: &[AiMessage],
    target: &str,
) -> Option<String> {
    if !is_internal(target) || grounded_in_conversation(conversation, target) {
        return None;
    }
    if registered(db, target).await.unwrap_or(true) {
        return None;
    }
    Some(format!(
        "O alvo '{}' é um endereço interno que não veio do usuário nem de uma consulta: não invente alvos. Use os IPs de list_devices ou list_monitors, ou pergunte ao usuário com ask_user.",
        target.trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> AiMessage {
        AiMessage {
            role: role.into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    #[test]
    fn interno_e_o_que_so_existe_na_rede_do_usuario() {
        for alvo in [
            "10.0.0.1",
            "192.168.1.1",
            "172.16.4.2",
            "100.64.0.9",
            "169.254.1.1",
            "fd00::1",
            "[fe80::1]",
            "roteador",
            "nas.lan",
            "srv01.empresa.local",
        ] {
            assert!(is_internal(alvo), "{alvo}");
        }
        for alvo in [
            "8.8.8.8",
            "1.1.1.1",
            "google.com",
            "127.0.0.1",
            "localhost",
            "::1",
            "2001:4860:4860::8888",
        ] {
            assert!(!is_internal(alvo), "{alvo}");
        }
    }

    #[test]
    fn citacao_casa_so_a_palavra_inteira() {
        assert!(mentions("pinga o 10.0.0.1 agora", "10.0.0.1"));
        assert!(mentions("pinga o 10.0.0.1.", "10.0.0.1"));
        assert!(mentions(r#"{"ip":"10.0.0.1"}"#, "10.0.0.1"));
        assert!(mentions("snmp em 10.0.0.1:161", "10.0.0.1"));
        assert!(mentions("Testa o NAS.lan", "nas.lan"));
        assert!(!mentions("o 10.0.0.12 caiu", "10.0.0.1"));
        assert!(!mentions("o 110.0.0.1 caiu", "10.0.0.1"));
        assert!(!mentions("o 10.0.0.1.5 caiu", "10.0.0.1"));
        assert!(!mentions("qualquer coisa", ""));
    }

    #[test]
    fn origem_e_o_que_a_ia_leu_nao_o_que_ela_escreveu() {
        let conversa = [
            msg("system", "Você é o NetMonitor AI."),
            msg("user", "quais ping estão instáveis?"),
            msg("assistant", "Vou pingar 10.0.0.1"),
            msg(
                "tool",
                r#"{"devices":{"columns":["ip"],"rows":[["192.168.0.10"]]}}"#,
            ),
        ];
        assert!(!grounded_in_conversation(&conversa, "10.0.0.1"));
        assert!(grounded_in_conversation(&conversa, "192.168.0.10"));
    }
}
