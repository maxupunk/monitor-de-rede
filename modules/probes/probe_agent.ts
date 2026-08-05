import { MonitorRunner } from '#modules/monitoring/monitor_runner'
import { ProbeBuffer } from './probe_buffer.js'
import type { ProbeTask } from './probe_task_dispatcher.js'
import type { CheckResult } from '#modules/monitoring/contracts/check_result'

export interface ProbeAgentOptions {
  serverUrl?: string
  probeToken?: string
  intervalMs?: number
  bufferPath?: string
  version?: string
}

export class ProbeAgent {
  private serverUrl: string
  private probeToken: string
  private intervalMs: number
  private version: string
  private buffer: ProbeBuffer
  private runner = new MonitorRunner()
  private isRunning = false

  constructor(options?: ProbeAgentOptions) {
    this.serverUrl =
      options?.serverUrl ||
      process.env.PROBE_SERVER_URL ||
      process.env.SERVER_URL ||
      'http://localhost:3333'
    this.probeToken = options?.probeToken || process.env.PROBE_TOKEN || ''
    this.intervalMs = options?.intervalMs || Number(process.env.PROBE_INTERVAL_MS) || 5000
    this.version = options?.version || '1.0.0'
    this.buffer = new ProbeBuffer(options?.bufferPath)
  }

  async start(): Promise<void> {
    this.isRunning = true
    console.log(`[ProbeAgent] Inicializado conectando em ${this.serverUrl}`)

    while (this.isRunning) {
      try {
        await this.step()
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err)
        console.error(`[ProbeAgent] Erro no ciclo de execução: ${msg}`)
      }
      await new Promise((resolve) => setTimeout(resolve, this.intervalMs))
    }
  }

  stop(): void {
    this.isRunning = false
  }

  async step(): Promise<void> {
    const isOnline = await this.sendHeartbeat()

    if (isOnline) {
      await this.flushOfflineBuffer()
    }

    const tasks = await this.fetchTasks()
    if (!tasks || tasks.length === 0) {
      return
    }

    for (const task of tasks) {
      await this.executeAndReportTask(task)
    }
  }

  private async sendHeartbeat(): Promise<boolean> {
    try {
      const res = await fetch(`${this.serverUrl}/api/probes/heartbeat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Probe-Token': this.probeToken,
        },
        body: JSON.stringify({
          version: this.version,
          configuration: {
            nodeVersion: process.version,
            platform: process.platform,
            arch: process.arch,
          },
        }),
      })
      return res.ok
    } catch {
      return false
    }
  }

  private async fetchTasks(): Promise<ProbeTask[]> {
    try {
      const res = await fetch(`${this.serverUrl}/api/probes/tasks`, {
        method: 'GET',
        headers: {
          'X-Probe-Token': this.probeToken,
        },
      })
      if (!res.ok) return []
      const data = (await res.json()) as { tasks: ProbeTask[] }
      return data.tasks || []
    } catch {
      return []
    }
  }

  private async executeAndReportTask(task: ProbeTask): Promise<void> {
    let result: CheckResult
    try {
      result = await this.runner.runMonitor(task.type, task.payload)
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      const now = new Date()
      result = {
        success: false,
        status: 'down',
        durationMs: 0,
        startedAt: now,
        finishedAt: now,
        message: `Falha na execução pelo probe: ${msg}`,
        metrics: [],
        data: { error: msg },
      }
    }

    const reported = await this.reportResult(task.monitorId, task.id, result)
    if (!reported) {
      await this.buffer.saveResultOffline(task.id, { monitorId: task.monitorId, result })
    }
  }

  private async reportResult(
    monitorId: number,
    taskId: string,
    result: CheckResult
  ): Promise<boolean> {
    try {
      const res = await fetch(`${this.serverUrl}/api/probes/results`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Probe-Token': this.probeToken,
        },
        body: JSON.stringify({
          results: [
            {
              monitorId,
              taskId,
              result,
            },
          ],
        }),
      })
      return res.ok
    } catch {
      return false
    }
  }

  private async flushOfflineBuffer(): Promise<void> {
    const pending = await this.buffer.getPendingResults()
    if (pending.length === 0) return

    const resultsToReport = pending.map((item) => {
      const payload = item.result as { monitorId: number; result: CheckResult }
      return {
        monitorId: payload.monitorId,
        taskId: item.taskId,
        result: payload.result,
      }
    })

    try {
      const res = await fetch(`${this.serverUrl}/api/probes/results`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Probe-Token': this.probeToken,
        },
        body: JSON.stringify({ results: resultsToReport }),
      })

      if (res.ok) {
        await this.buffer.clearPendingResults()
      }
    } catch {
      // Falha ao reenviar, mantém no buffer
    }
  }
}
