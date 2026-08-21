//! Regras de alerta, catálogo e Central de Alertas (§7.11).

use axum::{extract::Query, http::HeaderMap, http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::{json, Value};

use crate::{
    dtos::{
        optional_body,
        resources::{
            AlertRuleInput, AlertRuleScopeQuery, CatalogApplyInput, InstabilityQuery,
            PaginationQuery, SilenceInput,
        },
    },
    models::{
        _entities::alert_events as alert_events_entity, alert_events, alert_rules, devices,
        monitors,
    },
    services::{
        alerts::{
            catalog::{
                service::{self as catalog, TemplateScope},
                templates,
            },
            contracts::AlertStatus,
            correlation, instability, silence,
        },
        audit::{
            AuditAction, AuditActor, AuditChanges, AuditEntryInput, AuditService, ResourceType,
        },
        devices::capabilities,
        events::EventBus,
        monitoring::{
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
        },
        shared::{
            errors::{AppError, AppResult},
            pagination::{normalize_limit, normalize_page, paginate_compat, LucidMeta, LucidPage},
        },
    },
    views::alerts::{
        rule_event_payload, serialize_event, serialize_events, AlertRelations, AlertRuleResponse,
        SerializedAlertEvent,
    },
};

/// Teto do modo array de `GET /api/alerts` (§5.4).
const ALERTS_ARRAY_LIMIT: u64 = 100;

/// O front envia a regra em linguagem simples (métrica/operador/valor); aqui
/// garantimos o formato `{field, operator, value}` esperado pelo avaliador.
fn normalize_condition(condition: &Value) -> Option<Value> {
    let object = condition.as_object()?;
    let field = object.get("field")?.as_str()?;
    let operator = object.get("operator")?.as_str()?;
    Some(json!({
        "field": field,
        "operator": operator,
        "value": object.get("value").cloned().unwrap_or(Value::Null),
    }))
}

fn invalid_condition() -> AppError {
    AppError::validation(
        "Condição inválida. Informe a métrica alvo, a comparação e o valor de referência da regra.",
    )
}

/// A janela de recuperação é um tempo de estabilidade: negativo não faz
/// sentido e seria tratado como zero pela máquina de estados — melhor recusar
/// do que gravar um valor que não faz o que diz.
fn invalid_recovery_window() -> AppError {
    AppError::validation(
        "Janela de recuperação inválida. Informe zero ou mais segundos de estabilidade exigida.",
    )
}

/// Mesma lógica da janela de recuperação: os limiares de flapping são
/// contagens e tempos: negativos seriam tratados como "desligado" pela máquina
/// de estados, e gravar um valor que não faz o que diz confunde quem configura.
fn invalid_flap_settings() -> AppError {
    AppError::validation(
        "Limiar de oscilação inválido. Informe zero ou mais recaídas e uma janela não negativa.",
    )
}

/// O cooldown é um intervalo de silêncio: negativo seria lido como "desligado"
/// pela política de notificação — mesma recusa das outras janelas.
fn invalid_cooldown() -> AppError {
    AppError::validation(
        "Intervalo entre notificações inválido. Informe zero ou mais segundos de silêncio.",
    )
}

/// Aplica um campo opcional não negativo: ausente mantém o atual.
fn non_negative(
    input: Option<i32>,
    current: i32,
    invalid: fn() -> AppError,
) -> Result<i32, AppError> {
    match input {
        Some(value) if value < 0 => Err(invalid()),
        Some(value) => Ok(value),
        None => Ok(current),
    }
}

/// Publicação de evento é best-effort: o CRUD já concluiu quando chegamos aqui.
async fn publish(ctx: &AppContext, kind: &str, payload: Value) {
    if let Ok(bus) = EventBus::from_context(ctx) {
        if let Err(error) = bus.publish(&ctx.db, kind, payload).await {
            tracing::warn!(%error, event = kind, "falha ao publicar evento de regra");
        }
    }
}

// --- Regras -----------------------------------------------------------------

/// Lista as regras, opcionalmente recortadas por escopo.
///
/// Sem parâmetro devolve tudo — a Central de Alertas continua sendo a fonte
/// única da verdade. Com `deviceId`, devolve as regras **daquele** dispositivo
/// e as dos monitores dele: é a mesma lista que a aba Regras da página do
/// dispositivo mostra, e não uma segunda listagem. Com `includeGlobal`, junta
/// as regras sem escopo — que valem para o parque inteiro e, portanto, também
/// para este equipamento.
async fn rules_index(
    State(ctx): State<AppContext>,
    Query(scope): Query<AlertRuleScopeQuery>,
) -> AppResult<Response> {
    let mut query = alert_rules::Entity::find_ordered();
    if let Some(device_id) = scope.device_id {
        let monitor_ids: Vec<i64> = monitors::Entity::find()
            .filter(monitors::Column::DeviceId.eq(device_id))
            .all(&ctx.db)
            .await?
            .into_iter()
            .map(|monitor| monitor.id)
            .collect();
        let mut condicao = Condition::any().add(alert_rules::Column::DeviceId.eq(device_id));
        if !monitor_ids.is_empty() {
            condicao = condicao.add(alert_rules::Column::MonitorId.is_in(monitor_ids));
        }
        // Uma regra global **também** é avaliada nas checagens deste
        // equipamento: escondê-la da aba dele mostraria "nenhuma regra" a quem
        // acabou de criar uma. Quem pede o acréscimo é a tela, porque em outros
        // recortes "só deste dispositivo" continua sendo a pergunta certa.
        if scope.include_global.unwrap_or(false) {
            condicao = condicao.add(
                Condition::all()
                    .add(alert_rules::Column::DeviceId.is_null())
                    .add(alert_rules::Column::MonitorId.is_null())
                    .add(alert_rules::Column::SiteId.is_null()),
            );
        }
        query = query.filter(condicao);
    }
    if let Some(monitor_id) = scope.monitor_id {
        query = query.filter(alert_rules::Column::MonitorId.eq(monitor_id));
    }
    if let Some(site_id) = scope.site_id {
        query = query.filter(alert_rules::Column::SiteId.eq(site_id));
    }
    let rules = query.all(&ctx.db).await?;
    Ok(format::json(
        rules
            .into_iter()
            .map(AlertRuleResponse::from)
            .collect::<Vec<_>>(),
    )?)
}

async fn rules_store(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<AlertRuleInput>,
) -> AppResult<Response> {
    let condition = input
        .condition
        .as_ref()
        .and_then(normalize_condition)
        .ok_or_else(invalid_condition)?;
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::validation("Nome da regra é obrigatório"))?;
    let recovery_window_seconds = input.recovery_window_seconds.unwrap_or(0);
    if recovery_window_seconds < 0 {
        return Err(invalid_recovery_window());
    }
    // Default da coluna: detecção desligada, mas com uma janela sensata já
    // pronta para quem só ligar o limiar depois.
    let flap_threshold = non_negative(input.flap_threshold, 0, invalid_flap_settings)?;
    let flap_window_seconds = non_negative(input.flap_window_seconds, 900, invalid_flap_settings)?;
    let notification_cooldown_seconds =
        non_negative(input.notification_cooldown_seconds, 0, invalid_cooldown)?;

    let rule = alert_rules::ActiveModel {
        // No `POST`, campo ausente e `null` significam a mesma coisa — a regra
        // nasce sem aquela dimensão de escopo.
        site_id: Set(input.site_id.flatten()),
        device_id: Set(input.device_id.flatten()),
        monitor_id: Set(input.monitor_id.flatten()),
        name: Set(name.to_string()),
        r#type: Set(input.rule_type.unwrap_or_else(|| "custom".into())),
        condition: Set(condition),
        severity: Set(input.severity.unwrap_or_else(|| "warning".into())),
        duration_seconds: Set(input.duration_seconds.unwrap_or(0)),
        recovery_window_seconds: Set(recovery_window_seconds),
        flap_threshold: Set(flap_threshold),
        flap_window_seconds: Set(flap_window_seconds),
        notification_cooldown_seconds: Set(notification_cooldown_seconds),
        inhibit_when_parent_down: Set(input.inhibit_when_parent_down.unwrap_or(false)),
        enabled: Set(input.enabled.unwrap_or(true)),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    publish(&ctx, "alert_rule:created", rule_event_payload(&rule)).await;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Create,
                resource_type: ResourceType::AlertRule,
                resource_id: Some(rule.id),
                resource_label: Some(rule.name.clone()),
                description: Some(format!("Regra de alerta '{}' criada", rule.name)),
                changes: None,
            },
        )
        .await;

    Ok((StatusCode::CREATED, Json(AlertRuleResponse::from(rule))).into_response())
}

async fn rules_update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<AlertRuleInput>,
) -> AppResult<Response> {
    let current = alert_rules::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Regra de alerta não encontrada"))?;
    let old_response = AlertRuleResponse::from(current.clone());

    // Campo ausente mantém o valor atual: o toggle da lista manda só `enabled`.
    let condition = match input.condition.as_ref() {
        Some(raw) => normalize_condition(raw).ok_or_else(invalid_condition)?,
        None => current.condition.clone(),
    };
    let recovery_window_seconds = match input.recovery_window_seconds {
        Some(value) if value < 0 => return Err(invalid_recovery_window()),
        Some(value) => value,
        None => current.recovery_window_seconds,
    };
    let flap_threshold = non_negative(
        input.flap_threshold,
        current.flap_threshold,
        invalid_flap_settings,
    )?;
    let flap_window_seconds = non_negative(
        input.flap_window_seconds,
        current.flap_window_seconds,
        invalid_flap_settings,
    )?;
    let notification_cooldown_seconds = non_negative(
        input.notification_cooldown_seconds,
        current.notification_cooldown_seconds,
        invalid_cooldown,
    )?;

    let rule = alert_rules::ActiveModel {
        id: Set(id),
        // `unwrap_or` sobre a dupla opção: campo ausente mantém o atual,
        // `null` explícito limpa. Ver a nota do `AlertRuleInput`.
        site_id: Set(input.site_id.unwrap_or(current.site_id)),
        device_id: Set(input.device_id.unwrap_or(current.device_id)),
        monitor_id: Set(input.monitor_id.unwrap_or(current.monitor_id)),
        name: Set(input
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&current.name)
            .to_string()),
        r#type: Set(input.rule_type.unwrap_or(current.r#type)),
        condition: Set(condition),
        severity: Set(input.severity.unwrap_or(current.severity)),
        duration_seconds: Set(input.duration_seconds.unwrap_or(current.duration_seconds)),
        recovery_window_seconds: Set(recovery_window_seconds),
        flap_threshold: Set(flap_threshold),
        flap_window_seconds: Set(flap_window_seconds),
        notification_cooldown_seconds: Set(notification_cooldown_seconds),
        inhibit_when_parent_down: Set(input
            .inhibit_when_parent_down
            .unwrap_or(current.inhibit_when_parent_down)),
        enabled: Set(input.enabled.unwrap_or(current.enabled)),
        ..Default::default()
    }
    .update(&ctx.db)
    .await?;

    publish(&ctx, "alert_rule:updated", rule_event_payload(&rule)).await;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Update,
                resource_type: ResourceType::AlertRule,
                resource_id: Some(rule.id),
                resource_label: Some(rule.name.clone()),
                description: Some(format!("Regra de alerta '{}' atualizada", rule.name)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(old_response).ok(),
                    new: serde_json::to_value(AlertRuleResponse::from(rule.clone())).ok(),
                }),
            },
        )
        .await;

    Ok(format::json(AlertRuleResponse::from(rule))?)
}

async fn rules_destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let rule = alert_rules::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Regra de alerta não encontrada"))?;
    // O payload é montado antes do DELETE: depois dele a linha não existe mais.
    let payload = rule_event_payload(&rule);
    let old_response = AlertRuleResponse::from(rule.clone());
    let old_name = rule.name.clone();
    alert_rules::Entity::delete_by_id(id).exec(&ctx.db).await?;
    publish(&ctx, "alert_rule:deleted", payload).await;

    let _ = AuditService::new(&ctx.db)
        .log(
            AuditActor::from_headers(&headers, &ctx.db)
                .await
                .unwrap_or_default(),
            AuditEntryInput {
                action: AuditAction::Delete,
                resource_type: ResourceType::AlertRule,
                resource_id: Some(id),
                resource_label: Some(old_name.clone()),
                description: Some(format!("Regra de alerta '{}' excluída", old_name)),
                changes: Some(AuditChanges {
                    old: serde_json::to_value(old_response).ok(),
                    new: None,
                }),
            },
        )
        .await;

    Ok(StatusCode::NO_CONTENT.into_response())
}

// --- Catálogo ---------------------------------------------------------------

/// O catálogo, global ou já fixado num dispositivo.
///
/// É **um** catálogo, não dois: abrir pela página do dispositivo apenas
/// preenche o escopo e filtra pela aplicabilidade; abrir por `/alerts` deixa o
/// operador escolher. O mesmo componente do frontend serve às duas telas.
async fn catalog_index(
    State(ctx): State<AppContext>,
    Query(scope): Query<AlertRuleScopeQuery>,
) -> AppResult<Response> {
    let categories: serde_json::Map<String, Value> = templates::CATEGORY_LABELS
        .iter()
        .map(|(key, label)| ((*key).to_string(), json!(label)))
        .collect();

    let escopo = TemplateScope {
        site_id: scope.site_id,
        device_id: scope.device_id,
        monitor_id: scope.monitor_id,
    };
    let capacidades = match scope.device_id {
        Some(device_id) => {
            let device = devices::Entity::find_by_id(device_id)
                .one(&ctx.db)
                .await?
                .ok_or_else(|| AppError::not_found("Dispositivo não encontrado"))?;
            Some(capabilities::for_device(&ctx.db, &device).await?)
        }
        None => None,
    };

    Ok(format::json(json!({
        "categories": categories,
        "templates": catalog::describe_for(&ctx.db, escopo, capacidades.as_ref()).await?,
    }))?)
}

async fn catalog_apply(
    State(ctx): State<AppContext>,
    Json(input): Json<CatalogApplyInput>,
) -> AppResult<Response> {
    let keys = input.keys.unwrap_or_default();
    if keys.is_empty() {
        return Err(AppError::validation(
            "Selecione ao menos uma regra pré-configurada para aplicar.",
        ));
    }

    let escopo = TemplateScope {
        site_id: input.site_id,
        device_id: input.device_id,
        monitor_id: input.monitor_id,
    };
    let result = catalog::apply_scoped(&ctx.db, &keys, escopo).await?;
    for rule in &result.created {
        publish(&ctx, "alert_rule:created", rule_event_payload(rule)).await;
    }

    let created: Vec<AlertRuleResponse> = result
        .created
        .into_iter()
        .map(AlertRuleResponse::from)
        .collect();
    Ok((
        StatusCode::CREATED,
        Json(json!({ "created": created, "skipped": result.skipped })),
    )
        .into_response())
}

// --- Eventos ----------------------------------------------------------------

/// Modo dual (§5.4): array cru sem `?page`, envelope paginado com ele.
///
/// A aba "Ativos" carrega a lista curta de uma vez (filtra e ordena no
/// cliente); o histórico completo — que cresce sem teto — pede página a página.
async fn index(
    State(ctx): State<AppContext>,
    Query(query): Query<PaginationQuery>,
) -> AppResult<Response> {
    let base = alert_events::Entity::find().order_by_desc(alert_events_entity::Column::Id);

    let Some(page) = query.page else {
        let events = base.limit(ALERTS_ARRAY_LIMIT).all(&ctx.db).await?;
        return Ok(format::json(serialize_events(&ctx.db, events).await?)?);
    };

    let page = normalize_page(Some(page));
    let limit = normalize_limit(query.limit);
    let paginator = base.paginate(&ctx.db, limit);
    let total = paginator.num_items().await?;
    let events = paginator.fetch_page(page.saturating_sub(1)).await?;
    Ok(format::json(LucidPage {
        data: serialize_events(&ctx.db, events).await?,
        meta: LucidMeta::new(total, limit, page),
    })?)
}

/// `GET /api/alerts/instability` — "este link oscilou 12x nas últimas 24h".
///
/// O indicador que a página do monitor e o dashboard consomem (Fase 3). Sem
/// `scopeKey`, o ranking dos alvos mais instáveis da janela.
async fn instability_index(
    State(ctx): State<AppContext>,
    Query(query): Query<InstabilityQuery>,
) -> AppResult<Response> {
    Ok(format::json(
        instability::load(
            &ctx.db,
            query.hours.unwrap_or(instability::DEFAULT_HOURS),
            query.scope_key.as_deref(),
        )
        .await?,
    )?)
}

/// Executa a checagem em tempo real do alvo do alerta (se for um monitor) e,
/// se ele recuperou, o motor move o alerta na máquina de estados — resolvendo
/// ou mandando para observação de estabilidade, conforme a janela da regra.
///
/// Devolve o evento recarregado do banco: o `process_result` pode tê-lo
/// resolvido por dentro, e responder com a cópia antiga mostraria ao operador
/// um alerta ativo que já não existe.
async fn check_and_resolve(
    ctx: &AppContext,
    event: alert_events::Model,
) -> AppResult<alert_events::Model> {
    if event.status == AlertStatus::Resolved.as_str() {
        return Ok(event);
    }
    let Some(monitor_id) = event.monitor_id else {
        return Ok(event);
    };
    let Some(monitor) = monitors::Entity::find_by_id(monitor_id)
        .one(&ctx.db)
        .await?
    else {
        return Ok(event);
    };
    if !monitor.enabled {
        return Ok(event);
    }

    let Some(_guard) =
        crate::services::monitoring::execution_guard::try_acquire_monitor(monitor.id)
    else {
        tracing::debug!(
            monitor_id,
            "verificação do alerta ignorada: monitor já em execução"
        );
        return Ok(event);
    };

    // Falha de execução não invalida o alerta: mantém-se a avaliação pelo
    // estado atual, exatamente como no backend anterior.
    match run_monitor(
        ctx,
        &monitor.r#type,
        &monitor.configuration,
        RunOptions {
            timeout_ms: Some(
                crate::services::monitoring::execution_guard::calculate_smart_timeout_seconds(
                    &monitor.r#type,
                    monitor.interval_seconds,
                ) as u64
                    * 1_000,
            ),
        },
    )
    .await
    {
        Ok(result) => {
            process_result(ctx, monitor.id, &result, monitor.probe_id).await?;
        }
        Err(error) => {
            tracing::warn!(%error, monitor_id, "verificação do alerta não pôde ser executada");
            return Ok(event);
        }
    }

    Ok(alert_events::Entity::find_by_id(event.id)
        .one(&ctx.db)
        .await?
        .unwrap_or(event))
}

async fn load_event(ctx: &AppContext, id: i64) -> AppResult<alert_events::Model> {
    alert_events::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or_else(|| AppError::not_found("Alerta não encontrado"))
}

async fn serialize_one(
    ctx: &AppContext,
    event: &alert_events::Model,
) -> AppResult<SerializedAlertEvent> {
    let relations = AlertRelations::load(&ctx.db, std::slice::from_ref(event)).await?;
    Ok(serialize_event(event, &relations))
}

async fn acknowledge(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let event = check_and_resolve(&ctx, load_event(&ctx, id).await?).await?;

    if event.status == AlertStatus::Resolved.as_str() {
        return Ok(format::json(json!({
            "message": format!("Alerta #{id} foi verificado e resolvido automaticamente"),
            "event": serialize_one(&ctx, &event).await?,
            "resolved": true,
        }))?);
    }

    let event = silence::acknowledge_alert(&ctx.db, event).await?;
    publish(
        &ctx,
        "alert:acknowledged",
        json!({
            "id": event.id,
            "alertEventId": event.id,
            "monitorId": event.monitor_id,
            "deviceId": event.device_id,
            "status": event.status,
            "severity": event.severity,
            "message": event.message,
        }),
    )
    .await;

    Ok(format::json(json!({
        "message": format!("Alerta #{id} reconhecido"),
        "event": serialize_one(&ctx, &event).await?,
        "resolved": false,
    }))?)
}

async fn verify(State(ctx): State<AppContext>, Path(id): Path<i64>) -> AppResult<Response> {
    let event = check_and_resolve(&ctx, load_event(&ctx, id).await?).await?;
    let resolved = event.status == AlertStatus::Resolved.as_str();
    Ok(format::json(json!({
        "message": if resolved {
            format!("Alerta #{id} resolvido com sucesso!")
        } else {
            format!("Alerta #{id} continua ativo.")
        },
        "event": serialize_one(&ctx, &event).await?,
        "resolved": resolved,
    }))?)
}

async fn verify_all(State(ctx): State<AppContext>) -> AppResult<Response> {
    let pending = alert_events::Entity::find()
        .filter(alert_events_entity::Column::Status.is_in(AlertStatus::OPEN))
        .all(&ctx.db)
        .await?;
    let total_checked = pending.len();
    let mut resolved_count = 0;
    for event in pending {
        if check_and_resolve(&ctx, event).await?.status == AlertStatus::Resolved.as_str() {
            resolved_count += 1;
        }
    }
    Ok(format::json(json!({
        "message": format!("{resolved_count} de {total_checked} alerta(s) pendente(s) resolvido(s)"),
        "totalChecked": total_checked,
        "resolvedCount": resolved_count,
    }))?)
}

async fn silence_alert(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
    body: String,
) -> AppResult<Response> {
    // Corpo opcional: sem `minutes` o silêncio é de 60 minutos, e um 404 de
    // alerta inexistente não pode virar 400 por falta de payload.
    let input: SilenceInput = optional_body(&body);
    let duration = input
        .minutes
        .or(input.duration_minutes)
        .filter(|value| *value > 0)
        .unwrap_or(silence::DEFAULT_SILENCE_MINUTES);
    let event = silence::silence_alert(&ctx.db, load_event(&ctx, id).await?, duration).await?;
    let serialized = serialize_one(&ctx, &event).await?;

    publish(
        &ctx,
        "alert:silenced",
        json!({
            "id": event.id,
            "alertEventId": event.id,
            "monitorId": event.monitor_id,
            "deviceId": event.device_id,
            "status": event.status,
            "severity": event.severity,
            "message": event.message,
            "silencedUntil": serialized.silenced_until,
            "durationMinutes": duration,
        }),
    )
    .await;

    Ok(format::json(json!({
        "message": format!("Alerta #{id} silenciado por {duration} minutos"),
        "event": serialized,
    }))?)
}

/// `GET /api/monitors/:id/alerts` — histórico do monitor, incluindo os
/// resolvidos. Sempre paginado, no envelope `{ data, meta }`.
///
/// A busca é por `monitorId` **ou** `scopeKey`: eventos antigos só têm a
/// coluna, e eventos de escopo só têm a chave.
///
/// # Errors
///
/// Propaga erro do banco.
pub async fn alerts_for_monitor(
    ctx: &AppContext,
    monitor_id: i64,
    query: &PaginationQuery,
) -> AppResult<LucidPage<SerializedAlertEvent>> {
    let filter = Condition::any()
        .add(alert_events_entity::Column::MonitorId.eq(monitor_id))
        .add(alert_events_entity::Column::ScopeKey.eq(format!("monitor:{monitor_id}")));
    let page = paginate_compat(
        &ctx.db,
        alert_events::Entity::find()
            .filter(filter)
            .order_by_desc(alert_events_entity::Column::CreatedAt),
        query.page.unwrap_or(1),
        query.limit.unwrap_or(20),
        |row| row,
    )
    .await?;
    Ok(LucidPage {
        data: serialize_events(&ctx.db, page.data).await?,
        meta: page.meta,
    })
}

/// `GET /api/alerts/{id}/correlation` — sugere causa raiz comum.
///
/// Analisa eventos abertos numa janela curta em torno do alerta e devolve o
/// evento mais provável de ser a causa raiz (tipicamente um pai de infraestrutura
/// que caiu primeiro), além dos eventos relacionados.
async fn correlation_index(
    State(ctx): State<AppContext>,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let result = correlation::analyze(&ctx.db, id, None).await?;
    Ok(format::json(result)?)
}

/// `/api/alert-rules` e o catálogo.
///
/// `/catalog` é registrado antes de `/{id}` porque o roteador casa o caminho
/// estático primeiro — mas deixar explícito evita que uma reordenação futura
/// transforme "catalog" num id inválido.
pub fn rules_routes() -> Routes {
    Routes::new()
        .prefix("/alert-rules")
        .add("/catalog", get(catalog_index).post(catalog_apply))
        .add("/", get(rules_index).post(rules_store))
        .add("/{id}", put(rules_update).delete(rules_destroy))
}

/// `/api/alerts` — a Central de Alertas.
pub fn routes() -> Routes {
    Routes::new()
        .prefix("/alerts")
        .add("/", get(index))
        .add("/instability", get(instability_index))
        .add("/verify-all", post(verify_all))
        .add("/{id}/acknowledge", post(acknowledge))
        .add("/{id}/verify", post(verify))
        .add("/{id}/silence", post(silence_alert))
        .add("/{id}/correlation", get(correlation_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condicao_e_normalizada_para_os_tres_campos() {
        let normalized = normalize_condition(&json!({
            "field": "latencyMs", "operator": "gt", "value": 200, "extra": "ignorado"
        }))
        .expect("condição válida");
        assert_eq!(
            normalized,
            json!({ "field": "latencyMs", "operator": "gt", "value": 200 })
        );
    }

    #[test]
    fn condicao_sem_valor_vira_null_e_nao_erro() {
        let normalized = normalize_condition(&json!({ "field": "status", "operator": "eq" }))
            .expect("field e operator bastam");
        assert_eq!(normalized["value"], Value::Null);
    }

    #[test]
    fn condicao_invalida_e_recusada() {
        assert!(normalize_condition(&json!({ "operator": "eq", "value": 1 })).is_none());
        assert!(normalize_condition(&json!({ "field": "status", "operator": 5 })).is_none());
        assert!(normalize_condition(&json!(["status", "eq", 1])).is_none());
        assert!(normalize_condition(&Value::Null).is_none());
    }

    #[test]
    fn condicao_invalida_devolve_422_com_o_texto_do_backend_anterior() {
        assert_eq!(
            invalid_condition().status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert!(invalid_condition()
            .to_string()
            .starts_with("Condição inválida."));
    }
}
