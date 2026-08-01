import type { CheckResult } from './contracts/check_result.js'

export class MonitorRunner {
  async runMonitor(type: string, config: unknown): Promise<CheckResult> {
    const startedAt = new Date()
    const finishedAt = new Date()
    return {
      success: true,
      status: 'up',
      startedAt,
      finishedAt,
      durationMs: 0,
      message: `Monitor ${type} executado`,
    }
  }
}
