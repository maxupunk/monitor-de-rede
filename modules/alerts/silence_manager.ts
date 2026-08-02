import { DateTime } from 'luxon'
import AlertEvent from '#models/alert_event'

export class SilenceManager {
  isSilenced(event: AlertEvent): boolean {
    if (event.status !== 'silenced') return false
    const silencedUntilStr = event.data?.silencedUntil as string | undefined
    if (!silencedUntilStr) return false

    const silencedUntil = DateTime.fromISO(silencedUntilStr)
    return silencedUntil.isValid && silencedUntil > DateTime.now()
  }

  async silenceAlert(event: AlertEvent, durationMinutes = 60): Promise<void> {
    const silencedUntil = DateTime.now().plus({ minutes: durationMinutes })
    event.status = 'silenced'
    event.data = {
      ...(event.data || {}),
      silencedUntil: silencedUntil.toISO()!,
    }
    await event.save()
  }

  async acknowledgeAlert(event: AlertEvent): Promise<void> {
    event.status = 'acknowledged'
    event.data = {
      ...(event.data || {}),
      acknowledgedAt: DateTime.now().toISO()!,
    }
    await event.save()
  }
}
