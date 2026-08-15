//! Histerese de **disparo** (Fase 5 do roadmap, F3/F7 da análise).
//!
//! `duration_seconds` mede continuidade: a regra só dispara quando a condição
//! se sustenta pelo tempo configurado. Até a Fase 4 essa contagem vivia num
//! `HashMap` estático de `Instant` e morria com o processo — reiniciar o
//! scheduler zerava a tolerância de todo mundo, e o caso "disparou depois da
//! tolerância" era intestável.
//!
//! Duas coisas mudam aqui, e a segunda é a que importa:
//!
//! 1. O relógio passa a ser [`DateTime<Utc>`] injetado, não `Instant`. A
//!    contagem vira testável sem `sleep`.
//! 2. Na primeira observação de um par (regra, alvo) — ou seja, depois de um
//!    restart — a contagem é **reconstruída a partir de `monitor_results`**, e
//!    não recomeçada do zero. O fato bruto já está no banco; o que faltava era
//!    lê-lo.
//!
//! **A reconstrução só afirma o que a observação gravada prova.** Uma linha de
//! `monitor_results` guarda status, duração, latência e o `data` do checker —
//! não as métricas soltas. Uma regra de `packetLoss` não encontra o campo no
//! histórico, a avaliação da linha mais recente já dá `false` e a reconstrução
//! simplesmente não acontece: a contagem começa agora, exatamente como antes.
//! Nenhum caminho aqui inventa continuidade que ninguém observou — é a objeção
//! que manteve o `pending_since` em memória até esta fase, e ela continua
//! valendo. O que a persistência não podia fazer, a leitura do histórico pode.
//!
//! A contagem em memória continua sendo o caminho quente: sem ela, cada
//! checagem de cada monitor saudável viraria escrita no banco. O banco entra
//! só quando a memória não sabe.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use crate::{
    models::{_entities::monitor_results as monitor_results_entity, monitor_results, monitors},
    services::{
        alerts::{
            contracts::AlertDataset,
            evaluator::{self, AlertRuleCondition},
            fields,
        },
        shared::errors::AppResult,
    },
};

/// Quantas observações a reconstrução examina, no máximo.
///
/// Cinquenta cobre com folga qualquer tolerância razoável (uma regra de 30 min
/// sobre um monitor de 60 s usa 30 linhas) e mantém a consulta barata — ela só
/// acontece na primeira observação depois de um restart.
const REBUILD_LIMIT: u64 = 50;

/// Quantos intervalos de checagem podem separar duas observações antes de a
/// continuidade ser considerada rompida.
///
/// Duas linhas que batem a condição, com meia hora de silêncio entre elas, não
/// provam meia hora de problema contínuo — provam duas medições. Três
/// intervalos toleram o atraso normal do agendador sem tolerar um apagão.
const MAX_GAP_INTERVALS: i32 = 3;

/// Depois de quanto tempo sem observação uma entrada em memória é varrida.
///
/// Fecha o vazamento lento (F7): monitor ou regra apagados param de ser
/// avaliados, e a entrada deles ficaria no mapa até o processo morrer.
pub const IDLE_TTL_HOURS: i64 = 24;

/// A condição batendo continuamente desde `since`, vista pela última vez em
/// `last_seen_at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingCondition {
    since: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

fn pending() -> &'static Mutex<HashMap<(i64, String), PendingCondition>> {
    static PENDING: OnceLock<Mutex<HashMap<(i64, String), PendingCondition>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Registra que a condição bateu e devolve `true` quando a tolerância da regra
/// já se cumpriu.
///
/// `monitor_id` é o que permite a reconstrução; alvos sem monitor (interface,
/// túnel) não têm histórico equivalente e caem no comportamento anterior —
/// contagem a partir da primeira observação deste processo.
///
/// # Errors
///
/// Propaga erro do banco na reconstrução.
pub async fn observe<C: ConnectionTrait>(
    db: &C,
    rule_id: i64,
    tolerance_seconds: i64,
    scope_key: &str,
    monitor_id: Option<i64>,
    condition: &AlertRuleCondition,
    now: DateTime<Utc>,
) -> AppResult<bool> {
    // Sem tolerância não há o que lembrar: a condição batendo já basta.
    if tolerance_seconds <= 0 {
        return Ok(true);
    }
    let tolerance = Duration::seconds(tolerance_seconds);
    let key = (rule_id, scope_key.to_string());

    match remembered(&key, now, tolerance) {
        Some(since) => Ok(now - since >= tolerance),
        None => {
            // Memória vazia: ou é a primeira ocorrência, ou o processo
            // reiniciou. O histórico sabe a diferença.
            let since = match monitor_id {
                Some(monitor_id) => rebuilt_since(db, monitor_id, condition, now).await?,
                None => None,
            }
            .unwrap_or(now);
            remember(key, since, now);
            Ok(now - since >= tolerance)
        }
    }
}

/// A condição parou de bater (ou a regra disparou e o episódio se encerrou): a
/// contagem recomeça do zero.
pub fn forget(rule_id: i64, scope_key: &str) {
    if let Ok(mut map) = pending().lock() {
        map.remove(&(rule_id, scope_key.to_string()));
    }
}

/// Descarta as contagens que ninguém mais alimenta e devolve quantas saíram.
///
/// Chamada junto da purga de dados antigos. Uma entrada só envelhece quando o
/// par (regra, alvo) deixa de ser avaliado — regra apagada, monitor removido,
/// dispositivo que saiu do inventário.
pub fn sweep(now: DateTime<Utc>, max_idle: Duration) -> usize {
    let Ok(mut map) = pending().lock() else {
        return 0;
    };
    let before = map.len();
    map.retain(|_, entry| now - entry.last_seen_at <= max_idle);
    before - map.len()
}

/// A contagem em memória, se ainda valer.
///
/// Uma tolerância inteira sem observação rompe a continuidade: o que quer que
/// tenha acontecido nesse intervalo, ninguém viu. A entrada é descartada e a
/// contagem recomeça — é a mesma exigência de "continuidade observada" que a
/// reconstrução aplica ao histórico.
fn remembered(
    key: &(i64, String),
    now: DateTime<Utc>,
    tolerance: Duration,
) -> Option<DateTime<Utc>> {
    let mut map = pending().lock().ok()?;
    let entry = *map.get(key)?;
    if now - entry.last_seen_at > tolerance {
        map.remove(key);
        return None;
    }
    map.insert(
        key.clone(),
        PendingCondition {
            since: entry.since,
            last_seen_at: now,
        },
    );
    Some(entry.since)
}

fn remember(key: (i64, String), since: DateTime<Utc>, now: DateTime<Utc>) {
    if let Ok(mut map) = pending().lock() {
        map.insert(
            key,
            PendingCondition {
                since,
                last_seen_at: now,
            },
        );
    }
}

/// Desde quando a condição bate, segundo o histórico gravado.
///
/// Caminha do resultado mais recente para trás enquanto a condição se sustenta
/// e a distância entre duas observações consecutivas couber em
/// [`MAX_GAP_INTERVALS`] intervalos. Devolve `None` quando a linha mais recente
/// já não satisfaz a condição — o que inclui, de propósito, toda regra cujo
/// campo o histórico não guarda.
async fn rebuilt_since<C: ConnectionTrait>(
    db: &C,
    monitor_id: i64,
    condition: &AlertRuleCondition,
    now: DateTime<Utc>,
) -> AppResult<Option<DateTime<Utc>>> {
    let Some(monitor) = monitors::Entity::find_by_id(monitor_id).one(db).await? else {
        return Ok(None);
    };
    // Ordenado por `started_at`, e não por id: a caminhada é cronológica, e é
    // exatamente o índice `monitor_results_monitor_started_index`.
    let history = monitor_results::Entity::find()
        .filter(monitor_results_entity::Column::MonitorId.eq(monitor_id))
        .order_by_desc(monitor_results_entity::Column::StartedAt)
        .limit(REBUILD_LIMIT)
        .all(db)
        .await?;

    let max_gap = Duration::seconds(i64::from(
        monitor.interval_seconds.max(1) * MAX_GAP_INTERVALS,
    ));
    let mut since: Option<DateTime<Utc>> = None;
    let mut newer_at = now;

    for row in history {
        let observed_at = row.started_at.with_timezone(&Utc);
        if newer_at - observed_at > max_gap {
            break;
        }
        if !evaluator::evaluate(condition, &historic_dataset(&monitor.r#type, &row)) {
            break;
        }
        since = Some(observed_at);
        newer_at = observed_at;
    }
    Ok(since)
}

/// Os fatos que uma observação **gravada** consegue provar.
///
/// É deliberadamente menor que o dataset do ciclo vivo
/// ([`super::datasets::monitor_result::build`]): `monitor_results` guarda
/// status, duração, latência e o `data` do checker, mas não as métricas soltas.
/// Publicar `latencyMs` como `null` explícito quando a coluna está vazia segue
/// a mesma convenção do dataset vivo — o avaliador distingue "não medido" de
/// "medido zero".
fn historic_dataset(monitor_type: &str, row: &monitor_results::Model) -> AlertDataset {
    let mut dataset = AlertDataset::new();
    dataset.insert(fields::STATUS.into(), json!(row.status));
    dataset.insert("success".into(), json!(row.status == "up"));
    dataset.insert(fields::DURATION_MS.into(), json!(row.duration_ms));
    dataset.insert("type".into(), json!(monitor_type));
    dataset.insert(
        fields::LATENCY_MS.into(),
        row.latency_ms.map_or(Value::Null, |value| json!(value)),
    );
    if let Some(Value::Object(extras)) = &row.data {
        for (key, value) in extras {
            if !dataset.contains_key(key) {
                dataset.insert(key.clone(), value.clone());
            }
        }
    }
    dataset
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn condicao(field: &str, operator: &str, value: Value) -> AlertRuleCondition {
        AlertRuleCondition::from_json(&json!({
            "field": field, "operator": operator, "value": value
        }))
        .expect("condição válida")
    }

    fn resultado(id: i64, status: &str, segundos_atras: i64) -> monitor_results::Model {
        let at = (t0() - Duration::seconds(segundos_atras)).into();
        monitor_results::Model {
            id,
            monitor_id: 1,
            probe_id: None,
            status: status.into(),
            started_at: at,
            finished_at: at,
            duration_ms: 12,
            latency_ms: Some(30.0),
            message: None,
            data: None,
            created_at: at,
        }
    }

    #[test]
    fn o_historico_prova_status_duracao_e_latencia() {
        let dataset = historic_dataset("ping", &resultado(1, "down", 0));
        assert_eq!(dataset[fields::STATUS], json!("down"));
        assert_eq!(dataset[fields::LATENCY_MS], json!(30.0));
        assert_eq!(dataset[fields::DURATION_MS], json!(12));
        assert_eq!(dataset["success"], json!(false));
        assert_eq!(dataset["type"], json!("ping"));
    }

    #[test]
    fn latencia_ausente_vira_null_explicito_e_nao_chave_faltando() {
        let mut row = resultado(1, "up", 0);
        row.latency_ms = None;
        let dataset = historic_dataset("tcp", &row);
        assert_eq!(dataset.get(fields::LATENCY_MS), Some(&Value::Null));
    }

    #[test]
    fn o_data_do_checker_entra_sem_sobrescrever_o_que_a_coluna_ja_disse() {
        let mut row = resultado(1, "down", 0);
        row.data = Some(json!({ "status": "up", "statusCode": 503 }));
        let dataset = historic_dataset("http", &row);
        // A coluna é a verdade sobre o status; o `data` só acrescenta.
        assert_eq!(dataset[fields::STATUS], json!("down"));
        assert_eq!(dataset["statusCode"], json!(503));
    }

    #[test]
    fn campo_que_o_historico_nao_guarda_nao_reconstroi_nada() {
        // `packetLoss` só existe como métrica solta do ciclo vivo. A avaliação
        // da linha mais recente falha e a contagem recomeça do zero — sem
        // inventar continuidade.
        let dataset = historic_dataset("ping", &resultado(1, "down", 0));
        assert!(!evaluator::evaluate(
            &condicao(fields::PACKET_LOSS, "gt", json!(10)),
            &dataset
        ));
    }

    #[test]
    fn a_condicao_e_avaliada_igual_sobre_o_historico_e_sobre_o_ciclo_vivo() {
        let dataset = historic_dataset("ping", &resultado(1, "down", 0));
        assert!(evaluator::evaluate(
            &condicao(fields::STATUS, "eq", json!("down")),
            &dataset
        ));
        assert!(!evaluator::evaluate(
            &condicao(fields::STATUS, "eq", json!("up")),
            &dataset
        ));
    }

    // --- Contagem em memória --------------------------------------------------

    #[test]
    fn a_varredura_descarta_so_o_que_ninguem_mais_alimenta() {
        let chave_viva = (900_101, "monitor:900101".to_string());
        let chave_morta = (900_102, "monitor:900102".to_string());
        remember(chave_viva.clone(), t0(), t0());
        remember(chave_morta.clone(), t0(), t0() - Duration::hours(48));

        let removidas = sweep(t0(), Duration::hours(IDLE_TTL_HOURS));
        assert!(removidas >= 1, "a entrada ociosa deveria ter saído");
        assert!(
            remembered(&chave_viva, t0(), Duration::seconds(300)).is_some(),
            "a entrada viva não pode ser varrida"
        );
        assert!(remembered(&chave_morta, t0(), Duration::seconds(300)).is_none());
        forget(chave_viva.0, &chave_viva.1);
    }

    #[test]
    fn uma_tolerancia_inteira_sem_observacao_reinicia_a_contagem() {
        let chave = (900_103, "monitor:900103".to_string());
        let tolerancia = Duration::seconds(300);
        remember(chave.clone(), t0(), t0());

        // Dentro da tolerância: a contagem original sobrevive.
        assert_eq!(
            remembered(&chave, t0() + Duration::seconds(300), tolerancia),
            Some(t0())
        );
        // Além dela: ninguém viu o que aconteceu no intervalo.
        remember(chave.clone(), t0(), t0());
        assert_eq!(
            remembered(&chave, t0() + Duration::seconds(301), tolerancia),
            None
        );
        forget(chave.0, &chave.1);
    }

    #[test]
    fn a_primeira_ocorrencia_apenas_inicia_a_contagem() {
        // Matriz de paridade #24: `durationSeconds` só dispara depois de a
        // condição se sustentar — a primeira passagem nunca alerta.
        let chave = (900_105, "monitor:900105".to_string());
        let tolerancia = Duration::seconds(300);
        assert!(remembered(&chave, t0(), tolerancia).is_none());

        remember(chave.clone(), t0(), t0());
        let since = remembered(&chave, t0(), tolerancia).expect("contagem iniciada");
        assert!(t0() - since < tolerancia, "ainda não cumpriu a tolerância");

        // Só com a tolerância inteira cumprida o disparo é liberado.
        let depois = t0() + tolerancia;
        assert!(depois - remembered(&chave, depois, tolerancia).unwrap() >= tolerancia);
        forget(chave.0, &chave.1);
    }

    #[test]
    fn a_condicao_que_deixa_de_bater_reinicia_a_contagem() {
        let chave = (900_106, "monitor:900106".to_string());
        remember(chave.clone(), t0(), t0());
        forget(chave.0, &chave.1);
        assert!(
            remembered(&chave, t0(), Duration::seconds(300)).is_none(),
            "esquecer é recomeçar do zero"
        );
    }

    #[test]
    fn a_contagem_e_por_regra_e_por_alvo() {
        let regra = 900_107;
        remember((regra, "monitor:10".to_string()), t0(), t0());
        assert!(remembered(
            &(regra, "monitor:10".to_string()),
            t0(),
            Duration::seconds(300)
        )
        .is_some());
        // Alvo diferente da mesma regra tem contagem própria.
        assert!(remembered(
            &(regra, "monitor:11".to_string()),
            t0(),
            Duration::seconds(300)
        )
        .is_none());
        // E regra diferente do mesmo alvo, também.
        assert!(remembered(
            &(regra + 1, "monitor:10".to_string()),
            t0(),
            Duration::seconds(300)
        )
        .is_none());
        forget(regra, "monitor:10");
    }

    #[test]
    fn observar_renova_o_ultimo_avistamento_sem_mexer_no_inicio() {
        let chave = (900_104, "monitor:900104".to_string());
        remember(chave.clone(), t0(), t0());
        let depois = t0() + Duration::seconds(120);
        assert_eq!(
            remembered(&chave, depois, Duration::seconds(300)),
            Some(t0())
        );
        // O `last_seen_at` avançou: 300 s depois de `depois` ainda vale.
        assert_eq!(
            remembered(
                &chave,
                depois + Duration::seconds(300),
                Duration::seconds(300)
            ),
            Some(t0())
        );
        forget(chave.0, &chave.1);
    }
}
