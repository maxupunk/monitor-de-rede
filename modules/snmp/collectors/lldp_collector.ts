import type { SnmpClient } from '../clients/snmp_client.js'

export interface LldpNeighbor {
  localPort: string
  remotePort: string
  remoteSysName: string
  remoteMgmtAddress?: string
}

export class LldpCollector {
  async collect(_client: SnmpClient): Promise<LldpNeighbor[]> {
    return []
  }
}
