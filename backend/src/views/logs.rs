//! Serialização da tela de logs, com a hidratação do nome do dispositivo.
//!
//! **A hidratação é o preço do banco separado.** Não há FK nem `JOIN` entre
//! `device_logs.device_id` e `devices.id`: eles moram em arquivos diferentes.
//! Então a página vem do banco de logs e os nomes vêm da base principal num
//! `IN (…)` sobre o conjunto de ids da página — no máximo 200 ids, quase sempre
//! bem menos, porque uma tela de logs costuma mostrar poucos dispositivos
//! repetidos muitas vezes.

use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};

use crate::{
    dtos::logs::{
        LogEntry, LogIngestMetrics, LogNatDiagnostics, LogPageMeta, LogPageResponse,
        LogSourceCandidate, LogSourceEntry, LogSourcesResponse,
    },
    models::{_entities::devices, logs::device_logs},
    services::{
        shared::errors::AppResult,
        syslog::{
            nat::NatDetector,
            queue::IngestSnapshot,
            repository::{LogPage, LogQuery},
            sources::{SeenSource, SourceKind},
        },
    },
};

/// Rótulo em português da severidade do RFC 5424.
///
/// Fica no backend, e não na tela, porque a Fase 6 vai usar os mesmos rótulos
/// no texto da notificação — duas tabelas iguais divergiriam na primeira
/// alteração.
#[must_use]
pub fn severity_label(severity: i16) -> Option<&'static str> {
    match severity {
        0 => Some("emergência"),
        1 => Some("alerta"),
        2 => Some("crítico"),
        3 => Some("erro"),
        4 => Some("aviso"),
        5 => Some("notícia"),
        6 => Some("informação"),
        7 => Some("depuração"),
        _ => None,
    }
}

/// Monta a resposta da página, hidratando os nomes numa segunda consulta.
///
/// # Errors
///
/// Propaga erro do banco principal.
pub async fn serialize_page(
    inventory: &DatabaseConnection,
    page: LogPage,
    query: &LogQuery,
) -> AppResult<LogPageResponse> {
    let nomes = nomes_de_dispositivo(inventory, &page.rows).await?;
    let meta = LogPageMeta {
        next_cursor: page.next_cursor.map(|cursor| cursor.encode()),
        has_more: page.next_cursor.is_some(),
        limit: query.limit,
        from: query.from.to_rfc3339(),
        to: query.to.to_rfc3339(),
    };
    Ok(LogPageResponse {
        data: page
            .rows
            .into_iter()
            .map(|linha| serialize_entry(linha, &nomes))
            .collect(),
        meta,
    })
}

/// Um `IN (…)` sobre os ids distintos da página.
async fn nomes_de_dispositivo(
    inventory: &DatabaseConnection,
    rows: &[device_logs::Model],
) -> AppResult<HashMap<i64, String>> {
    let ids: HashSet<i64> = rows.iter().filter_map(|linha| linha.device_id).collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let encontrados: Vec<(i64, String)> = devices::Entity::find()
        .select_only()
        .column(devices::Column::Id)
        .column(devices::Column::Name)
        .filter(devices::Column::Id.is_in(ids))
        .into_tuple()
        .all(inventory)
        .await?;
    Ok(encontrados.into_iter().collect())
}

/// Serializa sem hidratar o nome do dispositivo.
///
/// É o caminho do live tail: o escritor está no meio da ingestão e consultar
/// `devices` a cada lote para preencher um rótulo seria pagar uma ida ao banco
/// principal por linha publicada. A tela já conhece os nomes — ela acabou de
/// paginar — e completa o que faltar pelo `deviceId`.
#[must_use]
pub fn serialize_entry_sem_nome(linha: device_logs::Model) -> LogEntry {
    serialize_entry(linha, &HashMap::new())
}

fn serialize_entry(linha: device_logs::Model, nomes: &HashMap<i64, String>) -> LogEntry {
    LogEntry {
        id: linha.id,
        device_id: linha.device_id,
        device_name: linha.device_id.and_then(|id| nomes.get(&id)).cloned(),
        source_ip: linha.source_ip,
        received_at: linha.received_at.to_rfc3339(),
        device_time: linha.device_time.map(|instante| instante.to_rfc3339()),
        facility: linha.facility,
        severity: linha.severity,
        severity_label: linha.severity.and_then(severity_label).map(str::to_owned),
        hostname: linha.hostname,
        app_name: linha.app_name,
        pid: linha.pid,
        topics: linha.topics.map_or_else(Vec::new, |texto| {
            texto
                .split(',')
                .filter(|parte| !parte.is_empty())
                .map(str::to_owned)
                .collect()
        }),
        message: linha.message,
    }
}

/// Serializa as fontes vistas, hidratando nome de dispositivo e candidatos.
///
/// # Errors
///
/// Propaga erro do banco principal.
pub async fn serialize_sources(
    inventory: &DatabaseConnection,
    fontes: Vec<SeenSource>,
    unknown_count: usize,
    metrics: IngestSnapshot,
    nat: &NatDetector,
) -> AppResult<LogSourcesResponse> {
    let masked_count = fontes.iter().filter(|fonte| fonte.masked).count();
    let ids: HashSet<i64> = fontes
        .iter()
        .flat_map(|fonte| fonte.device_id.into_iter().chain(fonte.candidates.clone()))
        .collect();
    let nomes = nomes_por_id(inventory, ids).await?;

    Ok(LogSourcesResponse {
        data: fontes
            .into_iter()
            .map(|fonte| LogSourceEntry {
                kind: match fonte.kind {
                    SourceKind::Device => "device",
                    SourceKind::Network => "network",
                    SourceKind::Ambiguous => "ambiguous",
                    SourceKind::Unknown => "unknown",
                }
                .to_owned(),
                device_name: fonte.device_id.and_then(|id| nomes.get(&id)).cloned(),
                candidates: fonte
                    .candidates
                    .iter()
                    .map(|id| LogSourceCandidate {
                        id: *id,
                        name: nomes
                            .get(id)
                            .cloned()
                            .unwrap_or_else(|| format!("Dispositivo {id}")),
                    })
                    .collect(),
                source_ip: fonte.source_ip,
                bind_key: fonte.bind_key,
                masked: fonte.masked,
                device_id: fonte.device_id,
                hostname: fonte.hostname,
                message_count: fonte.message_count,
                dropped_count: fonte.dropped_count,
                first_seen_at: fonte.first_seen_at.to_rfc3339(),
                last_seen_at: fonte.last_seen_at.to_rfc3339(),
                last_message: fonte.last_message,
            })
            .collect(),
        unknown_count,
        metrics: LogIngestMetrics {
            received: metrics.received,
            stored: metrics.queued,
            dropped_queue_full: metrics.dropped_queue_full,
            dropped_rate_limit: metrics.dropped_rate_limit,
            dropped_unknown_source: metrics.dropped_unknown_source,
            dropped_oversized: metrics.dropped_oversized,
        },
        nat: LogNatDiagnostics {
            masked_count,
            containerized: nat.containerized,
            gateways: nat.gateways(),
        },
    })
}

async fn nomes_por_id(
    inventory: &DatabaseConnection,
    ids: HashSet<i64>,
) -> AppResult<HashMap<i64, String>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let encontrados: Vec<(i64, String)> = devices::Entity::find()
        .select_only()
        .column(devices::Column::Id)
        .column(devices::Column::Name)
        .filter(devices::Column::Id.is_in(ids))
        .into_tuple()
        .all(inventory)
        .await?;
    Ok(encontrados.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn linha(
        device_id: Option<i64>,
        topics: Option<&str>,
        severity: Option<i16>,
    ) -> device_logs::Model {
        device_logs::Model {
            id: 1,
            device_id,
            source_ip: "192.168.88.1".into(),
            received_at: Utc::now().into(),
            device_time: None,
            facility: Some(16),
            severity,
            hostname: Some("MikroTik".into()),
            app_name: None,
            pid: None,
            topics: topics.map(str::to_owned),
            message: "mensagem".into(),
            created_at: Utc::now().into(),
        }
    }

    #[test]
    fn os_topicos_chegam_como_lista() {
        let entrada = serialize_entry(
            linha(None, Some("system,info,account"), None),
            &HashMap::new(),
        );
        assert_eq!(entrada.topics, vec!["system", "info", "account"]);
    }

    #[test]
    fn sem_topicos_a_lista_e_vazia_e_nao_nula() {
        // `null` obrigaria toda leitura da tela a checar antes de iterar.
        let entrada = serialize_entry(linha(None, None, None), &HashMap::new());
        assert!(entrada.topics.is_empty());
        let vazio = serialize_entry(linha(None, Some(""), None), &HashMap::new());
        assert!(vazio.topics.is_empty());
    }

    #[test]
    fn o_nome_do_dispositivo_vem_do_mapa_hidratado() {
        let mut nomes = HashMap::new();
        nomes.insert(7_i64, "Roteador Matriz".to_owned());
        let entrada = serialize_entry(linha(Some(7), None, None), &nomes);
        assert_eq!(entrada.device_name.as_deref(), Some("Roteador Matriz"));
    }

    #[test]
    fn dispositivo_apagado_deixa_a_linha_sem_nome_em_vez_de_sumir() {
        // Sem FK entre os bancos, a linha sobrevive ao aparelho. Ela continua
        // sendo log legítimo até a retenção recolher.
        let entrada = serialize_entry(linha(Some(99), None, None), &HashMap::new());
        assert_eq!(entrada.device_id, Some(99));
        assert_eq!(entrada.device_name, None);
    }

    #[test]
    fn a_severidade_ganha_rotulo_em_portugues() {
        let entrada = serialize_entry(linha(None, None, Some(3)), &HashMap::new());
        assert_eq!(entrada.severity_label.as_deref(), Some("erro"));
        let sem = serialize_entry(linha(None, None, None), &HashMap::new());
        assert_eq!(sem.severity_label, None);
        // Valor fora da tabela do RFC não inventa rótulo.
        let invalida = serialize_entry(linha(None, None, Some(42)), &HashMap::new());
        assert_eq!(invalida.severity_label, None);
    }
}
