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

export class SnmpClient {
  constructor(private config: SnmpConfig) {}

  async get(oids: string[]): Promise<Record<string, unknown>> {
    return {}
  }

  async walk(oid: string): Promise<Record<string, unknown>[]> {
    return []
  }
}
