import type { HttpContext } from '@adonisjs/core/http'
import AlertRule from '#models/alert_rule'
import AlertEvent from '#models/alert_event'
import Monitor from '#models/monitor'
import { SilenceManager } from '#modules/alerts/silence_manager'
import { AlertRuleCatalogService } from '#modules/alerts/catalog/alert_rule_catalog_service'
import { ALERT_RULE_CATEGORY_LABELS } from '#modules/alerts/catalog/alert_rule_templates'
import { EventBus } from '#modules/events/event_bus'
import { MonitorRunner } from '#modules/monitoring/monitor_runner'
import { ResultProcessor } from '#modules/monitoring/result_processor'

/** Referência enxuta a uma entidade relacionada no payload de alertas */
interface RelatedSummary {
  id: number
  name: string
}

/** Contrato de resposta de `GET /api/alerts` */
export interface SerializedAlertEvent {
  id: number
  alertRuleId: number | null
  deviceId: number | null
  monitorId: number | null
  scopeKey: string | null
  status: AlertEvent['status']
  severity: AlertEvent['severity']
  message: string | null
  data: Record<string, unknown> | null
  startedAt: string | null
  resolvedAt: string | null
  createdAt: string | null
  updatedAt: string | null
  /** Derivado: nome da regra + alvo, com fallback para o título gravado no evento */
  title: string
  alertRule: RelatedSummary | null
  device: RelatedSummary | null
  monitor: RelatedSummary | null
  silencedUntil: string | null
}

const RULE_FIELDS = [
  'siteId',
  'deviceId',
  'monitorId',
  'name',
  'type',
  'condition',
  'severity',
  'durationSeconds',
  'enabled',
] as const

export default class AlertsController {
  private silenceManager = new SilenceManager()
  private catalog = new AlertRuleCatalogService()
  private eventBus = EventBus.getInstance()
  private monitorRunner = new MonitorRunner()
  private resultProcessor = new ResultProcessor()

  /**
   * O front envia a regra em linguagem simples (métrica/operador/valor); aqui
   * garantimos o formato `{ field, operator, value }` esperado pelo avaliador.
   */
  private normalizeCondition(condition: unknown): Record<string, unknown> | null {
    if (!condition || typeof condition !== 'object' || Array.isArray(condition)) return null
    const { field, operator, value } = condition as Record<string, unknown>
    if (typeof field !== 'string' || typeof operator !== 'string') return null
    return { field, operator, value }
  }

  /** Payload enxuto usado nos eventos SSE de regras */
  private ruleEventPayload(rule: AlertRule) {
    return {
      id: rule.id,
      name: rule.name,
      type: rule.type,
      templateKey: rule.templateKey ?? null,
      condition: rule.condition,
      severity: rule.severity,
      durationSeconds: rule.durationSeconds,
      enabled: rule.enabled,
      isEnabled: rule.enabled,
    }
  }

  async rulesIndex({ response }: HttpContext) {
    const rules = await AlertRule.query().orderBy('id', 'asc')
    return response.ok(rules)
  }

  /** Catálogo de regras pré-configuradas, já marcando o que existe no banco. */
  async catalogIndex({ response }: HttpContext) {
    return response.ok({
      categories: ALERT_RULE_CATEGORY_LABELS,
      templates: await this.catalog.describe(),
    })
  }

  /**
   * Aplica as regras escolhidas no catálogo. Idempotente: o que já existe é
   * reportado em `skipped` em vez de ser duplicado.
   */
  async catalogApply({ request, response }: HttpContext) {
    const keys = request.input('keys')

    if (!Array.isArray(keys) || keys.length === 0) {
      return response.unprocessableEntity({
        error: 'Selecione ao menos uma regra pré-configurada para aplicar.',
      })
    }

    const result = await this.catalog.apply(keys.map((key: unknown) => String(key)))

    for (const rule of result.created) {
      this.eventBus.emit('alert_rule:created', this.ruleEventPayload(rule))
    }

    return response.created({ created: result.created, skipped: result.skipped })
  }

  async rulesStore({ request, response }: HttpContext) {
    const data = request.only([...RULE_FIELDS])

    const condition = this.normalizeCondition(data.condition)
    if (!condition) {
      return response.unprocessableEntity({
        error:
          'Condição inválida. Informe a métrica alvo, a comparação e o valor de referência da regra.',
      })
    }
    data.condition = condition

    if (data.enabled === undefined) data.enabled = true
    if (data.severity === undefined) data.severity = 'warning'
    if (data.type === undefined) data.type = 'custom'
    if (data.durationSeconds === undefined) data.durationSeconds = 0

    const rule = await AlertRule.create(data)
    this.eventBus.emit('alert_rule:created', this.ruleEventPayload(rule))
    return response.created(rule)
  }

  async rulesUpdate({ params, request, response }: HttpContext) {
    const rule = await AlertRule.findOrFail(params.id)
    const data = request.only([...RULE_FIELDS])

    if (data.condition !== undefined) {
      const condition = this.normalizeCondition(data.condition)
      if (!condition) {
        return response.unprocessableEntity({
          error:
            'Condição inválida. Informe a métrica alvo, a comparação e o valor de referência da regra.',
        })
      }
      data.condition = condition
    }

    rule.merge(data)
    await rule.save()
    this.eventBus.emit('alert_rule:updated', this.ruleEventPayload(rule))
    return response.ok(rule)
  }

  async rulesDestroy({ params, response }: HttpContext) {
    const rule = await AlertRule.findOrFail(params.id)
    const payload = this.ruleEventPayload(rule)
    await rule.delete()
    this.eventBus.emit('alert_rule:deleted', payload)
    return response.noContent()
  }

  async index({ request, response }: HttpContext) {
    const query = AlertEvent.query()
      .preload('alertRule')
      .preload('device')
      .preload('monitor')
      .orderBy('id', 'desc')

    // Paginação sob demanda: a aba "Ativos" continua carregando a lista curta de
    // uma vez (ela filtra e ordena no cliente), enquanto o histórico completo —
    // que cresce sem teto — pede página a página.
    const pageParam = request.input('page')
    if (pageParam) {
      const page = Number(pageParam) || 1
      const limit = Math.min(Number(request.input('limit', 20)), 100)
      const paginated = await query.paginate(page, limit)

      return response.ok({
        data: paginated.all().map((event) => this.serializeEvent(event)),
        meta: paginated.toJSON().meta,
      })
    }

    const events = await query.limit(100)
    return response.ok(events.map((event) => this.serializeEvent(event)))
  }

  /**
   * Achata as relações e deriva o título exibido na Central de Alertas.
   *
   * A forma é declarada explicitamente (`SerializedAlertEvent`) em vez de
   * espalhar `event.serialize()`: assim o contrato do endpoint é verificável
   * pelo TypeScript, tanto aqui quanto em quem consome o cliente tipado.
   */
  private serializeEvent(event: AlertEvent): SerializedAlertEvent {
    const storedTitle = event.data?.title as string | undefined
    const ruleName = event.alertRule?.name ?? (event.data?.ruleName as string | undefined)
    const target = event.device?.name ?? event.monitor?.name ?? null

    return {
      id: event.id,
      alertRuleId: event.alertRuleId,
      deviceId: event.deviceId,
      monitorId: event.monitorId,
      scopeKey: event.scopeKey ?? null,
      status: event.status,
      severity: event.severity,
      message: event.message,
      data: event.data ?? null,
      startedAt: event.startedAt?.toISO() ?? null,
      resolvedAt: event.resolvedAt?.toISO() ?? null,
      createdAt: event.createdAt?.toISO() ?? null,
      updatedAt: event.updatedAt?.toISO() ?? null,
      title: storedTitle || [ruleName, target].filter(Boolean).join(' — ') || 'Alerta do sistema',
      alertRule: event.alertRule ? { id: event.alertRule.id, name: event.alertRule.name } : null,
      device: event.device ? { id: event.device.id, name: event.device.name } : null,
      monitor: event.monitor ? { id: event.monitor.id, name: event.monitor.name } : null,
      silencedUntil: (event.data?.silencedUntil as string | undefined) ?? null,
    }
  }

  /**
   * Executa a checagem em tempo real do alvo do alerta (se for um monitor)
   * e se recuperar, finaliza o alerta automaticamente.
   */
  private async checkAndResolveAlert(event: AlertEvent): Promise<boolean> {
    if ((event.status as string) === 'resolved') return true

    if (event.monitorId) {
      const monitor = await Monitor.find(event.monitorId)
      if (monitor && monitor.enabled) {
        try {
          const result = await this.monitorRunner.runMonitor(monitor.type, monitor.configuration, {
            timeoutMs: (monitor.timeoutSeconds || 5) * 1000,
          })
          await this.resultProcessor.processResult(monitor.id, result, monitor.probeId)
          await event.refresh()
          if ((event.status as string) === 'resolved') {
            return true
          }
        } catch {
          // Se houver erro de execução na checagem, mantém avaliação pelo estado atual
        }
      }
    }

    return (event.status as string) === 'resolved'
  }

  async acknowledge({ params, response }: HttpContext) {
    const event = await AlertEvent.findOrFail(params.id)
    await event.load('alertRule')
    await event.load('device')
    await event.load('monitor')

    const isResolved = await this.checkAndResolveAlert(event)

    if (isResolved) {
      return response.ok({
        message: `Alerta #${event.id} foi verificado e resolvido automaticamente`,
        event: this.serializeEvent(event),
        resolved: true,
      })
    }

    await this.silenceManager.acknowledgeAlert(event)

    this.eventBus.emit('alert:acknowledged', {
      id: event.id,
      alertEventId: event.id,
      monitorId: event.monitorId,
      deviceId: event.deviceId,
      status: event.status,
      severity: event.severity,
      message: event.message,
    })

    return response.ok({
      message: `Alerta #${event.id} reconhecido`,
      event: this.serializeEvent(event),
      resolved: false,
    })
  }

  async verify({ params, response }: HttpContext) {
    const event = await AlertEvent.findOrFail(params.id)
    await event.load('alertRule')
    await event.load('device')
    await event.load('monitor')

    const isResolved = await this.checkAndResolveAlert(event)

    return response.ok({
      message: isResolved
        ? `Alerta #${event.id} resolvido com sucesso!`
        : `Alerta #${event.id} continua ativo.`,
      event: this.serializeEvent(event),
      resolved: isResolved,
    })
  }

  async verifyAll({ response }: HttpContext) {
    const activeEvents = await AlertEvent.query()
      .whereIn('status', ['active', 'acknowledged', 'silenced'])
      .preload('alertRule')
      .preload('device')
      .preload('monitor')

    let resolvedCount = 0

    for (const event of activeEvents) {
      const isResolved = await this.checkAndResolveAlert(event)
      if (isResolved) {
        resolvedCount++
      }
    }

    return response.ok({
      message: `${resolvedCount} de ${activeEvents.length} alerta(s) pendente(s) resolvido(s)`,
      totalChecked: activeEvents.length,
      resolvedCount,
    })
  }

  async silence({ params, request, response }: HttpContext) {
    const event = await AlertEvent.findOrFail(params.id)
    const { minutes, durationMinutes } = request.only(['minutes', 'durationMinutes'])
    const duration = Number(minutes ?? durationMinutes) || 60
    await this.silenceManager.silenceAlert(event, duration)

    this.eventBus.emit('alert:silenced', {
      id: event.id,
      alertEventId: event.id,
      monitorId: event.monitorId,
      deviceId: event.deviceId,
      status: event.status,
      severity: event.severity,
      message: event.message,
      silencedUntil: (event.data?.silencedUntil as string | undefined) ?? null,
      durationMinutes: duration,
    })

    return response.ok({ message: `Alerta #${event.id} silenciado por ${duration} minutos`, event })
  }
}
