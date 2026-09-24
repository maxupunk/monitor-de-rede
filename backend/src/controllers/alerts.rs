//! Regras de alerta, catálogo e Central de Alertas (§7.11).

use axum::{extract::Query, http::HeaderMap, http::StatusCode, response::IntoResponse};
use loco_rs::prelude::*;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
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
            actions as alert_actions,
            catalog::{
                service::{self as catalog, TemplateScope},
                templates,
            },
            contracts::AlertStatus,
            correlation, instability, rules, silence,
        },
        audit::AuditActor,
        devices::capabilities,
        monitoring::{
            result_processor::process_result,
            runner::{run_monitor, RunOptions},
        },
        shared::{
            errors::{AppError, AppResult},
            pagination::{
                normalize_limit, normalize_page, paginate, PaginatedResponse, PaginationMeta,
            },
        },
    },
    views::alerts::{
        rule_event_payload, serialize_event, serialize_events, AlertRelations, AlertRuleResponse,
        SerializedAlertEvent,
    },
};

/// Teto do modo array de `GET /api/alerts` (§5.4).
const ALERTS_ARRAY_LIMIT: u64 = 100;

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
    let actor = AuditActor::from_headers(&headers, &ctx.db)
        .await
        .unwrap_or_default();
    let rule = rules::create(&ctx, input, actor).await?;
    Ok((StatusCode::CREATED, Json(AlertRuleResponse::from(rule))).into_response())
}

async fn rules_update(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<AlertRuleInput>,
) -> AppResult<Response> {
    let actor = AuditActor::from_headers(&headers, &ctx.db)
        .await
        .unwrap_or_default();
    let rule = rules::update(&ctx, id, input, actor).await?;
    Ok(format::json(AlertRuleResponse::from(rule))?)
}

async fn rules_destroy(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let actor = AuditActor::from_headers(&headers, &ctx.db)
        .await
        .unwrap_or_default();
    rules::delete(&ctx, id, actor).await?;
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
        rules::publish(&ctx, "alert_rule:created", rule_event_payload(rule)).await;
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
    Ok(format::json(PaginatedResponse {
        data: serialize_events(&ctx.db, events).await?,
        meta: PaginationMeta::new(total, limit, page),
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
    // estado atual, que continua sendo a fonte canônica.
    let execution_configuration =
        crate::services::monitoring::ping_diagnostics::prepare_configuration(ctx, &monitor).await?;
    match run_monitor(
        ctx,
        &monitor.r#type,
        &execution_configuration,
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

    let event = alert_actions::acknowledge(&ctx, event).await?;

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
    let event = alert_actions::silence(&ctx, load_event(&ctx, id).await?, duration).await?;
    let serialized = serialize_one(&ctx, &event).await?;

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
) -> AppResult<PaginatedResponse<SerializedAlertEvent>> {
    let filter = Condition::any()
        .add(alert_events_entity::Column::MonitorId.eq(monitor_id))
        .add(alert_events_entity::Column::ScopeKey.eq(format!("monitor:{monitor_id}")));
    let page = paginate(
        &ctx.db,
        alert_events::Entity::find()
            .filter(filter)
            .order_by_desc(alert_events_entity::Column::CreatedAt),
        query.page.unwrap_or(1),
        query.limit.unwrap_or(20),
        |row| row,
    )
    .await?;
    Ok(PaginatedResponse {
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

/// `GET /api/alerts/root-cause-analysis` — agrupamento e diagnóstico global de incidentes ativos.
async fn root_cause_analysis_index(State(ctx): State<AppContext>) -> AppResult<Response> {
    let result = correlation::analyze_active_clusters(&ctx.db, None).await?;
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
        .add("/root-cause-analysis", get(root_cause_analysis_index))
        .add("/verify-all", post(verify_all))
        .add("/{id}/acknowledge", post(acknowledge))
        .add("/{id}/verify", post(verify))
        .add("/{id}/silence", post(silence_alert))
        .add("/{id}/correlation", get(correlation_index))
}
