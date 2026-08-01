import type { SnmpClient } from '../clients/snmp_client.js'

export interface SnmpInterface {
  ifIndex: number
  ifName?: string
  ifDescr?: string
  ifAlias?: string
  ifType?: number
  ifSpeed?: number
  ifAdminStatus?: number
  ifOperStatus?: number
  macAddress?: string
}

export class InterfaceCollector {
  async collect(client: SnmpClient): Promise<SnmpInterface[]> {
    return []
  }
}
