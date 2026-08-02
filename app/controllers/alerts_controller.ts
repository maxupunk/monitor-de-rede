import type { HttpContext } from '@adonisjs/core/http'
import AlertRule from '#models/alert_rule'
import AlertEvent from '#models/alert_event'
import { SilenceManager } from '#modules/alerts/silence_manager'

export default class AlertsController {
  private silenceManager = new SilenceManager()

  async rulesIndex({ response }: HttpContext) {
    const rules = await AlertRule.all()
    return response.ok(rules)
  }

  async rulesStore({ request, response }: HttpContext) {
    const data = request.only([
      'siteId',
      'deviceId',
      'monitorId',
      'name',
      'type',
      'condition',
      'severity',
      'durationSeconds',
      'enabled',
    ])
    if (data.enabled === undefined) data.enabled = true
    if (data.severity === undefined) data.severity = 'warning'
    if (data.type === undefined) data.type = 'custom'

    const rule = await AlertRule.create(data)
    return response.created(rule)
  }

  async rulesUpdate({ params, request, response }: HttpContext) {
    const rule = await AlertRule.findOrFail(params.id)
    const data = request.only([
      'siteId',
      'deviceId',
      'monitorId',
      'name',
      'type',
      'condition',
      'severity',
      'durationSeconds',
      'enabled',
    ])
    rule.merge(data)
    await rule.save()
    return response.ok(rule)
  }

  async rulesDestroy({ params, response }: HttpContext) {
    const rule = await AlertRule.findOrFail(params.id)
    await rule.delete()
    return response.noContent()
  }

  async index({ response }: HttpContext) {
    const events = await AlertEvent.query().orderBy('id', 'desc').limit(100)
    return response.ok(events)
  }

  async acknowledge({ params, response }: HttpContext) {
    const event = await AlertEvent.findOrFail(params.id)
    await this.silenceManager.acknowledgeAlert(event)
    return response.ok({ message: `Alerta #${event.id} reconhecido`, event })
  }

  async silence({ params, request, response }: HttpContext) {
    const event = await AlertEvent.findOrFail(params.id)
    const { minutes } = request.only(['minutes'])
    const duration = Number(minutes) || 60
    await this.silenceManager.silenceAlert(event, duration)
    return response.ok({ message: `Alerta #${event.id} silenciado por ${duration} minutos`, event })
  }
}
