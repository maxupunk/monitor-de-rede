import type { SnmpClient } from '../clients/snmp_client.js'

export interface InterfaceTraffic {
  ifIndex: number
  inOctets: number
  outOctets: number
  inErrors: number
  outErrors: number
  inBps?: number
  outBps?: number
  recordedAt: Date
}

export class TrafficCollector {
  public static readonly BASE_IF_TABLE = '1.3.6.1.2.1.2.2.1'
  public static readonly BASE_IF_XTABLE = '1.3.6.1.2.1.31.1.1.1'

  // ifTable columns
  public static readonly COL_IF_IN_OCTETS = 10
  public static readonly COL_IF_IN_ERRORS = 14
  public static readonly COL_IF_OUT_OCTETS = 16
  public static readonly COL_IF_OUT_ERRORS = 20

  // ifXTable 64-bit columns
  public static readonly COL_IF_HC_IN_OCTETS = 6
  public static readonly COL_IF_HC_OUT_OCTETS = 10

  async collect(client: SnmpClient): Promise<InterfaceTraffic[]> {
    const entries = await client.walk(TrafficCollector.BASE_IF_TABLE)
    const xEntries = await client.walk(TrafficCollector.BASE_IF_XTABLE)

    const map = new Map<number, InterfaceTraffic>()
    const now = new Date()

    const getOrCreate = (index: number): InterfaceTraffic => {
      let item = map.get(index)
      if (!item) {
        item = {
          ifIndex: index,
          inOctets: 0,
          outOctets: 0,
          inErrors: 0,
          outErrors: 0,
          recordedAt: now,
        }
        map.set(index, item)
      }
      return item
    }

    // 32-bit counters from ifTable
    for (const entry of entries) {
      const parts = entry.oid.split('.')
      if (parts.length < 2) continue
      const index = parseInt(parts[parts.length - 1], 10)
      const column = parseInt(parts[parts.length - 2], 10)

      if (isNaN(index) || isNaN(column)) continue

      const item = getOrCreate(index)

      switch (column) {
        case TrafficCollector.COL_IF_IN_OCTETS:
          if (item.inOctets === 0) item.inOctets = Number(entry.value) || 0
          break
        case TrafficCollector.COL_IF_OUT_OCTETS:
          if (item.outOctets === 0) item.outOctets = Number(entry.value) || 0
          break
        case TrafficCollector.COL_IF_IN_ERRORS:
          item.inErrors = Number(entry.value) || 0
          break
        case TrafficCollector.COL_IF_OUT_ERRORS:
          item.outErrors = Number(entry.value) || 0
          break
      }
    }

    // 64-bit HC counters from ifXTable (override 32-bit if available)
    for (const entry of xEntries) {
      const parts = entry.oid.split('.')
      if (parts.length < 2) continue
      const index = parseInt(parts[parts.length - 1], 10)
      const column = parseInt(parts[parts.length - 2], 10)

      if (isNaN(index) || isNaN(column)) continue

      const item = getOrCreate(index)

      switch (column) {
        case TrafficCollector.COL_IF_HC_IN_OCTETS: {
          const val = Number(entry.value) || 0
          if (val > 0 || item.inOctets === 0) {
            item.inOctets = val
          }
          break
        }
        case TrafficCollector.COL_IF_HC_OUT_OCTETS: {
          const val = Number(entry.value) || 0
          if (val > 0 || item.outOctets === 0) {
            item.outOctets = val
          }
          break
        }
      }
    }

    return Array.from(map.values())
  }

  public calculateRates(previous: InterfaceTraffic, current: InterfaceTraffic): { inBps: number; outBps: number } {
    const timeDeltaSec = (current.recordedAt.getTime() - previous.recordedAt.getTime()) / 1000.0
    if (timeDeltaSec <= 0) {
      return { inBps: 0, outBps: 0 }
    }

    let inDiff = current.inOctets - previous.inOctets
    if (inDiff < 0) {
      if (previous.inOctets > 4_294_967_296) {
        // Rollover de contador de 64 bits (2^64)
        inDiff += 18_446_744_073_709_551_616
      } else {
        // Rollover padrão de 32 bits (2^32)
        inDiff += 4_294_967_296
      }
      if (inDiff < 0) {
        // Reinício do equipamento (reboot) ou contador reinicializado
        inDiff = current.inOctets
      }
    }

    let outDiff = current.outOctets - previous.outOctets
    if (outDiff < 0) {
      if (previous.outOctets > 4_294_967_296) {
        outDiff += 18_446_744_073_709_551_616
      } else {
        outDiff += 4_294_967_296
      }
      if (outDiff < 0) {
        outDiff = current.outOctets
      }
    }

    const inBps = Math.max(0, Math.round((inDiff * 8) / timeDeltaSec))
    const outBps = Math.max(0, Math.round((outDiff * 8) / timeDeltaSec))

    return { inBps, outBps }
  }
}
