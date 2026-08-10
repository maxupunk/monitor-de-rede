import type Device from '#models/device'
import type DeviceInterface from '#models/device_interface'
import { AlertManager } from '#modules/alerts/alert_manager'
import { AlertScopeKey } from '#modules/alerts/contracts/alert_evaluation'
import { ALERT_FIELDS } from '#modules/alerts/alert_fields'
import {
  buildInterfaceStateDataset,
  describeInterfaceState,
  hasInterfaceTransition,
  isInterfaceRecovery,
} from '#modules/alerts/datasets/interface_state_dataset'
import { EventBus } from '#modules/events/event_bus'
import { formatSpeed, normalizeSpeed } from './link_speed.js'

// Reexportados por compatibilidade: `link_speed.ts` é a origem das funções.
export { formatSpeed, normalizeSpeed }

/**
 * Observa o estado das interfaces coletadas via SNMP.
 *
 * O serviço não decide mais o que é alerta: ele publica os fatos no feed em
 * tempo real e entrega o mesmo conjunto ao motor de alertas. Políticas como
 * "downgrade de negociação é um aviso" agora vivem em "Regras Configuradas"
 * (catálogo `interface_speed_downgrade`), podendo ser ajustadas ou desligadas
 * pelo operador sem alterar código.
 */
export class InterfaceMonitoringService {
  private alertManager = new AlertManager()
  private eventBus = EventBus.getInstance()

  async evaluateInterfaceState(
    device: Device,
    iface: DeviceInterface,
    previousOperStatus: string | null,
    previousSpeed: number | null
  ): Promise<void> {
    const dataset = buildInterfaceStateDataset(iface, previousOperStatus, previousSpeed)
    const message = describeInterfaceState(dataset)

    if (hasInterfaceTransition(dataset)) {
      this.publishTransitions(device, iface, dataset, message)
    }

    await this.alertManager.evaluate({
      scope: { siteId: device.siteId ?? null, deviceId: device.id, monitorId: null },
      scopeKey: AlertScopeKey.interface(iface.id),
      targetLabel: `${device.name} / ${iface.name}`,
      dataset,
      message,
      data: {
        eventType: 'interface_state',
        interfaceId: iface.id,
        ifIndex: iface.snmpIndex,
        ...dataset,
      },
      recovered: isInterfaceRecovery(dataset),
    })
  }

  /** Feed em tempo real: os fatos observados, independentemente de alertar. */
  private publishTransitions(
    device: Device,
    iface: DeviceInterface,
    dataset: Record<string, unknown>,
    message: string
  ): void {
    const base = {
      deviceId: device.id,
      deviceName: device.name,
      interfaceId: iface.id,
      ifName: iface.name,
      ifIndex: iface.snmpIndex,
      message,
    }

    const statusTransition = dataset[ALERT_FIELDS.interfaceStatusTransition]
    if (statusTransition) {
      this.eventBus.emit('interface:status_change', {
        ...base,
        previousStatus: dataset.interfacePreviousOperStatus ?? null,
        currentStatus: dataset[ALERT_FIELDS.interfaceOperStatus] ?? null,
        transition: statusTransition,
      })
    }

    const speedTransition = dataset[ALERT_FIELDS.interfaceSpeedTransition]
    if (speedTransition) {
      const previousSpeedBps = (dataset.interfacePreviousSpeedBps as number) ?? null
      const currentSpeedBps = (dataset[ALERT_FIELDS.interfaceSpeedBps] as number) ?? null

      this.eventBus.emit(
        speedTransition === 'downgrade' ? 'interface:speed_downgrade' : 'interface:speed_change',
        {
          ...base,
          previousSpeed: previousSpeedBps,
          currentSpeed: currentSpeedBps,
          previousSpeedFormatted: formatSpeed(previousSpeedBps),
          currentSpeedFormatted: formatSpeed(currentSpeedBps),
          transition: speedTransition,
        }
      )
    }
  }
}
