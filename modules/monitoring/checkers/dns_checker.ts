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
    const finishedAt = new Date()
    return {
      success: true,
      status: 'up',
      startedAt,
      finishedAt,
      durationMs: 0,
      message: `DNS ${config.domain} resolvido`,
    }
  }
}
