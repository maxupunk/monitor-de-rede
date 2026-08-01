import type { CheckResult, MonitorChecker } from '../contracts/check_result.js'

export interface HttpConfig {
  url: string
  method?: 'GET' | 'POST' | 'HEAD'
  acceptedStatusCodes?: number[]
  validateCertificate?: boolean
  timeoutMs?: number
  headers?: Record<string, string>
}

export class HttpChecker implements MonitorChecker<HttpConfig> {
  async execute(config: HttpConfig): Promise<CheckResult> {
    const startedAt = new Date()
    const method = config.method || 'GET'
    const timeoutMs = config.timeoutMs || 10000
    const acceptedCodes = config.acceptedStatusCodes || [200, 201, 202, 204, 301, 302]

    try {
      const response = await fetch(config.url, {
        method,
        headers: config.headers,
        signal: AbortSignal.timeout(timeoutMs),
      })

      const finishedAt = new Date()
      const durationMs = finishedAt.getTime() - startedAt.getTime()
      const isStatusAccepted = acceptedCodes.includes(response.status)

      return {
        success: isStatusAccepted,
        status: isStatusAccepted ? 'up' : 'warning',
        startedAt,
        finishedAt,
        durationMs,
        message: `HTTP ${method} ${config.url} respondeu com código ${response.status} em ${durationMs}ms`,
        metrics: [
          { name: 'response_time', value: durationMs, unit: 'ms' },
          { name: 'status_code', value: response.status, unit: 'code' },
        ],
        data: {
          statusCode: response.status,
          statusText: response.statusText,
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
        message: `Falha na requisição HTTP ${method} para ${config.url}: ${errorMessage}`,
        metrics: [{ name: 'response_time', value: durationMs, unit: 'ms' }],
      }
    }
  }
}
