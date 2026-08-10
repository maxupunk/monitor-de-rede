import { DateTime } from 'luxon'
import AlertEvent from '#models/alert_event'
import { AlertScopeKey } from './contracts/alert_evaluation.js'
import { NotificationService } from '#modules/notifications/notification_service'
import { EventBus } from '#modules/events/event_bus'

/**
 * Normalização automática: fecha os alertas abertos de um alvo quando ele volta
 * ao normal. Trabalha por `scopeKey`, então serve tanto para monitores quanto
 * para alvos sem monitor (interfaces, por exemplo).
 */
export class RecoveryManager {
  private notificationService = new NotificationService()
  private eventBus = EventBus.getInstance()

  async resolveScope(scopeKey: string, message = 'Monitoramento normalizado'): Promise<void> {
    const activeEvents = await AlertEvent.query()
      .where((query) => {
        query.where('scopeKey', scopeKey)
        if (scopeKey.startsWith('monitor:')) {
          const monitorId = Number(scopeKey.split(':')[1])
          if (!isNaN(monitorId)) {
            query.orWhere('monitorId', monitorId)
          }
        }
      })
      .whereIn('status', ['active', 'acknowledged', 'silenced'])

    for (const event of activeEvents) {
      event.status = 'resolved'
      event.resolvedAt = DateTime.now()
      await event.save()

      await this.notificationService.notify({
        title: `✅ [RESOLVIDO] Alerta #${event.id}`,
        body: `${event.message || 'Alerta'} foi normalizado. ${message}`,
        severity: 'info',
        metadata: { alertEventId: event.id, scopeKey, monitorId: event.monitorId },
      })

      this.eventBus.emit('alert:resolved', {
        id: event.id,
        alertEventId: event.id,
        scopeKey,
        monitorId: event.monitorId,
        deviceId: event.deviceId,
        severity: event.severity,
        status: event.status,
        title: (event.data?.title as string) || `Alerta #${event.id}`,
        message: event.message,
        resolvedAt: event.resolvedAt.toISO()!,
      })
    }
  }

  /** Atalho para o alvo mais comum: os alertas abertos de um monitor. */
  async resolveAlertsForMonitor(monitorId: number, message?: string): Promise<void> {
    await this.resolveScope(AlertScopeKey.monitor(monitorId), message)
  }
}
