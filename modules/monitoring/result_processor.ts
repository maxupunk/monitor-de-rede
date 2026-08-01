import type { CheckResult, MonitorStatus } from './contracts/check_result.js'

export class ResultProcessor {
  async processResult(monitorId: string, result: CheckResult): Promise<void> {
    // Processamento do resultado do monitoramento
  }
}
