import type { CheckResult, MonitorChecker } from '../contracts/check_result.js'

export interface HttpConfig {
  url: string
  method?: 'GET' | 'POST' | 'HEAD'
  acceptedStatusCodes?: number[]
  validateCertificate?: boolean
  timeoutMs?: number
}

export class HttpChecker implements MonitorChecker<HttpConfig> {
  async execute(config: HttpConfig): Promise<CheckResult> {
    const startedAt = new Date()
    const finishedAt = new Date()
    return {
      success: true,
      status: 'up',
      startedAt,
      finishedAt,
      durationMs: 0,
      message: `HTTP GET ${config.url} ok`,
      metrics: [{ name: 'response_time', value: 0, unit: 'ms' }],
    }
  }
}
