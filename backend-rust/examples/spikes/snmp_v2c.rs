//! **SPIKE-01 — Cliente SNMP** (§3.4 do roadmap).
//!
//! Pergunta: `rasn-snmp` cobre `get` e `walk` (GETNEXT) com transporte próprio
//! em `tokio::net::UdpSocket`, ou é preciso cair no `snmp2` síncrono dentro de
//! `spawn_blocking`?
//!
//! O protótipo roda em dois modos:
//!
//! * **offline** (padrão): faz o round-trip de codificação BER de um
//!   `GetRequest` e de um `Response`, provando que o codec basta para montar e
//!   ler PDUs sem nenhum agente na rede. É o que roda em CI.
//! * **ao vivo**: com `SNMP_TARGET=<host:porta>` definido, lê `sysDescr.0` e
//!   percorre `ifDescr` por GETNEXT contra um agente real.
//!
//! ```sh
//! cargo run --example spike_snmp_v2c
//! SNMP_TARGET=192.168.0.1:161 SNMP_COMMUNITY=public cargo run --example spike_snmp_v2c
//! ```

use std::time::Duration;

use rasn::types::ObjectIdentifier;
use rasn_snmp::{
    v2::{GetNextRequest, GetRequest, Pdu, Pdus, VarBind, VarBindValue},
    v2c::Message,
};
use tokio::net::UdpSocket;

/// `1.3.6.1.2.1.1.1.0` — `sysDescr.0`.
const SYS_DESCR: &[u32] = &[1, 3, 6, 1, 2, 1, 1, 1, 0];
/// `1.3.6.1.2.1.2.2.1.2` — coluna `ifDescr` da `ifTable`.
const IF_DESCR: &[u32] = &[1, 3, 6, 1, 2, 1, 2, 2, 1, 2];

const TIMEOUT: Duration = Duration::from_secs(3);
/// O agente não pode responder mais do que a MTU típica sem fragmentar; 64 KiB
/// cobre com folga qualquer resposta de GETNEXT.
const MAX_RESPONSE: usize = 65_535;

fn oid(partes: &[u32]) -> ObjectIdentifier {
    ObjectIdentifier::new_unchecked(partes.to_vec().into())
}

/// **Achado do SPIKE-01:** `EncodeError` e `DecodeError` do `rasn` 0.18 não
/// implementam `std::error::Error`, então `?` não converte para
/// `Box<dyn Error>` nem para `anyhow`. O cliente definitivo precisa de um
/// `SnmpError` (`thiserror`) com `From` explícito — não dá para propagar
/// direto. Aqui a conversão vira texto.
fn erro(contexto: &str, causa: impl core::fmt::Display) -> Box<dyn std::error::Error> {
    format!("{contexto}: {causa}").into()
}

fn codifique<T: rasn::Encode>(msg: &T) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    rasn::ber::encode(msg).map_err(|e| erro("codificar BER", e))
}

fn decodifique(bytes: &[u8]) -> Result<Message<Pdus>, Box<dyn std::error::Error>> {
    rasn::ber::decode(bytes).map_err(|e| erro("decodificar BER", e))
}

/// Monta a mensagem v2c. `version = 1` é SNMPv2c (v1 seria 0) — a numeração da
/// RFC 1901 é deslocada em um, e trocar isso silenciosamente faz o agente
/// descartar o pacote sem erro.
fn mensagem<T>(comunidade: &str, data: T) -> Message<T> {
    Message {
        version: 1.into(),
        community: comunidade.as_bytes().to_vec().into(),
        data,
    }
}

fn pdu(request_id: i32, alvo: &[u32]) -> Pdu {
    Pdu {
        request_id,
        error_status: Pdu::ERROR_STATUS_NO_ERROR,
        error_index: 0,
        variable_bindings: vec![VarBind {
            name: oid(alvo),
            // Numa requisição o valor vai vazio; quem preenche é o agente.
            value: VarBindValue::Unspecified,
        }],
    }
}

/// Prova que codificar e decodificar fecha o ciclo — sem rede, sem agente.
fn round_trip_offline() -> Result<(), Box<dyn std::error::Error>> {
    let requisicao = mensagem("public", GetRequest(pdu(42, SYS_DESCR)));
    let bytes = codifique(&requisicao)?;
    println!(
        "[ok] GetRequest de sysDescr.0 codificado em {} bytes",
        bytes.len()
    );

    // O agente responde com um `Pdus` (choice) — é assim que o cliente real vai
    // decodificar, porque ele não sabe de antemão qual PDU chega.
    let decodificada = decodifique(&bytes)?;
    assert_eq!(decodificada.community.as_ref(), b"public");

    let Pdus::GetRequest(GetRequest(pdu_lido)) = decodificada.data else {
        return Err("PDU decodificado não é um GetRequest".into());
    };
    assert_eq!(pdu_lido.request_id, 42);
    assert_eq!(pdu_lido.variable_bindings[0].name, oid(SYS_DESCR));
    println!("[ok] round-trip BER preserva request_id, community e OID");

    let proximo = mensagem("public", GetNextRequest(pdu(43, IF_DESCR)));
    let bytes = codifique(&proximo)?;
    decodifique(&bytes)?;
    println!("[ok] GetNextRequest (base do walk) também fecha o ciclo");

    Ok(())
}

async fn conversar(
    socket: &UdpSocket,
    requisicao: &[u8],
) -> Result<Message<Pdus>, Box<dyn std::error::Error>> {
    socket.send(requisicao).await?;
    let mut buffer = vec![0u8; MAX_RESPONSE];
    let lidos = tokio::time::timeout(TIMEOUT, socket.recv(&mut buffer)).await??;
    decodifique(&buffer[..lidos])
}

fn valor_em_texto(bind: &VarBind) -> String {
    match &bind.value {
        VarBindValue::Value(syntax) => format!("{syntax:?}"),
        outro => format!("{outro:?}"),
    }
}

async fn ao_vivo(alvo: &str, comunidade: &str) -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(alvo).await?;
    println!("\n-- modo ao vivo contra {alvo} --");

    // GET simples.
    let bytes = codifique(&mensagem(comunidade, GetRequest(pdu(1, SYS_DESCR))))?;
    let resposta = conversar(&socket, &bytes).await?;
    if let Pdus::Response(r) = &resposta.data {
        for bind in &r.0.variable_bindings {
            println!("[get] sysDescr.0 = {}", valor_em_texto(bind));
        }
    }

    // Walk por GETNEXT: para quando o OID devolvido sai de baixo do prefixo.
    let mut atual = IF_DESCR.to_vec();
    for i in 0..64 {
        let bytes = codifique(&mensagem(comunidade, GetNextRequest(pdu(1000 + i, &atual))))?;
        let resposta = conversar(&socket, &bytes).await?;

        let Pdus::Response(r) = &resposta.data else {
            return Err("agente respondeu algo que não é Response".into());
        };
        let Some(bind) = r.0.variable_bindings.first() else {
            break;
        };

        let devolvido: Vec<u32> = bind.name.iter().copied().collect();
        if !devolvido.starts_with(IF_DESCR) {
            println!("[walk] fim da coluna ifDescr após {i} interfaces");
            break;
        }

        println!("[walk] {devolvido:?} = {}", valor_em_texto(bind));
        atual = devolvido;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    round_trip_offline()?;

    match std::env::var("SNMP_TARGET") {
        Ok(alvo) => {
            let comunidade =
                std::env::var("SNMP_COMMUNITY").unwrap_or_else(|_| "public".to_string());
            ao_vivo(&alvo, &comunidade).await?;
        }
        Err(_) => {
            println!("\n(defina SNMP_TARGET=host:161 para rodar contra um agente real)");
        }
    }

    println!("\nSPIKE-01: rasn-snmp cobre get e walk com transporte tokio próprio.");
    Ok(())
}
