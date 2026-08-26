//! O que um dispositivo **de fato** oferece — a projeção que governa a tela.
//!
//! Uma única resposta do backend decide três coisas que antes eram decididas em
//! três lugares diferentes: quais abas `/devices/{id}` mostra, quais botões o
//! cabeçalho oferece e quais templates de alerta são aplicáveis. Ter uma fonte
//! só não é elegância: enquanto a aba de Interfaces SNMP e o botão "Coletar"
//! respondiam a critérios diferentes, existia um estado em que a tela oferecia
//! coletar de um equipamento que nunca respondeu.
//!
//! # A regra que decide tudo aqui: evidência, não configuração
//!
//! `devices.snmp_enabled` é campo de **cadastro** — significa "o operador
//! pretende usar SNMP", não "o equipamento responde". Uma aba de interfaces
//! aberta a partir dessa intenção mostra uma lista vazia e um botão de coleta
//! que só pode falhar. Por isso toda capacidade abaixo nasce de algo
//! **persistido por uma comunicação que aconteceu**: uma interface inventariada,
//! uma métrica gravada, um evento registrado, um log recebido.
//!
//! O estado "configurado, mas ainda não conectado" não some: ele vira
//! [`DeviceCapabilities::snmp_configured`] sem `snmp_connected`, e a Visão
//! Geral o apresenta como uma ação a executar.

use std::collections::HashSet;

use crate::{
    dtos::devices::DeviceCapabilities,
    models::{
        _entities::{alert_events, device_interfaces, metrics, vpn_peers},
        devices, monitors,
    },
    services::{
        alerts::fields,
        devices::system_device,
        monitoring::{health::series, managed, reachability},
        shared::errors::AppResult,
    },
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect,
};

impl DeviceCapabilities {
    /// Verdadeiro quando o dispositivo publica todos os campos exigidos.
    #[must_use]
    pub fn publishes(&self, required: &[&str]) -> bool {
        required
            .iter()
            .all(|campo| self.alert_fields.iter().any(|publicado| publicado == campo))
    }
}

/// Calcula as capacidades de um dispositivo.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn for_device(
    db: &DatabaseConnection,
    device: &devices::Model,
) -> AppResult<DeviceCapabilities> {
    let is_system = system_device::is_protected(device);

    let monitores = monitors::Entity::find()
        .filter(monitors::Column::DeviceId.eq(device.id))
        .all(db)
        .await?;

    // Séries já gravadas: é a prova de que a coleta funcionou pelo menos uma
    // vez. Uma consulta agregada, e não uma por série — a `metrics` é a tabela
    // de maior volume do sistema.
    let series_gravadas: HashSet<String> = metrics::Entity::find()
        .filter(metrics::Column::DeviceId.eq(device.id))
        .select_only()
        .column(metrics::Column::Name)
        .distinct()
        .into_tuple::<String>()
        .all(db)
        .await?
        .into_iter()
        .collect();

    let interfaces = device_interfaces::Entity::find()
        .filter(device_interfaces::Column::DeviceId.eq(device.id))
        .count(db)
        .await?
        > 0;

    // Conexão SNMP provada: ou temos inventário de interfaces, ou temos uma
    // série que só o SNMP grava. `snmp_enabled` sozinho nunca basta.
    let snmp_connected = device.snmp_enabled
        && (interfaces
            || series_gravadas.contains("snmp_uptime")
            || (monitores.iter().any(|m| m.r#type == "snmp")
                && (series_gravadas.contains(series::CPU_USAGE)
                    || series_gravadas.contains(series::MEMORY_USAGE))));

    let events = alert_events::Entity::find()
        .filter(alert_events::Column::DeviceId.eq(device.id))
        .count(db)
        .await?
        > 0;

    let vpn = vpn_peers::Entity::find()
        .filter(vpn_peers::Column::DeviceId.eq(device.id))
        .count(db)
        .await?
        > 0;

    // Logs é também o ponto de entrada para configurar ou retomar o Syslog.
    // Esconder a aba até existir uma declaração manual de sistema criava um
    // ciclo impossível: sistemas detectados automaticamente funcionavam, mas o
    // operador não tinha como voltar ao assistente depois de fechar o cadastro.
    let logs = true;

    let health = series_gravadas.contains(series::CPU_USAGE)
        || series_gravadas.contains(series::MEMORY_USAGE)
        || series_gravadas.contains(series::STORAGE_USAGE);

    let alert_fields = published_fields(&monitores, &series_gravadas);

    Ok(DeviceCapabilities {
        device_id: device.id,
        is_system,
        snmp_configured: device.snmp_enabled,
        snmp_connected,
        interfaces: interfaces && snmp_connected,
        events,
        logs,
        vpn,
        health,
        // As mesmas capacidades governam os botões. No dispositivo do sistema,
        // escanear as próprias portas ou editar IP e comunidade SNMP de um
        // equipamento protegido não são ações válidas — e um botão que só pode
        // devolver erro é pior que botão nenhum.
        can_snmp_scan: !is_system,
        can_snmp_collect: !is_system && snmp_connected,
        can_scan_ports: !is_system && device.ip_address.is_some(),
        can_edit_identity: !is_system,
        can_create_monitor: !is_system,
        reach_monitor_blocked_reason: reachability::auto_provisioning_blocked_reason(device),
        alert_fields,
    })
}

/// O vocabulário de alerta que os monitores e as séries do dispositivo
/// publicam.
///
/// Deriva do que existe, e não de uma tabela paralela: cada tipo de monitor
/// declara o que sabe medir, e as séries gravadas confirmam o que já foi medido
/// de fato. Um template pedindo um campo ausente daqui não é oferecido, e é
/// isso que impede a tela de sugerir "CPU acima de 85%" para um dispositivo que
/// só responde ping.
fn published_fields(
    monitores: &[monitors::Model],
    series_gravadas: &HashSet<String>,
) -> Vec<String> {
    let mut campos: Vec<&'static str> = Vec::new();
    let adicione = |campo: &'static str, campos: &mut Vec<&'static str>| {
        if !campos.contains(&campo) {
            campos.push(campo);
        }
    };

    for monitor in monitores {
        match monitor.r#type.to_lowercase().as_str() {
            "ping" => {
                adicione(fields::STATUS, &mut campos);
                adicione(fields::LATENCY_MS, &mut campos);
                adicione(fields::PACKET_LOSS, &mut campos);
            }
            "http" | "https" => {
                adicione(fields::STATUS, &mut campos);
                adicione(fields::LATENCY_MS, &mut campos);
                adicione(fields::STATUS_CODE, &mut campos);
                adicione(fields::DURATION_MS, &mut campos);
            }
            "tcp" => {
                adicione(fields::STATUS, &mut campos);
                adicione(fields::CONNECT_TIME_MS, &mut campos);
            }
            "dns" => {
                adicione(fields::STATUS, &mut campos);
                adicione(fields::RESOLUTION_TIME_MS, &mut campos);
            }
            "snmp" => {
                adicione(fields::STATUS, &mut campos);
                adicione(fields::SNMP_UPTIME, &mut campos);
                adicione(fields::IF_OPER_STATUS, &mut campos);
                adicione(fields::IF_SPEED, &mut campos);
            }
            // **Sem `STATUS`, e isso não é esquecimento.** O `status` de uma
            // coleta de saúde descreve a *coleta*, não o alcance do
            // equipamento: ela devolve `up` quando mediu algo e `unknown`
            // quando não conseguiu medir nada — nunca `down`. Um template de
            // "Dispositivo sem resposta" (`status == 'down'`) apareceria
            // aplicável e jamais dispararia; e o caso que ele descreveria — o
            // processo parado — é o que a seção 6 do roadmap declara fora de
            // escopo, porque um processo parado não alerta sobre si.
            managed::SYSTEM_HEALTH => {
                adicione(fields::CPU_USAGE_PERCENT, &mut campos);
                adicione(fields::MEMORY_USED_PERCENT, &mut campos);
                adicione(fields::STORAGE_USED_PERCENT, &mut campos);
                adicione(fields::LOAD_AVERAGE_1M, &mut campos);
            }
            _ => {
                adicione(fields::STATUS, &mut campos);
            }
        }
    }

    // O que já foi gravado confirma o que o monitor promete — e cobre o caso
    // do roteador SNMP cujo monitor de `cpu_usage` já produziu série.
    for (serie, campo) in [
        (series::CPU_USAGE, fields::CPU_USAGE_PERCENT),
        (series::MEMORY_USAGE, fields::MEMORY_USED_PERCENT),
        (series::STORAGE_USAGE, fields::STORAGE_USED_PERCENT),
        (series::LOAD_AVERAGE_1M, fields::LOAD_AVERAGE_1M),
        (series::IN_BPS, fields::IN_BPS),
        (series::OUT_BPS, fields::OUT_BPS),
    ] {
        if series_gravadas.contains(serie) {
            adicione(campo, &mut campos);
        }
    }

    campos.into_iter().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(kind: &str) -> monitors::Model {
        let agora = chrono::Utc::now();
        monitors::Model {
            id: 1,
            device_id: Some(1),
            probe_id: None,
            r#type: kind.into(),
            name: kind.into(),
            configuration: serde_json::json!({}),
            interval_seconds: 60,
            timeout_seconds: 10,
            retry_count: 0,
            enabled: true,
            next_run_at: None,
            last_run_at: None,
            status: "unknown".into(),
            created_at: agora.into(),
            updated_at: agora.into(),
        }
    }

    #[test]
    fn quem_so_faz_ping_nao_publica_campo_de_saude() {
        let campos = published_fields(&[monitor("ping")], &HashSet::new());
        assert!(campos.iter().any(|c| c == fields::LATENCY_MS));
        assert!(
            !campos.iter().any(|c| c == fields::CPU_USAGE_PERCENT),
            "oferecer um template de CPU aqui produziria uma regra que nunca dispara"
        );
    }

    #[test]
    fn o_monitor_gerenciado_nao_publica_status_de_alcance() {
        // Oferecer "Dispositivo sem resposta" para a máquina que faria o
        // alerta é a definição de regra inútil: `system_health` devolve `up`
        // ou `unknown`, nunca `down`.
        let campos = published_fields(&[monitor(managed::SYSTEM_HEALTH)], &HashSet::new());
        assert!(
            !campos.iter().any(|c| c == fields::STATUS),
            "um template de alcance sobre a coleta de saúde nunca dispararia"
        );
    }

    #[test]
    fn o_monitor_gerenciado_publica_o_vocabulario_de_saude() {
        let campos = published_fields(&[monitor(managed::SYSTEM_HEALTH)], &HashSet::new());
        for campo in [
            fields::CPU_USAGE_PERCENT,
            fields::MEMORY_USED_PERCENT,
            fields::STORAGE_USED_PERCENT,
            fields::LOAD_AVERAGE_1M,
        ] {
            assert!(campos.iter().any(|c| c == campo), "faltou {campo}");
        }
    }

    #[test]
    fn o_roteador_snmp_publica_cpu_quando_ja_gravou_a_serie() {
        // É o mesmo campo do servidor. É esse o ponto da Fase 3: o alerta de
        // CPU nasce para o parque, não para o servidor.
        let gravadas: HashSet<String> = [series::CPU_USAGE.to_string()].into_iter().collect();
        let campos = published_fields(&[monitor("snmp")], &gravadas);
        assert!(campos.iter().any(|c| c == fields::CPU_USAGE_PERCENT));
    }

    #[test]
    fn campos_nao_se_repetem_com_varios_monitores_do_mesmo_tipo() {
        let campos = published_fields(&[monitor("ping"), monitor("ping")], &HashSet::new());
        let unicos: HashSet<&String> = campos.iter().collect();
        assert_eq!(unicos.len(), campos.len());
    }
}
