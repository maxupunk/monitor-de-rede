import type { DiscoveredHost } from './icmp_scanner.js'
import { SnmpService } from '#modules/snmp/snmp_service'
import { DeviceIdentifier } from '#modules/discovery/device_identifier'

const SNMP_BATCH_SIZE = 10

/**
 * Tenta descobrir informações via SNMP em hosts que têm a porta 161 aberta.
 * Usa detecção automática de versão/comunidade (public/private em v1/v2c).
 */
export class SnmpDiscoveryScanner {
  private snmpService = new SnmpService()
  private identifier = new DeviceIdentifier()

  async scanHosts(hosts: DiscoveredHost[], signal?: AbortSignal): Promise<DiscoveredHost[]> {
    if (signal?.aborted) {
      const error = new Error('Varredura cancelada.')
      error.name = 'AbortError'
      throw error
    }

    const snmpCandidates = hosts.filter(
      (h) => h.openPorts?.includes(161) || h.openPorts?.includes(162)
    )

    if (snmpCandidates.length === 0) return []

    const discovered: DiscoveredHost[] = []

    for (let i = 0; i < snmpCandidates.length; i += SNMP_BATCH_SIZE) {
      if (signal?.aborted) {
        const error = new Error('Varredura cancelada.')
        error.name = 'AbortError'
        throw error
      }
      const batch = snmpCandidates.slice(i, i + SNMP_BATCH_SIZE)
      const results = await Promise.all(batch.map((h) => this.probeHost(h)))
      for (const res of results) {
        if (res) discovered.push(res)
      }
    }

    return discovered
  }

  private async probeHost(host: DiscoveredHost): Promise<DiscoveredHost | null> {
    try {
      const result = await this.snmpService.detectConnection(host.ipAddress, 161, {})

      if (!result.responded) return null

      const vendor = this.extractVendor(result.sysDescr)
      const enriched: DiscoveredHost = {
        ipAddress: host.ipAddress,
        hostname: result.sysName,
        vendor: vendor ?? undefined,
        confidence: 95,
        data: {
          source: 'snmp',
          sysDescr: result.sysDescr,
          sysName: result.sysName,
          snmpVersion: result.version,
          snmpCommunity: result.community,
        },
      }

      enriched.deviceType = this.identifier.identifyType({
        ...enriched,
        openPorts: host.openPorts,
      })

      return enriched
    } catch {
      return null
    }
  }

  private extractVendor(sysDescr?: string): string | null {
    if (!sysDescr) return null
    const lower = sysDescr.toLowerCase()

    const vendors = [
      { key: 'mikrotik', name: 'MikroTik' },
      { key: 'ubiquiti', name: 'Ubiquiti Networks' },
      { key: 'cisco', name: 'Cisco Systems' },
      { key: 'juniper', name: 'Juniper Networks' },
      { key: 'hp', name: 'Hewlett-Packard' },
      { key: 'hewlett', name: 'Hewlett-Packard' },
      { key: 'dell', name: 'Dell' },
      { key: 'huawei', name: 'Huawei' },
      { key: 'tp-link', name: 'TP-Link' },
      { key: 'ruckus', name: 'Ruckus Wireless' },
      { key: 'aruba', name: 'Aruba Networks' },
      { key: 'fortinet', name: 'Fortinet' },
      { key: 'sonicwall', name: 'SonicWall' },
      { key: 'synology', name: 'Synology' },
      { key: 'qnap', name: 'QNAP' },
      { key: 'netgear', name: 'NETGEAR' },
      { key: 'zte', name: 'ZTE' },
      { key: 'd-link', name: 'D-Link' },
      { key: 'lenovo', name: 'Lenovo' },
      { key: 'supermicro', name: 'Supermicro' },
      { key: 'vmware', name: 'VMware' },
      { key: 'esxi', name: 'VMware' },
    ]

    for (const vendor of vendors) {
      if (lower.includes(vendor.key)) return vendor.name
    }

    return null
  }
}
