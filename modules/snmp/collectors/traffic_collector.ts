import type { SnmpClient } from '../clients/snmp_client.js'

export interface InterfaceTraffic {
  ifIndex: number
  inOctets: number
  outOctets: number
  inErrors: number
  outErrors: number
  recordedAt: Date
}

export class TrafficCollector {
  async collect(_client: SnmpClient): Promise<InterfaceTraffic[]> {
    return []
  }
}
