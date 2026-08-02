export interface SnmpConfig {
  host: string
  version: 'v1' | 'v2c' | 'v3'
  community?: string
  username?: string
  authProtocol?: 'MD5' | 'SHA'
  authKey?: string
  privProtocol?: 'DES' | 'AES'
  privKey?: string
  port?: number
  timeoutMs?: number
}

export interface SnmpWalkEntry {
  oid: string
  value: unknown
}

export class SnmpClient {
  private mockGetResponses: Map<string, unknown> = new Map()
  private mockWalkResponses: Map<string, SnmpWalkEntry[]> = new Map()

  constructor(public config: SnmpConfig) {}

  public setMockGet(oidMap: Record<string, unknown>): void {
    for (const [oid, val] of Object.entries(oidMap)) {
      this.mockGetResponses.set(oid, val)
    }
  }

  public setMockWalk(baseOid: string, entries: SnmpWalkEntry[]): void {
    this.mockWalkResponses.set(baseOid, entries)
  }

  async get(oids: string[]): Promise<Record<string, unknown>> {
    const result: Record<string, unknown> = {}
    for (const oid of oids) {
      if (this.mockGetResponses.has(oid)) {
        result[oid] = this.mockGetResponses.get(oid)
      } else {
        result[oid] = null
      }
    }
    return result
  }

  async walk(baseOid: string): Promise<SnmpWalkEntry[]> {
    if (this.mockWalkResponses.has(baseOid)) {
      return this.mockWalkResponses.get(baseOid)!
    }
    for (const [key, entries] of this.mockWalkResponses.entries()) {
      if (key.startsWith(baseOid) || baseOid.startsWith(key)) {
        return entries
      }
    }
    return []
  }
}
