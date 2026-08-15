//! Diário e despacho de notificações (Fase 4 do roadmap, §2.3 da análise).
//!
//! Até a Fase 3, quem abria um alerta entregava a notificação na linha
//! seguinte. Dois problemas moravam aí: um crash entre o `INSERT` do evento e o
//! envio perdia a notificação sem deixar rastro, e não havia onde pendurar
//! cooldown ou agrupamento — a decisão e a entrega eram o mesmo gesto.
//!
//! Aqui elas se separam. O motor **enfileira** ([`enqueue`]), a política pura
//! ([`super::policy`]) decide o destino, e o ciclo do scheduler **despacha**
//! ([`dispatch_pending`]). O que a tabela ganha em troca do salto de alguns
//! segundos na entrega:
//!
//! - **Entrega ao menos uma vez**: a linha fica `pending` até os canais serem
//!   chamados. Processo que morre no meio reenvia no ciclo seguinte.
//! - **Cooldown**: a última entrega de cada par (regra, alvo) está gravada.
//! - **Agrupamento**: mensagens represadas do mesmo grupo saem consolidadas.
//! - **Inibição**: a linha inibível espera a carência e é julgada na entrega,
//!   quando o pai já teve tempo de ser detectado.
//! - **Auditoria**: "por que não fui avisado?" tem resposta — a linha
//!   suprimida guarda o motivo.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use loco_rs::app::AppContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};

use crate::{
    models::{devices, notification_outbox, sites},
    services::{
        alerts::inhibition,
        notifications::{
            contracts::{NotificationMessage, Severity},
            formatter,
            policy::{
                self, Decision, DeliveryInput, DeliveryLog, DigestPolicy, NotificationKind,
                NotificationPolicy, SuppressReason,
            },
            NotificationService,
        },
        shared::errors::AppResult,
    },
};

/// Valores de `notification_outbox.status`.
const PENDING: &str = "pending";
const SENT: &str = "sent";
const SUPPRESSED: &str = "suppressed";

/// Quantas linhas um despacho processa por passagem.
///
/// O ciclo do scheduler roda a cada poucos segundos; um teto evita que uma
/// tempestade acumulada trave o ciclo inteiro numa única passagem. O que sobra
/// sai na seguinte, em ordem de id.
const DISPATCH_BATCH: u64 = 200;

/// Grupo de correlação `global`: alvo sem site e sem dispositivo.
const GLOBAL_GROUP: &str = "global";

/// Uma notificação pedida pelo motor de alertas.
///
/// O site não entra: ele é derivado do dispositivo na hora do enfileiramento,
/// junto com a hierarquia. Assim quem pede a notificação — manager e recovery,
/// que chegam ao mesmo ponto por caminhos diferentes — não precisa carregar
/// contexto que o banco já sabe.
#[derive(Debug, Clone)]
pub struct NotificationRequest {
    pub alert_rule_id: Option<i64>,
    pub scope_key: Option<String>,
    pub device_id: Option<i64>,
    pub kind: NotificationKind,
    /// Parâmetros da regra; tudo zerado quando a regra sumiu.
    pub policy: NotificationPolicy,
    /// Há silêncio vigente pedido pelo operador para este alerta.
    pub silenced: bool,
    pub message: NotificationMessage,
}

/// O que uma passagem do despachante fez.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DispatchStats {
    /// Mensagens efetivamente entregues aos canais (uma consolidada conta 1).
    pub delivered: u64,
    /// Linhas que couberam numa mensagem consolidada.
    pub consolidated: u64,
    /// Linhas engolidas pela inibição na hora da entrega.
    pub suppressed: u64,
}

impl DispatchStats {
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.delivered + self.consolidated + self.suppressed
    }
}

/// Chave de correlação do agrupamento.
///
/// Site primeiro: "8 alertas no site Matriz" é a frase que o operador entende.
/// Sem site, o dispositivo agrupa os alertas do mesmo equipamento; sem nenhum
/// dos dois, tudo cai no grupo global — que é o certo para alertas de alvo
/// solto (checagem externa, por exemplo).
#[must_use]
pub fn group_key(site_id: Option<i64>, device_id: Option<i64>) -> String {
    match (site_id, device_id) {
        (Some(site), _) => format!("site:{site}"),
        (None, Some(device)) => format!("device:{device}"),
        (None, None) => GLOBAL_GROUP.to_string(),
    }
}

/// Enfileira a notificação, gravando a decisão da política.
///
/// Devolve a [`Decision`] tomada — é o que os testes inspecionam sem precisar
/// esperar o despachante.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn enqueue(ctx: &AppContext, request: NotificationRequest) -> AppResult<Decision> {
    let now = Utc::now();
    // Uma consulta responde às duas perguntas do dispositivo: em que site ele
    // agrupa e se tem pai declarado para a inibição julgar depois.
    let device = match request.device_id {
        Some(device_id) => devices::Entity::find_by_id(device_id).one(&ctx.db).await?,
        None => None,
    };
    let group = group_key(
        device.as_ref().and_then(|device| device.site_id),
        request.device_id,
    );
    let digest = DigestPolicy::from_env();

    let decision = policy::decide(&DeliveryInput {
        kind: request.kind,
        severity: request.message.severity,
        policy: request.policy,
        digest,
        log: delivery_log(
            &ctx.db,
            request.alert_rule_id,
            request.scope_key.as_deref(),
            now,
        )
        .await?,
        group_last_sent_at: group_last_sent_at(&ctx.db, &group).await?,
        silenced: request.silenced,
        now,
    });

    // Só faz sentido pagar a carência da inibição quando há de fato um pai
    // declarado: sem hierarquia, atrasar a mensagem seria custo sem benefício.
    let inhibitable = request.policy.inhibit_when_parent_down
        && device.is_some_and(|device| device.parent_id.is_some());

    let (status, suppress_reason, mut deliver_after) = match decision {
        Decision::Deliver => (PENDING, None, now),
        Decision::Digest { after } => (PENDING, None, after),
        Decision::Suppress(reason) => (SUPPRESSED, Some(reason), now),
    };
    if status == PENDING && inhibitable {
        deliver_after =
            deliver_after.max(now + Duration::seconds(policy::INHIBITION_GRACE_SECONDS));
    }

    notification_outbox::ActiveModel {
        alert_rule_id: Set(request.alert_rule_id),
        scope_key: Set(request.scope_key),
        device_id: Set(request.device_id),
        group_key: Set(group),
        kind: Set(request.kind.as_str().into()),
        title: Set(request.message.title),
        body: Set(request.message.body),
        severity: Set(request.message.severity.as_str().into()),
        metadata: Set(request.message.metadata),
        status: Set(status.into()),
        suppress_reason: Set(suppress_reason.map(|reason| reason.as_str().to_string())),
        inhibitable: Set(inhibitable),
        deliver_after: Set(deliver_after.into()),
        sent_at: Set(None),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    Ok(decision)
}

/// Entrega o que já venceu, consolidando o que couber numa mensagem só.
///
/// Chamado a cada ciclo do scheduler. Falha de canal **não** sobe (§8.9): a
/// linha é marcada como entregue de qualquer forma, porque o canal já registrou
/// o próprio erro e reenviar em laço só multiplicaria a falha.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn dispatch_pending(ctx: &AppContext) -> AppResult<DispatchStats> {
    let now = Utc::now();
    let due = notification_outbox::Entity::find()
        .filter(notification_outbox::Column::Status.eq(PENDING))
        .filter(notification_outbox::Column::DeliverAfter.lte(now))
        .order_by_asc(notification_outbox::Column::Id)
        .limit(DISPATCH_BATCH)
        .all(&ctx.db)
        .await?;
    if due.is_empty() {
        return Ok(DispatchStats::default());
    }

    let mut stats = DispatchStats::default();
    let mut deliverable = Vec::new();
    for row in due {
        if let Some(ancestor) = inhibiting_ancestor(ctx, &row).await? {
            tracing::debug!(
                notification_id = row.id,
                device_id = row.device_id,
                ancestor,
                "notificação inibida: o pai do dispositivo está em alerta"
            );
            mark(
                ctx,
                &[row.id],
                SUPPRESSED,
                Some(SuppressReason::Inhibited),
                now,
            )
            .await?;
            stats.suppressed += 1;
            continue;
        }
        deliverable.push(row);
    }
    if deliverable.is_empty() {
        return Ok(stats);
    }

    // Uma instância só de canais por passagem: antes da Fase 4, cada alerta
    // relia o ambiente para montar os quatro destinos do zero.
    let channels = NotificationService::with_default_channels();
    for (group, rows) in by_group(deliverable) {
        let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
        let message = if rows.len() == 1 {
            stats.delivered += 1;
            single_message(&rows[0])
        } else {
            stats.delivered += 1;
            stats.consolidated += rows.len() as u64;
            let label = group_label(&ctx.db, &group).await?;
            formatter::alert_digest(&label, &digest_items(&rows), highest_severity(&rows))
        };
        channels.notify(ctx, &message).await;
        mark(ctx, &ids, SENT, None, now).await?;
    }

    Ok(stats)
}

/// O ancestral em alerta que justifica engolir esta linha.
async fn inhibiting_ancestor(
    ctx: &AppContext,
    row: &notification_outbox::Model,
) -> AppResult<Option<i64>> {
    if !row.inhibitable {
        return Ok(None);
    }
    let Some(device_id) = row.device_id else {
        return Ok(None);
    };
    inhibition::explaining_ancestor(&ctx.db, device_id).await
}

/// Agrupa preservando a ordem de chegada dos grupos e das linhas.
fn by_group(
    rows: Vec<notification_outbox::Model>,
) -> Vec<(String, Vec<notification_outbox::Model>)> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<notification_outbox::Model>> = HashMap::new();
    for row in rows {
        let key = row.group_key.clone();
        if !grouped.contains_key(&key) {
            order.push(key.clone());
        }
        grouped.entry(key).or_default().push(row);
    }
    order
        .into_iter()
        .filter_map(|key| grouped.remove(&key).map(|rows| (key, rows)))
        .collect()
}

fn single_message(row: &notification_outbox::Model) -> NotificationMessage {
    NotificationMessage {
        title: row.title.clone(),
        body: row.body.clone(),
        severity: Severity::parse(&row.severity),
        metadata: row.metadata.clone(),
    }
}

/// Os pares (título, corpo) que a mensagem consolidada enumera.
fn digest_items(rows: &[notification_outbox::Model]) -> Vec<(String, String)> {
    rows.iter()
        .map(|row| (row.title.clone(), row.body.clone()))
        .collect()
}

/// A maior severidade do lote manda no cabeçalho: um crítico no meio de oito
/// avisos não pode chegar rotulado como aviso.
fn highest_severity(rows: &[notification_outbox::Model]) -> Severity {
    rows.iter()
        .map(|row| Severity::parse(&row.severity))
        .max_by_key(|severity| match severity {
            Severity::Info => 0,
            Severity::Warning => 1,
            Severity::Critical => 2,
        })
        .unwrap_or(Severity::Warning)
}

/// Nome legível do grupo, resolvido só quando há mensagem consolidada.
async fn group_label<C: ConnectionTrait>(db: &C, group: &str) -> AppResult<String> {
    if let Some(id) = group
        .strip_prefix("site:")
        .and_then(|id| id.parse::<i64>().ok())
    {
        if let Some(site) = sites::Entity::find_by_id(id).one(db).await? {
            return Ok(site.name);
        }
    }
    if let Some(id) = group
        .strip_prefix("device:")
        .and_then(|id| id.parse::<i64>().ok())
    {
        if let Some(device) = devices::Entity::find_by_id(id).one(db).await? {
            return Ok(device.name);
        }
    }
    Ok("vários alvos".to_string())
}

/// Marca as linhas com o desfecho. `sent_at` é carimbado nos dois casos: o
/// cooldown e o agrupamento medem do instante da **decisão**, e uma linha
/// suprimida por inibição não deve reabrir a janela do grupo.
async fn mark(
    ctx: &AppContext,
    ids: &[i64],
    status: &str,
    reason: Option<SuppressReason>,
    now: DateTime<Utc>,
) -> AppResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let stamp: sea_orm::prelude::DateTimeWithTimeZone = now.into();
    notification_outbox::Entity::update_many()
        .col_expr(
            notification_outbox::Column::Status,
            sea_orm::sea_query::Expr::value(status),
        )
        .col_expr(
            notification_outbox::Column::SuppressReason,
            sea_orm::sea_query::Expr::value(reason.map(|reason| reason.as_str())),
        )
        .col_expr(
            notification_outbox::Column::SentAt,
            sea_orm::sea_query::Expr::value(if status == SENT { Some(stamp) } else { None }),
        )
        .filter(notification_outbox::Column::Id.is_in(ids.to_vec()))
        .exec(&ctx.db)
        .await?;
    Ok(())
}

/// Filtro do par (regra, alvo). `NULL` precisa de `IS NULL`: `= NULL` nunca
/// casa, e o par sem regra existe — uma regra apagada deixa o histórico dela.
fn pair_filter(alert_rule_id: Option<i64>, scope_key: Option<&str>) -> Condition {
    let by_rule = match alert_rule_id {
        Some(id) => notification_outbox::Column::AlertRuleId.eq(id),
        None => notification_outbox::Column::AlertRuleId.is_null(),
    };
    let by_scope = match scope_key {
        Some(key) => notification_outbox::Column::ScopeKey.eq(key),
        None => notification_outbox::Column::ScopeKey.is_null(),
    };
    Condition::all().add(by_rule).add(by_scope)
}

/// O que já foi entregue para o par (regra, alvo).
///
/// Duas consultas curtas em vez de uma varredura: a última mensagem entregue
/// (que diz se há problema anunciado em aberto) e a última mensagem de
/// **problema** (que ancora o cooldown). O `now` entra só para não considerar
/// entregas com carimbo no futuro, que um relógio desacertado poderia gravar.
async fn delivery_log<C: ConnectionTrait>(
    db: &C,
    alert_rule_id: Option<i64>,
    scope_key: Option<&str>,
    now: DateTime<Utc>,
) -> AppResult<DeliveryLog> {
    let base = || {
        notification_outbox::Entity::find()
            .filter(pair_filter(alert_rule_id, scope_key))
            .filter(notification_outbox::Column::Status.eq(SENT))
            .filter(notification_outbox::Column::SentAt.lte(now))
            .order_by_desc(notification_outbox::Column::Id)
    };

    let announced = base()
        .one(db)
        .await?
        .and_then(|row| row.kind.parse::<NotificationKind>().ok())
        .is_some_and(NotificationKind::announces_problem);

    let last_problem_at = base()
        .filter(notification_outbox::Column::Kind.is_in([
            NotificationKind::Problem.as_str(),
            NotificationKind::Flapping.as_str(),
        ]))
        .one(db)
        .await?
        .and_then(|row| row.sent_at)
        .map(|at| at.with_timezone(&Utc));

    Ok(DeliveryLog {
        last_problem_at,
        announced,
    })
}

/// Última entrega de qualquer mensagem do grupo correlato.
async fn group_last_sent_at<C: ConnectionTrait>(
    db: &C,
    group: &str,
) -> AppResult<Option<DateTime<Utc>>> {
    Ok(notification_outbox::Entity::find()
        .filter(notification_outbox::Column::GroupKey.eq(group))
        .filter(notification_outbox::Column::Status.eq(SENT))
        .order_by_desc(notification_outbox::Column::Id)
        .one(db)
        .await?
        .and_then(|row| row.sent_at)
        .map(|at| at.with_timezone(&Utc)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn linha(id: i64, group: &str, severity: &str) -> notification_outbox::Model {
        let now = Utc::now().into();
        notification_outbox::Model {
            id,
            alert_rule_id: Some(1),
            scope_key: Some(format!("monitor:{id}")),
            device_id: None,
            group_key: group.into(),
            kind: "problem".into(),
            title: format!("Alerta {id}"),
            body: format!("Corpo {id}"),
            severity: severity.into(),
            metadata: json!({}),
            status: PENDING.into(),
            suppress_reason: None,
            inhibitable: false,
            deliver_after: now,
            sent_at: None,
            created_at: now,
        }
    }

    #[test]
    fn o_grupo_prefere_o_site_e_cai_para_o_dispositivo() {
        assert_eq!(group_key(Some(3), Some(12)), "site:3");
        assert_eq!(group_key(None, Some(12)), "device:12");
        assert_eq!(group_key(None, None), "global");
    }

    #[test]
    fn o_agrupamento_preserva_a_ordem_de_chegada() {
        let grupos = by_group(vec![
            linha(1, "site:1", "warning"),
            linha(2, "site:2", "warning"),
            linha(3, "site:1", "warning"),
        ]);
        let chaves: Vec<&str> = grupos.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(chaves, ["site:1", "site:2"]);
        assert_eq!(grupos[0].1.len(), 2);
        assert_eq!(grupos[1].1.len(), 1);
    }

    #[test]
    fn a_maior_severidade_do_lote_manda_no_cabecalho() {
        let rows = [
            linha(1, "site:1", "info"),
            linha(2, "site:1", "critical"),
            linha(3, "site:1", "warning"),
        ];
        assert_eq!(highest_severity(&rows), Severity::Critical);
        assert_eq!(highest_severity(&rows[..1]), Severity::Info);
        assert_eq!(highest_severity(&[]), Severity::Warning);
    }

    #[test]
    fn a_mensagem_avulsa_e_a_propria_linha() {
        let message = single_message(&linha(7, "site:1", "critical"));
        assert_eq!(message.title, "Alerta 7");
        assert_eq!(message.body, "Corpo 7");
        assert_eq!(message.severity, Severity::Critical);
    }

    #[test]
    fn as_estatisticas_somam_os_tres_desfechos() {
        let stats = DispatchStats {
            delivered: 2,
            consolidated: 8,
            suppressed: 3,
        };
        assert_eq!(stats.total(), 13);
        assert_eq!(DispatchStats::default().total(), 0);
    }
}
