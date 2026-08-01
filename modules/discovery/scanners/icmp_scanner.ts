import dns from 'node:dns/promises'
import type { CheckResult } from '../../monitoring/contracts/check_result.js'
import { PingChecker } from '../../monitoring/checkers/ping_checker.js'

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

  private parseCidrToIps(cidr: string): string[] {
    if (!cidr.includes('/')) {
      return [cidr.trim()]
    }

    const [baseIp, prefixStr] = cidr.split('/')
    const prefix = Number.parseInt(prefixStr, 10)

    if (Number.isNaN(prefix) || prefix < 16 || prefix > 30) {
      return [baseIp.trim()]
    }

    const parts = baseIp.split('.').map((p) => Number.parseInt(p, 10))
    if (parts.length !== 4) return [baseIp]

    const ipNum = (parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3]
    const mask = ~((1 << (32 - prefix)) - 1)
    const startNum = (ipNum & mask) + 1
    const endNum = (ipNum | ~mask) - 1

    const ips: string[] = []
    const limit = Math.min(endNum, startNum + 254)

    for (let current = startNum; current <= limit; current++) {
      const p1 = (current >>> 24) & 255
      const p2 = (current >>> 16) & 255
      const p3 = (current >>> 8) & 255
      const p4 = current & 255
      ips.push(`${p1}.${p2}.${p3}.${p4}`)
    }

    return ips
  }
}
