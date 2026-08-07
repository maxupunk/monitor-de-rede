import dns from 'node:dns/promises'
import type { CheckResult } from '../../monitoring/contracts/check_result.js'
import { PingChecker } from '../../monitoring/checkers/ping_checker.js'
import { expandCidr } from '../cidr_range.js'

export interface DiscoveredHost {
  ipAddress: string
  macAddress?: string
  hostname?: string
  mdnsName?: string
  vendor?: string
  deviceType?: string
  openPorts?: number[]
  confidence: number
  data?: Record<string, unknown>
}

export class IcmpScanner {
  private pingChecker = new PingChecker()

  async scanNetwork(cidr: string): Promise<DiscoveredHost[]> {
    const ips = this.parseCidrToIps(cidr)
    const discovered: DiscoveredHost[] = []

    const batchSize = 20
    for (let i = 0; i < ips.length; i += batchSize) {
      const batch = ips.slice(i, i + batchSize)
      const results = await Promise.all(batch.map((ip) => this.checkHost(ip)))

      for (const res of results) {
        if (res) {
          discovered.push(res)
        }
      }
    }

    return discovered
  }

  private async checkHost(ip: string): Promise<DiscoveredHost | null> {
    const pingRes: CheckResult = await this.pingChecker.execute({
      host: ip,
      packetCount: 1,
      timeoutMs: 1500,
    })

    if (!pingRes.success && pingRes.status === 'down') {
      return null
    }

    let hostname: string | undefined
    try {
      const hostnames = await dns.reverse(ip)
      if (hostnames.length > 0) {
        hostname = hostnames[0]
      }
    } catch {
      // Ignorar falha de PTR DNS reverso
    }

    return {
      ipAddress: ip,
      hostname,
      confidence: 50,
      data: { pingMessage: pingRes.message },
    }
  }

  /**
   * A expansão vive em `cidr_range.ts` porque a mesma resposta é usada fora do
   * scanner — para validar o CIDR cadastrado na rede antes de agendar a
   * varredura e para dizer à UI quantos hosts serão varridos.
   */
  private parseCidrToIps(cidr: string): string[] {
    try {
      return expandCidr(cidr)
    } catch {
      // Faixa malformada: varre apenas o que foi informado, como antes.
      return [String(cidr ?? '').trim()].filter(Boolean)
    }
  }
}
