//! O `vpn-probe`: o agente dedicado que mede ICMP/SNMP de dentro do namespace
//! de rede do WireGuard (AGENTS §6).
//!
//! Mora no núcleo porque é um **probe** — o registro no boot, a troca de banco
//! e a autenticação pelo token padrão são do núcleo — que a área VPN também
//! consulta (atribuição de monitores, leitura de um ping que falha).

/// Nome do probe dedicado que compartilha o namespace de rede do WireGuard.
#[must_use]
pub fn vpn_probe_name() -> String {
    std::env::var("VPN_PROBE_NAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "vpn-probe".to_string())
}

/// O túnel sobe em **outro** namespace de rede, alcançável só pelo agente
/// dedicado (`vpn-probe`)?
///
/// `false` — o padrão — descreve a topologia de um container só: o WireGuard
/// sobe ao lado da API e a `wg0` é do próprio processo, que portanto alcança a
/// faixa do túnel sem intermediário.
///
/// Duas decisões dependem desta resposta e precisam ser a mesma:
///
/// * registrar (ou não) o `vpn-probe` no boot — `initializers::monitoring`;
/// * como ler um ping que falha com o túnel de pé — `peer_hints`, na área VPN.
#[must_use]
pub fn probe_is_external() -> bool {
    std::env::var("VPN_PROBE_EXTERNAL").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    })
}
