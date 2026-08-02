import { DateTime } from 'luxon'
import AlertEvent from '#models/alert_event'
import { NotificationService } from '#modules/notifications/notification_service'
import { EventBus } from '#modules/events/event_bus'

export class RecoveryManager {
  private notificationService = new NotificationService()
  private eventBus = EventBus.getInstance()

  async resolveAlertsForMonitor(monitorId: number, message = 'Monitoramento normalizado'): Promise<void> {
    const activeEvents = await AlertEvent.query()
      .where('monitorId', monitorId)
      .whereIn('status', ['active', 'acknowledged', 'silenced'])

    for (const event of activeEvents) {
      event.status = 'resolved'
      event.resolvedAt = DateTime.now()
      await event.save()

      await this.notificationService.notify({
        title: `✅ [RESOLVIDO] Alerta #${event.id}`,
        body: `${event.message || 'Alerta'} foi normalizado. ${message}`,
        severity: 'info',
        metadata: { alertEventId: event.id, monitorId },
      })

      this.eventBus.emit('alert:resolved', {
        alertEventId: event.id,
        monitorId,
        resolvedAt: event.resolvedAt.toISO()!,
      })
    }
  }
}
