import { DateTime } from 'luxon'
import type Device from '#models/device'
import Monitor from '#models/monitor'
import { EventBus } from '#modules/events/event_bus'

export type DeviceStatus = 'online' | 'offline' | 'warning' | 'unknown'
export type MonitorStatus = 'up' | 'down' | 'warning' | 'unknown' | 'disabled'

/**
 * Estados que não opinam sobre a disponibilidade do dispositivo: `unknown` é um
 * monitor que ainda não rodou e `disabled` é um alvo desligado
 * administrativamente (ex.: interface em admin down). Nenhum dos dois deve
 * derrubar — nem sustentar — o status do dispositivo.
 */
const NON_CONCLUSIVE: ReadonlySet<MonitorStatus> = new Set(['unknown', 'disabled'])

export interface DeviceStatusTransition {
  /** `true` somente quando o status anterior difere do novo */
  changed: boolean
  previousStatus: DeviceStatus
  status: DeviceStatus
}

/**
 * Ponto único de escrita do status de dispositivo.
 *
 * Antes cada produtor (resultado de monitor, coleta SNMP, provisionamento VPN)
 * escrevia `device.status` por conta própria. Como a coleta SNMP marcava
 * "online" em silêncio e o ping seguinte marcava "offline", o dispositivo
 * alternava de estado a cada ciclo: o banco registrava uma transição real, o
 * feed publicava `device:status` e a tela — que nunca chegou a ver o "online" —
 * exibia "offline ➔ offline". Concentrar a decisão aqui elimina a alternância e
 * garante que só transição de verdade vire evento.
 */
export class DeviceStatusService {
  private eventBus = EventBus.getInstance()

  /**
   * Consolida os monitores habilitados de um dispositivo em um único status.
   * Um dispositivo que responde ping mas não responde SNMP fica em `warning`,
   * em vez de disputar entre online e offline a cada verificação.
   */
  static aggregate(statuses: MonitorStatus[], fallback: DeviceStatus = 'unknown'): DeviceStatus {
    const conclusive = statuses.filter((status) => !NON_CONCLUSIVE.has(status))
    if (conclusive.length === 0) return fallback

    const hasUp = conclusive.includes('up')
    const hasDown = conclusive.includes('down')

    if (hasUp && hasDown) return 'warning'
    if (hasDown) return 'offline'
    if (conclusive.includes('warning')) return 'warning'
    return 'online'
  }

  /**
   * Recalcula o status a partir dos monitores habilitados. `observedStatus` é
   * usado apenas quando o dispositivo não tem monitor algum — caso da coleta
   * SNMP avulsa, em que a própria coleta é a evidência de disponibilidade.
   */
  async refreshFromMonitors(
    device: Device,
    options: { observedStatus?: DeviceStatus; seenAt?: DateTime | null } = {}
  ): Promise<DeviceStatusTransition> {
    const monitors = await Monitor.query()
      .where('deviceId', device.id)
      .where('enabled', true)
      .select('status')

    const next =
      monitors.length > 0
        ? DeviceStatusService.aggregate(
            monitors.map((monitor) => monitor.status as MonitorStatus),
            device.status
          )
        : (options.observedStatus ?? device.status)

    return this.apply(device, next, options.seenAt ?? null)
  }

  /**
   * Persiste o status decidido e publica `device:status` apenas na transição.
   *
   * `lastSeenAt` é telemetria (o dispositivo continua sendo visto a cada ciclo)
   * e por isso avança sem gerar evento; a gravação só acontece se algum campo
   * de fato mudou.
   */
  async apply(
    device: Device,
    status: DeviceStatus,
    seenAt: DateTime | null = null
  ): Promise<DeviceStatusTransition> {
    const previousStatus = device.status as DeviceStatus
    const changed = previousStatus !== status

    device.status = status
    if (seenAt) device.lastSeenAt = seenAt

    if (device.$isDirty) {
      await device.save()
    }

    if (changed) {
      this.eventBus.emit('device:status', {
        id: device.id,
        deviceId: device.id,
        name: device.name,
        status: device.status,
        previousStatus,
        ipAddress: device.ipAddress ?? null,
        lastSeenAt: device.lastSeenAt?.toISO() ?? null,
        changedAt: DateTime.now().toISO()!,
      })
    }

    return { changed, previousStatus, status }
  }
}
