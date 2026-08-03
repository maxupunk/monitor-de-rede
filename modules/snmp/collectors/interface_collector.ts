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
  public static readonly BASE_IF_TABLE = '1.3.6.1.2.1.2.2.1'
  public static readonly BASE_IF_XTABLE = '1.3.6.1.2.1.31.1.1.1'

  // Sub OID column numbers for ifTable (1.3.6.1.2.1.2.2.1.X)
  public static readonly COL_IF_INDEX = 1
  public static readonly COL_IF_DESCR = 2
  public static readonly COL_IF_TYPE = 3
  public static readonly COL_IF_SPEED = 5
  public static readonly COL_IF_PHYS_ADDR = 6
  public static readonly COL_IF_ADMIN_STATUS = 7
  public static readonly COL_IF_OPER_STATUS = 8

  // Sub OID column numbers for ifXTable (1.3.6.1.2.1.31.1.1.1.X)
  public static readonly COL_IF_NAME = 1
  public static readonly COL_IF_HIGH_SPEED = 15
  public static readonly COL_IF_ALIAS = 18

  async collect(client: SnmpClient): Promise<SnmpInterface[]> {
    const entries = await client.walk(InterfaceCollector.BASE_IF_TABLE)
    const xEntries = await client.walk(InterfaceCollector.BASE_IF_XTABLE)

    const map = new Map<number, Partial<SnmpInterface>>()

    const getOrCreate = (index: number): Partial<SnmpInterface> => {
      let item = map.get(index)
      if (!item) {
        item = { ifIndex: index }
        map.set(index, item)
      }
      return item
    }

    for (const entry of entries) {
      const parts = entry.oid.split('.')
      if (parts.length < 2) continue
      const index = parseInt(parts[parts.length - 1], 10)
      const column = parseInt(parts[parts.length - 2], 10)

      if (isNaN(index) || isNaN(column)) continue

      const item = getOrCreate(index)

      switch (column) {
        case InterfaceCollector.COL_IF_DESCR:
          item.ifDescr = String(entry.value)
          break
        case InterfaceCollector.COL_IF_TYPE:
          item.ifType = Number(entry.value)
          break
        case InterfaceCollector.COL_IF_SPEED:
          item.ifSpeed = Number(entry.value)
          break
        case InterfaceCollector.COL_IF_PHYS_ADDR:
          item.macAddress = this.formatMac(entry.value)
          break
        case InterfaceCollector.COL_IF_ADMIN_STATUS:
          item.ifAdminStatus = Number(entry.value)
          break
        case InterfaceCollector.COL_IF_OPER_STATUS:
          item.ifOperStatus = Number(entry.value)
          break
      }
    }

    for (const entry of xEntries) {
      const parts = entry.oid.split('.')
      if (parts.length < 2) continue
      const index = parseInt(parts[parts.length - 1], 10)
      const column = parseInt(parts[parts.length - 2], 10)

      if (isNaN(index) || isNaN(column)) continue

      const item = getOrCreate(index)

      switch (column) {
        case InterfaceCollector.COL_IF_NAME:
          item.ifName = String(entry.value)
          break
        case InterfaceCollector.COL_IF_ALIAS:
          item.ifAlias = String(entry.value)
          break
        case InterfaceCollector.COL_IF_HIGH_SPEED:
          // ifHighSpeed is in Mbps (1,000,000 bits/sec)
          if (entry.value !== undefined && entry.value !== null) {
            const highSpeedBps = Number(entry.value) * 1_000_000
            if (highSpeedBps > 0) {
              item.ifSpeed = highSpeedBps
            }
          }
          break
      }
    }

    return Array.from(map.values()).map((iface) => ({
      ifIndex: iface.ifIndex!,
      ifName: iface.ifName || iface.ifDescr || `eth${iface.ifIndex}`,
      ifDescr: iface.ifDescr,
      ifAlias: iface.ifAlias,
      ifType: iface.ifType,
      ifSpeed: iface.ifSpeed,
      ifAdminStatus: iface.ifAdminStatus ?? 1,
      ifOperStatus: iface.ifOperStatus ?? 1,
      macAddress: iface.macAddress,
    }))
  }

  private formatMac(value: unknown): string | undefined {
    if (!value) return undefined

    if (Buffer.isBuffer(value)) {
      if (value.length === 6) {
        return Array.from(value)
          .map((b) => b.toString(16).padStart(2, '0'))
          .join(':')
      }
      if (value.length === 0) return undefined
    }

    const str = String(value).trim()
    if (!str || str === '00:00:00:00:00:00') return undefined

    // Endereço MAC já formatado e.g. "00:25:fe:26:96:7f" ou "00-25-fe-26-96-7f"
    if (/^([0-9a-fA-F]{2}[:-]){5}[0-9a-fA-F]{2}$/.test(str)) {
      return str.toLowerCase()
    }

    // String hexadecimal pura de 12 caracteres e.g. "0025fe26967f"
    if (/^[0-9a-fA-F]{12}$/.test(str)) {
      return str.match(/.{1,2}/g)!.join(':').toLowerCase()
    }

    // String binária bruta de 6 bytes
    if (typeof value === 'string' && value.length === 6) {
      const buf = Buffer.from(value, 'binary')
      if (buf.length === 6) {
        return Array.from(buf)
          .map((b) => b.toString(16).padStart(2, '0'))
          .join(':')
      }
    }

    // Se contém caracteres inválidos/substituição, tenta decodificar como binary buffer
    if (typeof value === 'string' && (str.includes('\uFFFD') || /[\x00-\x1F]/.test(str))) {
      const buf = Buffer.from(value, 'binary')
      if (buf.length === 6) {
        return Array.from(buf)
          .map((b) => b.toString(16).padStart(2, '0'))
          .join(':')
      }
    }

    return str
  }
}
