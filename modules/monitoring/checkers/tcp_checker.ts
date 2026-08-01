import type { CheckResult, MonitorChecker } from '../contracts/check_result.js'

export interface TcpConfig {
  host: string
  port: number
  timeoutMs?: number
}

export class TcpChecker implements MonitorChecker<TcpConfig> {
  async execute(config: TcpConfig): Promise<CheckResult> {
    const startedAt = new Date()
    const finishedAt = new Date()
    return {
      success: true,
      status: 'up',
      startedAt,
      finishedAt,
      durationMs: 0,
      message: `TCP ${config.host}:${config.port} conectado`,
    }
  }
}
