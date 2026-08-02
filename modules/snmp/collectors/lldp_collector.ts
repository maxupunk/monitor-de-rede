import type { SnmpClient } from '../clients/snmp_client.js'

export interface LldpNeighbor {
  localPort: string
  remotePort: string
  remoteSysName: string
  remoteMgmtAddress?: string
  protocol: 'lldp' | 'cdp'
}

export class LldpCollector {
  public static readonly BASE_LLDP_REM_TABLE = '1.0.8802.1.1.2.1.4.1.1'
  public static readonly BASE_CDP_CACHE_TABLE = '1.3.6.1.4.1.9.9.23.1.2.1'

  // LLDP columns (1.0.8802.1.1.2.1.4.1.1.X)
  public static readonly COL_LLDP_PORT_ID = 7
  public static readonly COL_LLDP_SYS_NAME = 9

  // CDP columns (1.3.6.1.4.1.9.9.23.1.2.1.1.X)
  public static readonly COL_CDP_ADDRESS = 4
  public static readonly COL_CDP_DEVICE_ID = 6
  public static readonly COL_CDP_DEVICE_PORT = 7

  async collect(client: SnmpClient): Promise<LldpNeighbor[]> {
    const neighbors: LldpNeighbor[] = []

    // 1. LLDP Collection
    const lldpEntries = await client.walk(LldpCollector.BASE_LLDP_REM_TABLE)
    const lldpMap = new Map<string, { localPort: string; remotePort?: string; remoteSysName?: string }>()

    for (const entry of lldpEntries) {
      const parts = entry.oid.split('.')
      if (parts.length < 13) continue

      // In LLDP remTable (1.0.8802.1.1.2.1.4.1.1.[column].[timeMark].[localPort].[remIndex])
      const column = parseInt(parts[10], 10)
      const localPortNum = parts[parts.length - 2]
      const remIndex = parts[parts.length - 1]
      const key = `${localPortNum}_${remIndex}`

      let item = lldpMap.get(key)
      if (!item) {
        item = { localPort: localPortNum }
        lldpMap.set(key, item)
      }

      if (column === LldpCollector.COL_LLDP_PORT_ID) {
        item.remotePort = String(entry.value)
      } else if (column === LldpCollector.COL_LLDP_SYS_NAME) {
        item.remoteSysName = String(entry.value)
      }
    }

    for (const item of lldpMap.values()) {
      if (item.remoteSysName || item.remotePort) {
        neighbors.push({
          localPort: item.localPort,
          remotePort: item.remotePort || 'unknown',
          remoteSysName: item.remoteSysName || 'unknown',
          protocol: 'lldp',
        })
      }
    }

    // 2. CDP Collection
    const cdpEntries = await client.walk(LldpCollector.BASE_CDP_CACHE_TABLE)
    const cdpMap = new Map<string, { localPort: string; remotePort?: string; remoteSysName?: string; remoteMgmtAddress?: string }>()

    for (const entry of cdpEntries) {
      const parts = entry.oid.split('.')
      if (parts.length < 12) continue

      // In CDP cacheTable (1.3.6.1.4.1.9.9.23.1.2.1.1.[column].[ifIndex].[cdpIndex])
      const column = parseInt(parts[11], 10)
      const ifIndex = parts[parts.length - 2]
      const key = `${ifIndex}`

      let item = cdpMap.get(key)
      if (!item) {
        item = { localPort: ifIndex }
        cdpMap.set(key, item)
      }

      if (column === LldpCollector.COL_CDP_DEVICE_ID) {
        item.remoteSysName = String(entry.value)
      } else if (column === LldpCollector.COL_CDP_DEVICE_PORT) {
        item.remotePort = String(entry.value)
      } else if (column === LldpCollector.COL_CDP_ADDRESS) {
        item.remoteMgmtAddress = String(entry.value)
      }
    }

    for (const item of cdpMap.values()) {
      if (item.remoteSysName || item.remotePort) {
        neighbors.push({
          localPort: item.localPort,
          remotePort: item.remotePort || 'unknown',
          remoteSysName: item.remoteSysName || 'unknown',
          remoteMgmtAddress: item.remoteMgmtAddress,
          protocol: 'cdp',
        })
      }
    }

    return neighbors
  }
}
