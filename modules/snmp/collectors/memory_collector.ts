import type { SnmpClient } from '../clients/snmp_client.js'
import { snmpNumber } from './snmp_value.js'

export interface SnmpMemoryInfo {
  totalKb?: number
  availKb?: number
  freeKb?: number
  usedKb?: number
  usedPercent?: number
}

export class MemoryCollector {
  public static readonly OID_MEM_TOTAL_REAL = '1.3.6.1.4.1.2021.4.5.0'
  public static readonly OID_MEM_AVAIL_REAL = '1.3.6.1.4.1.2021.4.6.0'
  public static readonly OID_MEM_TOTAL_FREE = '1.3.6.1.4.1.2021.4.11.0'

  async collect(client: SnmpClient): Promise<SnmpMemoryInfo> {
    const response = await client.get([
      MemoryCollector.OID_MEM_TOTAL_REAL,
      MemoryCollector.OID_MEM_AVAIL_REAL,
      MemoryCollector.OID_MEM_TOTAL_FREE,
    ])

    const totalKb = snmpNumber(response[MemoryCollector.OID_MEM_TOTAL_REAL])
    const availKb = snmpNumber(response[MemoryCollector.OID_MEM_AVAIL_REAL])
    const freeKb = snmpNumber(response[MemoryCollector.OID_MEM_TOTAL_FREE])

    const result: SnmpMemoryInfo = { totalKb, availKb, freeKb }

    const available = availKb !== undefined && !Number.isNaN(availKb) ? availKb : freeKb
    if (totalKb && totalKb > 0 && available !== undefined && !Number.isNaN(available)) {
      result.usedKb = Math.max(0, totalKb - available)
      result.usedPercent = Math.round((result.usedKb / totalKb) * 100)
    }

    return result
  }
}
