import { DateTime } from 'luxon'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import AlertEvent from '#models/alert_event'
import { NotificationService } from '#modules/notifications/notification_service'
import { EventBus } from '#modules/events/event_bus'

/**
 * `ifSpeed` é um contador de 32 bits: agentes que não expõem `ifHighSpeed`
 * devolvem o teto (4.294.967.295) para links acima de ~4,29 Gbps. Tratar esse
 * valor como velocidade real produzia falso downgrade/upgrade sempre que a
 * leitura alternava entre o teto e o valor verdadeiro.
 */
const IF_SPEED_SATURATED = 4_294_967_295

/** Converte a leitura crua em bps utilizável, ou `null` quando não é conclusiva. */
export function normalizeSpeed(bps: number | string | null | undefined): number | null {
  if (bps === null || bps === undefined) return null

  const value = Number(bps)
  if (!Number.isFinite(value) || value <= 0) return null
  if (value >= IF_SPEED_SATURATED) return null

  return value
}

export function formatSpeed(bps: number | null | undefined): string {
  if (bps === null || bps === undefined || isNaN(bps) || bps <= 0) {
    return 'Desconhecido'
  }
  if (bps >= 1_000_000_000) {
    const gbps = bps / 1_000_000_000
    return `${Number.isInteger(gbps) ? gbps : gbps.toFixed(1)} Gbps`
  }
  if (bps >= 1_000_000) {
    const mbps = bps / 1_000_000
    return `${Number.isInteger(mbps) ? mbps : mbps.toFixed(1)} Mbps`
  }
  if (bps >= 1_000) {
    const kbps = bps / 1_000
    return `${Number.isInteger(kbps) ? kbps : kbps.toFixed(1)} Kbps`
  }
  return `${bps} bps`
}

export class InterfaceMonitoringService {
  private notificationService = new NotificationService()
  private eventBus = EventBus.getInstance()

  async evaluateInterfaceState(
    device: Device,
    iface: DeviceInterface,
    previousOperStatus: string | null,
    previousSpeed: number | null
  ): Promise<void> {
    const currentOperStatus = iface.operStatus
    const currentSpeed = iface.speed

    // 1. Verificação de alteração de status operacional (UP <-> DOWN)
    if (previousOperStatus !== null && currentOperStatus && currentOperStatus !== previousOperStatus) {
      const isDown = currentOperStatus === 'down'
      const message = `Interface ${iface.name} alterou status: ${previousOperStatus.toUpperCase()} ➔ ${currentOperStatus.toUpperCase()}`

      const alertEvent = await AlertEvent.create({
        deviceId: device.id,
        alertRuleId: null,
        monitorId: null,
        status: isDown ? 'active' : 'resolved',
        severity: isDown ? 'warning' : 'info',
        startedAt: DateTime.now(),
        resolvedAt: isDown ? null : DateTime.now(),
        message,
        data: {
          eventType: 'interface_status_change',
          interfaceId: iface.id,
          ifIndex: iface.snmpIndex,
          ifName: iface.name,
          previousStatus: previousOperStatus,
          currentStatus: currentOperStatus,
        },
      })

      this.eventBus.emit('interface:status_change', {
        alertEventId: alertEvent.id,
        deviceId: device.id,
        interfaceId: iface.id,
        ifName: iface.name,
        previousStatus: previousOperStatus,
        currentStatus: currentOperStatus,
        message,
      })

      if (isDown) {
        await this.notificationService.notify({
          title: `⚠️ Queda de Interface em ${device.name}`,
          body: `Interface ${iface.name} alterou status de ${previousOperStatus.toUpperCase()} para ${currentOperStatus.toUpperCase()}`,
          severity: 'warning',
          metadata: { alertEventId: alertEvent.id, deviceId: device.id, interfaceId: iface.id },
        })
      }
    }

    // 2. Verificação de alteração na negociação de velocidade (Link Speed Negotiation)
    const prevNumSpeed = normalizeSpeed(previousSpeed)
    const currNumSpeed = normalizeSpeed(currentSpeed)

    if (prevNumSpeed !== null && currNumSpeed !== null && prevNumSpeed !== currNumSpeed) {
      const prevSpeedFormatted = formatSpeed(prevNumSpeed)
      const currentSpeedFormatted = formatSpeed(currNumSpeed)

      // Somente gera evento se houver mudança real na velocidade formatada (exclui 1 Gbps -> 1 Gbps)
      if (prevSpeedFormatted !== currentSpeedFormatted) {
        const isDowngrade = currNumSpeed < prevNumSpeed

        if (isDowngrade) {
          // ALERTA DE DOWNGRADE (ex: 1 Gbps -> 100 Mbps ou 2.5 Gbps -> 1 Gbps)
          const message = `🚨 ALERTA DE DOWNGRADE: Interface ${iface.name} sofreu downgrade de velocidade de ${prevSpeedFormatted} para ${currentSpeedFormatted}`

          const alertEvent = await AlertEvent.create({
            deviceId: device.id,
            alertRuleId: null,
            monitorId: null,
            status: 'active',
            severity: 'warning',
            startedAt: DateTime.now(),
            message,
            data: {
              eventType: 'interface_speed_downgrade',
              interfaceId: iface.id,
              ifIndex: iface.snmpIndex,
              ifName: iface.name,
              previousSpeed: prevNumSpeed,
              currentSpeed: currNumSpeed,
              previousSpeedFormatted: prevSpeedFormatted,
              currentSpeedFormatted: currentSpeedFormatted,
            },
          })

          this.eventBus.emit('interface:speed_downgrade', {
            alertEventId: alertEvent.id,
            deviceId: device.id,
            interfaceId: iface.id,
            ifName: iface.name,
            previousSpeed: prevNumSpeed,
            currentSpeed: currNumSpeed,
            previousSpeedFormatted: prevSpeedFormatted,
            currentSpeedFormatted: currentSpeedFormatted,
            message,
          })

          await this.notificationService.notify({
            title: `🚨 Downgrade de Interface em ${device.name}`,
            body: `Interface ${iface.name} sofreu downgrade na velocidade de negociação de ${prevSpeedFormatted} para ${currentSpeedFormatted}`,
            severity: 'warning',
            metadata: { alertEventId: alertEvent.id, deviceId: device.id, interfaceId: iface.id },
          })
        } else {
          // EVENTO DE UPGRADE OU ALTERAÇÃO DE VELOCIDADE PARA CIMA (ex: 100 Mbps -> 1 Gbps)
          const message = `Interface ${iface.name} alterou negociação de velocidade: ${prevSpeedFormatted} ➔ ${currentSpeedFormatted}`

          const alertEvent = await AlertEvent.create({
            deviceId: device.id,
            alertRuleId: null,
            monitorId: null,
            status: 'resolved',
            severity: 'info',
            startedAt: DateTime.now(),
            resolvedAt: DateTime.now(),
            message,
            data: {
              eventType: 'interface_speed_upgrade',
              interfaceId: iface.id,
              ifIndex: iface.snmpIndex,
              ifName: iface.name,
              previousSpeed: prevNumSpeed,
              currentSpeed: currNumSpeed,
              previousSpeedFormatted: prevSpeedFormatted,
              currentSpeedFormatted: currentSpeedFormatted,
            },
          })

          this.eventBus.emit('interface:speed_change', {
            alertEventId: alertEvent.id,
            deviceId: device.id,
            interfaceId: iface.id,
            ifName: iface.name,
            previousSpeed: prevNumSpeed,
            currentSpeed: currNumSpeed,
            previousSpeedFormatted: prevSpeedFormatted,
            currentSpeedFormatted: currentSpeedFormatted,
            message,
          })
        }
      }
    }
  }
}
