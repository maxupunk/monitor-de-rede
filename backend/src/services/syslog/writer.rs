//! Escritor em lote: drena a fila e grava no banco de logs.
//!
//! Dois gatilhos, o que vier primeiro: **500 linhas** ou **200 ms**. O teto de
//! linhas vem da conta de parâmetros — 500 × 12 colunas = 6 000, dentro dos
//! 32 766 do SQLite ≥ 3.32 (o que o `sqlx` empacota) e dos 65 535 do
//! PostgreSQL. Subir o lote sem refazer essa conta quebra a gravação inteira,
//! não uma linha.
//!
//! O tempo é o que domina na carga real: a 12 msg/s o lote de 500 nunca enche,
//! e o log aparece na tela em no máximo 200 ms. A 200 msg/s de pico são ~40
//! linhas por descarga — medido no SPIKE-06 em 5,9 ms para 500 linhas, ou seja
//! 0,23 % do tempo do processo.

use std::{sync::Arc, time::Duration};

use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait};
use tokio::sync::mpsc;

use super::{
    bus::LogBus,
    config::{DEFAULT_BATCH_INTERVAL, DEFAULT_BATCH_SIZE},
    queue::{IngestMetrics, PendingLog},
};
use crate::{models::logs::device_logs, views::logs::serialize_entry_sem_nome};

/// Consome a fila até ela fechar. Pensado para viver num `tokio::spawn`.
pub async fn run(
    db: DatabaseConnection,
    mut receiver: mpsc::Receiver<PendingLog>,
    metrics: Arc<IngestMetrics>,
    bus: LogBus,
) {
    run_with(
        db,
        &mut receiver,
        metrics,
        DEFAULT_BATCH_SIZE,
        DEFAULT_BATCH_INTERVAL,
        Some(bus),
    )
    .await;
}

/// O laço, com os gatilhos injetados — é assim que o teste roda sem esperar
/// 200 ms de relógio de parede.
#[allow(clippy::too_many_arguments)]
pub async fn run_with(
    db: DatabaseConnection,
    receiver: &mut mpsc::Receiver<PendingLog>,
    metrics: Arc<IngestMetrics>,
    batch_size: usize,
    interval: Duration,
    bus: Option<LogBus>,
) {
    let mut lote: Vec<PendingLog> = Vec::with_capacity(batch_size);
    loop {
        let recebido = if lote.is_empty() {
            // Lote vazio: não há prazo correndo, então espera sem acordar à
            // toa. Um servidor sem tráfego de log não deve gastar um tique a
            // cada 200 ms para não fazer nada.
            receiver.recv().await
        } else {
            match tokio::time::timeout(interval, receiver.recv()).await {
                Ok(recebido) => recebido,
                Err(_) => {
                    descarrega(&db, &mut lote, &metrics, bus.as_ref()).await;
                    continue;
                }
            }
        };

        match recebido {
            Some(log) => {
                lote.push(log);
                if lote.len() >= batch_size {
                    descarrega(&db, &mut lote, &metrics, bus.as_ref()).await;
                }
            }
            // Fila fechada: grava o que sobrou antes de sair. Sem isto, um
            // desligamento gracioso perderia o último lote.
            None => {
                descarrega(&db, &mut lote, &metrics, bus.as_ref()).await;
                return;
            }
        }
    }
}

/// Grava o lote e o esvazia.
///
/// Falha de banco **não** derruba o laço: o disco pode estar cheio ou o arquivo
/// travado, e parar a ingestão por causa disso deixaria o listener enchendo a
/// fila até o descarte. Melhor perder o lote com o erro no log e continuar
/// lendo — a próxima descarga tem chance de passar.
async fn descarrega(
    db: &DatabaseConnection,
    lote: &mut Vec<PendingLog>,
    metrics: &IngestMetrics,
    bus: Option<&LogBus>,
) {
    if lote.is_empty() {
        return;
    }
    let total = lote.len();
    let linhas: Vec<device_logs::ActiveModel> = lote.drain(..).map(para_active_model).collect();

    // O live tail publica **depois** da gravação, e a partir do que o banco
    // devolveu: assim a linha na tela tem o mesmo `id` que a paginação vai
    // trazer, e a lista consegue deduplicar na fronteira entre as duas fontes.
    //
    // `exec_with_returning` custa um `RETURNING` a mais, então só é usado
    // quando existe alguém olhando. Sem assinante — o caso comum — o caminho é
    // o `exec` seco de sempre.
    let assinantes = bus.is_some_and(LogBus::has_subscribers);
    let resultado = if assinantes {
        device_logs::Entity::insert_many(linhas)
            .exec_with_returning(db)
            .await
            .map(Some)
    } else {
        device_logs::Entity::insert_many(linhas)
            .exec(db)
            .await
            .map(|_| None)
    };

    match resultado {
        Ok(gravadas) => {
            tracing::trace!(total, "lote de logs gravado");
            if let (Some(bus), Some(gravadas)) = (bus, gravadas) {
                bus.publish_batch(gravadas.into_iter().map(serialize_entry_sem_nome));
            }
        }
        Err(error) => {
            // Conta como descarte por lotação: do ponto de vista de quem lê o
            // contador, a linha entrou e não saiu. Esconder isso numa métrica
            // separada só faria a soma não fechar.
            for _ in 0..total {
                metrics.record_queue_full();
            }
            tracing::warn!(%error, total, "falha ao gravar lote de logs");
        }
    }
}

fn para_active_model(log: PendingLog) -> device_logs::ActiveModel {
    device_logs::ActiveModel {
        device_id: Set(log.device_id),
        source_ip: Set(log.source_ip),
        received_at: Set(log.received_at.into()),
        device_time: Set(log.parsed.device_time),
        facility: Set(log.parsed.facility),
        severity: Set(log.parsed.severity),
        hostname: Set(log.parsed.hostname),
        app_name: Set(log.parsed.app_name),
        pid: Set(log.parsed.pid),
        topics: Set(log.parsed.topics),
        message: Set(log.parsed.message),
        source: Set(log.source.as_str().to_string()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::syslog::{db, parser::ParsedLog, queue::LogQueue, queue::LogSource};
    use chrono::Utc;
    use sea_orm::PaginatorTrait;
    use serial_test::serial;

    fn log(mensagem: &str) -> PendingLog {
        PendingLog {
            device_id: Some(1),
            source_ip: "192.168.88.1".into(),
            received_at: Utc::now(),
            parsed: ParsedLog {
                severity: Some(6),
                topics: Some("system,info".into()),
                message: mensagem.into(),
                ..ParsedLog::default()
            },
            source: LogSource::Syslog,
        }
    }

    async fn banco() -> DatabaseConnection {
        std::env::remove_var("SYSLOG_DB_URL");
        db::connect("sqlite::memory:")
            .await
            .expect("banco de logs")
            .connection()
            .clone()
    }

    #[tokio::test]
    #[serial]
    async fn o_lote_cheio_descarrega_antes_do_prazo() {
        let db = banco().await;
        let metrics = Arc::new(IngestMetrics::default());
        let (queue, mut receiver) = LogQueue::create(16, Arc::clone(&metrics));
        for indice in 0..3 {
            assert!(queue.try_enqueue(log(&format!("linha {indice}"))));
        }
        drop(queue);

        // Gatilho de tamanho em 3, prazo longo: se o laço dependesse do tempo,
        // o teste levaria um minuto.
        run_with(
            db.clone(),
            &mut receiver,
            Arc::clone(&metrics),
            3,
            Duration::from_secs(60),
            None,
        )
        .await;

        let total = device_logs::Entity::find()
            .count(&db)
            .await
            .expect("contagem");
        assert_eq!(total, 3);
    }

    #[tokio::test]
    #[serial]
    async fn o_prazo_descarrega_o_lote_incompleto() {
        let db = banco().await;
        let metrics = Arc::new(IngestMetrics::default());
        let (queue, mut receiver) = LogQueue::create(16, Arc::clone(&metrics));
        queue.try_enqueue(log("sozinha"));
        drop(queue);

        // Lote de 500 nunca enche com uma linha só: quem grava é o prazo.
        run_with(
            db.clone(),
            &mut receiver,
            Arc::clone(&metrics),
            500,
            Duration::from_millis(20),
            None,
        )
        .await;

        let total = device_logs::Entity::find()
            .count(&db)
            .await
            .expect("contagem");
        assert_eq!(total, 1);
    }

    #[tokio::test]
    #[serial]
    async fn o_fechamento_da_fila_grava_o_que_sobrou() {
        let db = banco().await;
        let metrics = Arc::new(IngestMetrics::default());
        let (queue, mut receiver) = LogQueue::create(16, Arc::clone(&metrics));
        queue.try_enqueue(log("última"));
        drop(queue);

        // Prazo longo e lote grande: só o fechamento da fila explica a
        // gravação. É o caminho do desligamento gracioso.
        run_with(
            db.clone(),
            &mut receiver,
            Arc::clone(&metrics),
            500,
            Duration::from_secs(60),
            None,
        )
        .await;

        let total = device_logs::Entity::find()
            .count(&db)
            .await
            .expect("contagem");
        assert_eq!(total, 1, "o último lote não pode se perder no desligamento");
    }

    #[tokio::test]
    #[serial]
    async fn as_colunas_chegam_ao_banco_no_lugar_certo() {
        let db = banco().await;
        let metrics = Arc::new(IngestMetrics::default());
        let (queue, mut receiver) = LogQueue::create(4, Arc::clone(&metrics));
        queue.try_enqueue(log("mensagem gravada"));
        drop(queue);
        run_with(
            db.clone(),
            &mut receiver,
            metrics,
            1,
            Duration::from_secs(60),
            None,
        )
        .await;

        let linha = device_logs::Entity::find()
            .one(&db)
            .await
            .expect("consulta")
            .expect("linha");
        assert_eq!(linha.message, "mensagem gravada");
        assert_eq!(linha.severity, Some(6));
        assert_eq!(linha.topics.as_deref(), Some("system,info"));
        assert_eq!(linha.device_id, Some(1));
        assert_eq!(linha.source_ip, "192.168.88.1");
    }

    #[tokio::test]
    #[serial]
    async fn o_live_tail_recebe_a_linha_com_o_id_do_banco() {
        // O `id` é o que permite à tela deduplicar entre o tail e a paginação:
        // as duas fontes se sobrepõem na fronteira, e sem chave estável a
        // mesma linha apareceria duas vezes.
        let db = banco().await;
        let bus = LogBus::create();
        let mut assinante = bus.subscribe();
        let metrics = Arc::new(IngestMetrics::default());
        let (queue, mut receiver) = LogQueue::create(4, Arc::clone(&metrics));
        queue.try_enqueue(log("ao vivo"));
        drop(queue);

        run_with(
            db,
            &mut receiver,
            metrics,
            1,
            Duration::from_secs(60),
            Some(bus),
        )
        .await;

        let entrada = assinante.try_recv().expect("o tail não recebeu nada");
        assert_eq!(entrada.message, "ao vivo");
        assert!(entrada.id > 0, "id veio do banco, não zerado");
        assert_eq!(entrada.severity_label.as_deref(), Some("informação"));
        assert_eq!(entrada.topics, vec!["system", "info"]);
    }

    #[tokio::test]
    #[serial]
    async fn sem_assinante_o_tail_nao_custa_nada_e_a_gravacao_segue() {
        // Caso comum: ninguém com a tela aberta. O escritor não pode pagar o
        // `RETURNING` nem serializar linha para jogar fora.
        let db = banco().await;
        let bus = LogBus::create();
        let metrics = Arc::new(IngestMetrics::default());
        let (queue, mut receiver) = LogQueue::create(4, Arc::clone(&metrics));
        queue.try_enqueue(log("sem plateia"));
        drop(queue);

        run_with(
            db.clone(),
            &mut receiver,
            metrics,
            1,
            Duration::from_secs(60),
            Some(bus),
        )
        .await;

        assert_eq!(
            device_logs::Entity::find()
                .count(&db)
                .await
                .expect("contagem"),
            1
        );
    }
}
