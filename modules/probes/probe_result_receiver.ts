import { ResultProcessor } from '#modules/monitoring/result_processor'
import type { CheckResult } from '#modules/monitoring/contracts/check_result'
import { errorMessage } from '#modules/shared/errors'

export interface ProbeResultPayload {
  monitorId: number
  taskId?: string
  result: CheckResult
}

export class ProbeResultReceiver {
  private resultProcessor = new ResultProcessor()

  async receiveResult(probeId: number, monitorId: number, result: CheckResult): Promise<void> {
    try {
      await this.resultProcessor.processResult(monitorId, result, probeId)
    } catch (err: unknown) {
      const msg = errorMessage(err)
      console.error(
        `[ProbeResultReceiver] Erro ao processar resultado do monitor #${monitorId} (Probe #${probeId}): ${msg}`
      )
    }
  }

  async receiveBatchResults(probeId: number, payloads: ProbeResultPayload[]): Promise<void> {
    for (const item of payloads) {
      await this.receiveResult(probeId, item.monitorId, item.result)
    }
  }
}
