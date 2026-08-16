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
    dtos::logs::{LogEntry, LogPageMeta, LogPageResponse},
    models::{_entities::devices, logs::device_logs},
    services::{
        shared::errors::AppResult,
        syslog::repository::{LogPage, LogQuery},
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
