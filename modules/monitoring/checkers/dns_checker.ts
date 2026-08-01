import dns from 'node:dns/promises'
import type { CheckResult, MonitorChecker } from '../contracts/check_result.js'

export interface DnsConfig {
  domain: string
  recordType?: 'A' | 'AAAA' | 'MX' | 'TXT' | 'CNAME' | 'NS'
  dnsServer?: string
  timeoutMs?: number
}

export class DnsChecker implements MonitorChecker<DnsConfig> {
  async execute(config: DnsConfig): Promise<CheckResult> {
    const startedAt = new Date()
    const recordType = config.recordType || 'A'
    const timeoutMs = config.timeoutMs || 5000

    try {
      const resolver = config.dnsServer ? new dns.Resolver() : dns
      if (config.dnsServer && resolver instanceof dns.Resolver) {
        resolver.setServers([config.dnsServer])
      }

      const lookupPromise = (async () => {
        switch (recordType) {
          case 'AAAA':
            return await resolver.resolve6(config.domain)
          case 'MX':
            return await resolver.resolveMx(config.domain)
          case 'TXT':
            return await resolver.resolveTxt(config.domain)
          case 'CNAME':
            return await resolver.resolveCname(config.domain)
          case 'NS':
            return await resolver.resolveNs(config.domain)
          case 'A':
          default:
            return await resolver.resolve4(config.domain)
        }
      })()

      const timeoutPromise = new Promise((_, reject) =>
        setTimeout(() => reject(new Error(`Timeout ao resolver DNS em ${timeoutMs}ms`)), timeoutMs)
      )

      const result = (await Promise.race([lookupPromise, timeoutPromise])) as unknown

      const finishedAt = new Date()
      const durationMs = finishedAt.getTime() - startedAt.getTime()

      return {
        success: true,
        status: 'up',
        startedAt,
        finishedAt,
        durationMs,
        message: `DNS ${config.domain} (${recordType}) resolvido com sucesso em ${durationMs}ms`,
        metrics: [{ name: 'resolution_time', value: durationMs, unit: 'ms' }],
        data: {
          records: result,
          recordType,
        },
      }
    } catch (err: unknown) {
      const finishedAt = new Date()
      const durationMs = finishedAt.getTime() - startedAt.getTime()
      const errorMessage = err instanceof Error ? err.message : String(err)

      return {
        success: false,
        status: 'down',
        startedAt,
        finishedAt,
        durationMs,
        message: `Falha ao resolver DNS ${config.domain} (${recordType}): ${errorMessage}`,
        metrics: [{ name: 'resolution_time', value: durationMs, unit: 'ms' }],
      }
    }
  }
}
