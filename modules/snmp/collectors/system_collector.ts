import type { SnmpClient } from '../clients/snmp_client.js'

export interface SnmpSystemInfo {
  sysName?: string
  sysDescr?: string
  sysObjectID?: string
  sysUpTime?: number
  sysContact?: string
  sysLocation?: string
}

export class SystemCollector {
  async collect(client: SnmpClient): Promise<SnmpSystemInfo> {
    return {}
  }
}
