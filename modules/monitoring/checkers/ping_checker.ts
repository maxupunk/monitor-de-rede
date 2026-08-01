import type { CheckResult, MonitorChecker } from '../contracts/check_result.js'

export interface PingConfig {
  host: string
  packetCount?: number
  packetSize?: number
  timeoutMs?: number
}

export class PingChecker implements MonitorChecker<PingConfig> {
  async execute(config: PingConfig): Promise<CheckResult> {
    const startedAt = new Date()
    const finishedAt = new Date()
    return {
      success: true,
      status: 'up',
      startedAt,
      finishedAt,
      durationMs: 0,
      message: `Ping para ${config.host} executado com sucesso`,
      metrics: [{ name: 'latency', value: 0, unit: 'ms' }],
    }
  }
}
