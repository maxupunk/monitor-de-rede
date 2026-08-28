//! **Monitor de alcance**: o que é, e para quem ele faz sentido existir.
//!
//! Um monitor de alcance pergunta "este equipamento responde pela rede?" —
//! `ping`, `tcp`, `http`, `https` e `dns`. A pergunta parece de controller, mas
//! não é: ela tem uma resposta só, e antes deste módulo ela era respondida em
//! três lugares — `sync_device_monitor`, `vpn::monitor_provisioner::provision`
//! e `POST /api/monitors` — que já **divergiam** em dois.
//!
//! # As duas regras, e por que elas são a mesma regra
//!
//! 1. **O dispositivo do sistema não é alcançado pela rede.** Um ping do host
//!    para si mesmo responde sempre e não informa nada; o que mede a saúde do
//!    servidor é a coleta local `system_health`.
//! 2. **Sem endereço, não há alvo.** Os dois provisionadores automáticos caíam
//!    para `device.name` quando `ip_address` era nulo — uma checagem contra um
//!    rótulo, que só pode falhar e deixa o equipamento `offline` para sempre.
//!
//! A segunda é a **causa** da primeira: o ping do servidor nasceu justamente
//! desse fallback, porque o dispositivo do sistema é o caso extremo do
//! dispositivo sem IP. Corrigir só a primeira deixaria o problema de pé para
//! todo dispositivo cadastrado sem endereço.
//!
//! # Divisão de responsabilidades
//!
//! - [`is_reach_check`] classifica um tipo — pergunta pura, sem banco.
//! - [`ensure_allowed_for_device`] é a guarda dos **quatro** caminhos de
//!   criação/edição. Recusa por regra de negócio, em português, dizendo por quê.
//! - [`auto_target`] é o alvo do provisionamento automático. `None` quando não
//!   há endereço — e aí não existe monitor a criar, em vez de um alvo inventado.
//! - [`purge_system_device`] desfaz o que versões anteriores gravaram, no boot.

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::{
    models::{devices, monitors},
    services::{
        devices::system_device,
        maintenance::resource_cleanup::ResourceCleanupService,
        shared::errors::{AppError, AppResult},
    },
};

/// Os tipos que medem alcance pela rede.
///
/// `snmp`, `ssl` e `port_scan` ficam de fora de propósito: o primeiro coleta
/// leituras de um agente, os outros dois inspecionam um serviço já alcançado.
/// Nenhum deles é a checagem "o equipamento está no ar" que esta lista define.
const REACH_TYPES: [&str; 5] = ["ping", "tcp", "http", "https", "dns"];

/// Verdadeiro para um tipo que mede alcance pela rede.
#[must_use]
pub fn is_reach_check(kind: &str) -> bool {
    REACH_TYPES
        .iter()
        .any(|conhecido| kind.trim().eq_ignore_ascii_case(conhecido))
}

/// Recusa um monitor de alcance apontado para um dispositivo que a rede não
/// alcança.
///
/// Vale para os quatro caminhos de criação. Um tipo que não mede alcance passa
/// sempre — é assim que a coleta de saúde do próprio servidor continua sendo
/// criada por este mesmo caminho.
///
/// # Errors
///
/// [`AppError::BusinessRule`] quando o dispositivo é o do sistema.
pub fn ensure_allowed_for_device(device: &devices::Model, kind: &str) -> AppResult<()> {
    if !is_reach_check(kind) {
        return Ok(());
    }
    if system_device::is_protected(device) {
        return Err(AppError::BusinessRule(format!(
            "{} representa esta instalação e não é alcançado pela rede: \
             um monitor de {} apontado para ele responderia sempre, sem medir nada. \
             A saúde do servidor é medida pela coleta local de saúde.",
            device.name,
            kind.trim().to_uppercase()
        )));
    }
    Ok(())
}

/// Alvo de um monitor de alcance **provisionado automaticamente**.
///
/// Só o endereço IP serve. O nome do dispositivo é rótulo de tela: usá-lo como
/// host gera uma resolução que falha, e um equipamento eternamente `offline`
/// por um monitor que ninguém pediu.
///
/// Um monitor criado à mão não passa por aqui — quem informa `url`, `domain` ou
/// `host` explicitamente está informando um alvo real.
#[must_use]
pub fn auto_target(device: &devices::Model) -> Option<String> {
    device
        .ip_address
        .as_deref()
        .map(str::trim)
        .filter(|endereco| !endereco.is_empty())
        .map(str::to_string)
}

/// Por que este dispositivo não recebe monitor de alcance automático.
///
/// `None` quando recebe. A mensagem é a mesma que a tela de cadastro mostra —
/// um texto só, para o backend e a interface não divergirem.
#[must_use]
pub fn auto_provisioning_blocked_reason(device: &devices::Model) -> Option<String> {
    if system_device::is_protected(device) {
        return Some(format!(
            "{} representa esta instalação e é medido pela coleta local de saúde, não por checagem de rede.",
            device.name
        ));
    }
    if auto_target(device).is_none() {
        return Some(
            "Sem endereço IP não há alvo para checar: o monitor de alcance só é criado depois que o endereço for informado."
                .to_string(),
        );
    }
    None
}

/// Remove do dispositivo do sistema qualquer monitor de alcance já gravado.
///
/// A guarda impede criar; ela não desfaz o que está no banco de quem atualizou
/// no meio do caminho. Idempotente: sem nada a remover, não faz nada.
///
/// A remoção passa pelo [`ResourceCleanupService`] — o mesmo caminho do
/// `DELETE /api/monitors/{id}` —, porque um monitor deixa histórico, métricas,
/// eventos e regras para trás, e órfãos desses são o tipo de sujeira que
/// reaparece meses depois como um alerta sem dono.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn purge_system_device(
    db: &DatabaseConnection,
    device: &devices::Model,
) -> AppResult<Vec<monitors::Model>> {
    if !system_device::is_protected(device) {
        return Ok(Vec::new());
    }

    let existentes = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device.id))
        .all(db)
        .await?;

    let mut removidos = Vec::new();
    for monitor in existentes {
        if !is_reach_check(&monitor.r#type) {
            continue;
        }
        // Apagar dado do operador em silêncio não é aceitável, mesmo quando o
        // dado é inútil: o log é o recibo de que o sistema — e não uma falha —
        // fez isso.
        tracing::info!(
            monitor_id = monitor.id,
            monitor_name = %monitor.name,
            monitor_type = %monitor.r#type,
            device_id = device.id,
            "monitor de alcance removido do dispositivo do sistema: ele não é alcançado pela rede"
        );
        ResourceCleanupService::delete_monitor(db, monitor.id).await?;
        removidos.push(monitor);
    }
    Ok(removidos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(system: bool, ip: Option<&str>) -> devices::Model {
        let agora = chrono::Utc::now();
        devices::Model {
            id: 1,
            site_id: None,
            network_id: None,
            parent_id: None,
            ip_address: ip.map(str::to_string),
            name: "Servidor NetMonitor".into(),
            r#type: "server".into(),
            vendor: None,
            model: None,
            serial_number: None,
            description: None,
            is_monitored: true,
            snmp_enabled: false,
            snmp_community: None,
            snmp_version: None,
            snmp_poll_interval_seconds: 60,
            access_mode: None,
            operating_system: None,
            syslog_server_address: None,
            system_key: system.then(|| system_device::NETMONITOR_KEY.to_string()),
            link_interface_id: None,
            link_interface_name: None,
            status: "unknown".into(),
            last_seen_at: None,
            created_at: agora.into(),
            updated_at: agora.into(),
        }
    }

    #[test]
    fn os_cinco_tipos_de_alcance_sao_reconhecidos_sem_depender_de_caixa() {
        for kind in ["ping", "TCP", "Http", "HTTPS", " dns "] {
            assert!(is_reach_check(kind), "{kind} deveria medir alcance");
        }
    }

    #[test]
    fn snmp_ssl_e_port_scan_nao_sao_monitores_de_alcance() {
        // Nenhum deles responde "o equipamento está no ar": o primeiro lê um
        // agente, os outros inspecionam um serviço já alcançado.
        for kind in ["snmp", "ssl", "port_scan", "system_health"] {
            assert!(!is_reach_check(kind), "{kind} não mede alcance");
        }
    }

    #[test]
    fn o_dispositivo_do_sistema_recusa_qualquer_monitor_de_alcance() {
        let servidor = device(true, None);
        for kind in ["ping", "tcp", "http", "https", "dns"] {
            let erro = ensure_allowed_for_device(&servidor, kind)
                .expect_err("o servidor não é alcançado pela rede");
            assert!(
                matches!(erro, AppError::BusinessRule(_)),
                "a recusa é de negócio, não validação de payload"
            );
        }
    }

    #[test]
    fn o_dispositivo_do_sistema_continua_aceitando_a_coleta_de_saude() {
        let servidor = device(true, None);
        assert!(ensure_allowed_for_device(&servidor, "system_health").is_ok());
        assert!(ensure_allowed_for_device(&servidor, "snmp").is_ok());
    }

    #[test]
    fn dispositivo_comum_aceita_monitor_de_alcance_com_ou_sem_ip() {
        // Sem IP a guarda não recusa: quem informa `url` ou `domain` à mão está
        // informando um alvo real. O que não acontece é o **provisionamento
        // automático** — e isso é `auto_target`, não esta função.
        assert!(ensure_allowed_for_device(&device(false, None), "http").is_ok());
        assert!(ensure_allowed_for_device(&device(false, Some("10.0.0.1")), "ping").is_ok());
    }

    #[test]
    fn o_alvo_automatico_nunca_cai_para_o_nome_do_dispositivo() {
        let sem_ip = device(false, None);
        assert_eq!(auto_target(&sem_ip), None);
        assert_eq!(auto_target(&device(false, Some("  "))), None);
        assert_eq!(
            auto_target(&device(false, Some(" 10.0.0.1 "))),
            Some("10.0.0.1".to_string())
        );
    }

    #[test]
    fn o_motivo_do_bloqueio_e_especifico_de_cada_causa() {
        assert!(auto_provisioning_blocked_reason(&device(false, Some("10.0.0.1"))).is_none());
        assert!(auto_provisioning_blocked_reason(&device(false, None))
            .is_some_and(|motivo| motivo.contains("endereço IP")));
        assert!(auto_provisioning_blocked_reason(&device(true, None))
            .is_some_and(|motivo| motivo.contains("coleta local de saúde")));
    }
}
