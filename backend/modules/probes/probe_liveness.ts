import { DateTime } from 'luxon'
import Probe from '#models/probe'
import { EventBus } from '#modules/events/event_bus'

/**
 * Vida dos probes.
 *
 * O heartbeat marca `online`, mas nada marcava o caminho de volta: um agente
 * derrubado continuava aparecendo como `online` para sempre, e as tarefas
 * despachadas para ele sumiam sem deixar rastro. Aqui a verdade vem de
 * `last_seen_at`, não do campo `status`.
 */

/**
 * Silêncio tolerado antes de dar o probe como fora do ar. O agente bate a cada
 * `PROBE_INTERVAL_MS` (5s por padrão), então a folga cobre com sobra tanto o
 * ritmo padrão quanto intervalos bem mais largos.
 */
export const PROBE_OFFLINE_AFTER_SECONDS = 90

/** Um probe só recebe tarefa se realmente estiver batendo o heartbeat. */
export function isProbeAlive(probe: Probe | null | undefined): boolean {
  if (!probe) return false
  if (probe.status === 'revoked') return false
  if (!probe.lastSeenAt) return false

  return DateTime.now().diff(probe.lastSeenAt, 'seconds').seconds <= PROBE_OFFLINE_AFTER_SECONDS
}

/**
 * Marca como `offline` os probes que pararam de bater o heartbeat e publica a
 * transição — é o que faz a tela de probes contar a verdade e explicar por que
 * os monitores daquele agente pararam.
 */
export class ProbeWatchdog {
  private eventBus = EventBus.getInstance()

  async markStaleProbesOffline(): Promise<number> {
    const candidates = await Probe.query().whereIn('status', ['online', 'busy'])
    let changed = 0

    for (const probe of candidates) {
      if (isProbeAlive(probe)) continue

      probe.status = 'offline'
      await probe.save()
      changed++

      this.eventBus.emit('probe:status', {
        id: probe.id,
        probeId: probe.id,
        name: probe.name,
        status: probe.status,
        version: probe.version ?? null,
        lastSeenAt: probe.lastSeenAt?.toISO() ?? null,
      })
    }

    return changed
  }
}
