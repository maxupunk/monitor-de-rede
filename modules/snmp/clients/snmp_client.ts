import snmp from 'net-snmp'

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

  private createSession(): any {
    const baseOptions = {
      port: this.config.port || 161,
      // UDP não garante entrega — 1 retry só era margem curta demais para varreduras
      // interativas (usuário clicando "Escanear" uma vez); 2 dá mais resiliência a perda
      // de pacote sem estender demais o tempo total de uma consulta que já falhou de vez.
      retries: 2,
      timeout: this.config.timeoutMs || 4000,
    }

    if (this.config.version === 'v3' && this.config.username) {
      const authProtocolMap: Record<string, any> = {
        MD5: snmp.AuthProtocols.md5,
        SHA: snmp.AuthProtocols.sha,
      }
      const privProtocolMap: Record<string, any> = {
        DES: snmp.PrivProtocols.des,
        AES: snmp.PrivProtocols.aes,
      }

      const user: any = {
        name: this.config.username,
        level: snmp.SecurityLevel.noAuthNoPriv,
      }
      if (this.config.authKey && this.config.authProtocol) {
        user.level = snmp.SecurityLevel.authNoPriv
        user.authProtocol = authProtocolMap[this.config.authProtocol] || snmp.AuthProtocols.sha
        user.authKey = this.config.authKey
      }
      if (this.config.privKey && this.config.privProtocol) {
        user.level = snmp.SecurityLevel.authPriv
        user.privProtocol = privProtocolMap[this.config.privProtocol] || snmp.PrivProtocols.aes
        user.privKey = this.config.privKey
      }

      return snmp.createV3Session(this.config.host, user, {
        ...baseOptions,
        version: snmp.Version3,
      } as any)
    }

    const version = this.config.version === 'v1' ? snmp.Version1 : snmp.Version2c
    return snmp.createSession(this.config.host, this.config.community || 'public', {
      ...baseOptions,
      version,
    } as any)
  }

  async get(oids: string[]): Promise<Record<string, unknown>> {
    if (this.mockGetResponses.size > 0) {
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

    return new Promise((resolve) => {
      let session: any
      try {
        session = this.createSession()
      } catch {
        return resolve({})
      }

      session.get(oids, (error: Error | null, varbinds: any[]) => {
        const result: Record<string, unknown> = {}
        if (error || !varbinds) {
          try {
            session.close()
          } catch {}
          return resolve(result)
        }

        for (const vb of varbinds) {
          if (snmp.isVarbindError(vb)) {
            result[vb.oid] = null
          } else {
            result[vb.oid] = this.formatVarbindValue(vb.value)
          }
        }

        try {
          session.close()
        } catch {}
        resolve(result)
      })
    })
  }

  async walk(baseOid: string): Promise<SnmpWalkEntry[]> {
    if (this.mockWalkResponses.size > 0) {
      let entries: SnmpWalkEntry[] = []
      if (this.mockWalkResponses.has(baseOid)) {
        entries = this.mockWalkResponses.get(baseOid)!
      } else {
        for (const [key, itemEntries] of this.mockWalkResponses.entries()) {
          if (key.startsWith(baseOid) || baseOid.startsWith(key)) {
            entries = itemEntries
            break
          }
        }
      }
      return entries.map((entry) => ({
        oid: entry.oid,
        value: this.formatVarbindValue(entry.value),
      }))
    }

    return new Promise((resolve) => {
      let session: any
      try {
        session = this.createSession()
      } catch {
        return resolve([])
      }

      const results: SnmpWalkEntry[] = []

      session.walk(
        baseOid,
        20,
        (varbinds: any[]) => {
          for (const vb of varbinds) {
            if (snmp.isVarbindError(vb)) continue

            // Garantir estritamente que a OID pertence à subárvore baseOid
            const isSubtree = vb.oid.startsWith(baseOid + '.') || vb.oid === baseOid
            if (!isSubtree) {
              // Cancela a varredura se ultrapassar a subárvore solicitada
              return true
            }

            results.push({
              oid: vb.oid,
              value: this.formatVarbindValue(vb.value),
            })
          }
        },
        (_error?: Error | null) => {
          try {
            session.close()
          } catch {}
          resolve(results)
        }
      )
    })
  }

  private formatVarbindValue(value: unknown): unknown {
    if (Buffer.isBuffer(value)) {
      if (value.length === 6) {
        // Formatar diretamente Buffer de 6 bytes como endereço MAC hexadecimal colon-separated
        return Array.from(value)
          .map((b) => b.toString(16).padStart(2, '0'))
          .join(':')
      }

      if (value.length === 8) {
        // Formatar Buffer de 8 bytes como valor numérico de contador 64-bit (Counter64 / net-snmp)
        try {
          const bigVal = value.readBigUInt64BE(0)
          return Number(bigVal)
        } catch {}
      }

      if (value.length === 0) return ''

      // Verificar se é uma string ASCII/UTF-8 válida sem caracteres nulos ou controle
      if (!value.includes(0x00)) {
        const str = value.toString('utf-8')
        if (!str.includes('\uFFFD') && /^[\x09\x0A\x0D\x20-\x7E\u00A0-\uFFFF]*$/.test(str)) {
          return str.trim()
        }
      }

      // Para buffers binários não texto, retornar string hex colon-separated
      return Array.from(value)
        .map((b) => b.toString(16).padStart(2, '0'))
        .join(':')
    }
    return value
  }
}
