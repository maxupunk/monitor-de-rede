import type { TransactionClientContract } from '@adonisjs/lucid/types/database'
import Monitor from '#models/monitor'
import Probe from '#models/probe'
import type Device from '#models/device'

/**
 * Provisionamento automático do monitoramento de um dispositivo da VPN (§4.7).
 *
 * Os monitores são atribuídos ao `vpn-probe`, o único agente que enxerga a
 * interface `wg0` — o probe da LAN continua intocado.
 */

/** Nome do probe dedicado que compartilha o namespace de rede do WireGuard. */
export const VPN_PROBE_NAME = process.env.VPN_PROBE_NAME || 'vpn-probe'

export interface MonitorProvisioningOptions {
  snmpEnabled?: boolean
  snmpCommunity?: string | null
  snmpVersion?: string | null
  intervalSeconds?: number
  trx?: TransactionClientContract
}

export class VpnMonitorProvisioner {
  /** Id do `vpn-probe`; `null` quando ainda não registrado (monitor roda local). */
  async resolveProbeId(trx?: TransactionClientContract): Promise<number | null> {
    const query = Probe.query().where('name', VPN_PROBE_NAME).whereNot('status', 'revoked')
    if (trx) query.useTransaction(trx)

    const probe = await query.first()
    return probe?.id ?? null
  }

  async provision(device: Device, options: MonitorProvisioningOptions = {}): Promise<Monitor[]> {
    const { trx } = options
    const probeId = await this.resolveProbeId(trx)
    const host = device.ipAddress || device.name
    const interval = options.intervalSeconds ?? 60
    const created: Monitor[] = []

    const pingMonitor = new Monitor()
    pingMonitor.deviceId = device.id
    pingMonitor.probeId = probeId
    pingMonitor.type = 'ping'
    pingMonitor.name = `Ping ${device.name}`
    pingMonitor.configuration = { host }
    pingMonitor.intervalSeconds = interval
    pingMonitor.timeoutSeconds = 5
    pingMonitor.retryCount = 3
    pingMonitor.enabled = true
    pingMonitor.status = 'unknown'
    if (trx) pingMonitor.useTransaction(trx)
    await pingMonitor.save()
    created.push(pingMonitor)

    if (options.snmpEnabled) {
      const snmpMonitor = new Monitor()
      snmpMonitor.deviceId = device.id
      snmpMonitor.probeId = probeId
      snmpMonitor.type = 'snmp'
      snmpMonitor.name = `SNMP ${device.name}`
      snmpMonitor.configuration = {
        host,
        version: options.snmpVersion || 'v2c',
        community: options.snmpCommunity || 'public',
        port: 161,
      }
      snmpMonitor.intervalSeconds = interval
      snmpMonitor.timeoutSeconds = 5
      snmpMonitor.retryCount = 3
      snmpMonitor.enabled = true
      snmpMonitor.status = 'unknown'
      if (trx) snmpMonitor.useTransaction(trx)
      await snmpMonitor.save()
      created.push(snmpMonitor)
    }

    return created
  }
}
