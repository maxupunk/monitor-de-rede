import net from 'node:net'
import type { CheckResult, MonitorChecker } from '../contracts/check_result.js'

export interface TcpConfig {
  host: string
  port: number
  timeoutMs?: number
}

export class TcpChecker implements MonitorChecker<TcpConfig> {
  async execute(config: TcpConfig): Promise<CheckResult> {
    const startedAt = new Date()
    const timeoutMs = config.timeoutMs || 5000

    return new Promise<CheckResult>((resolve) => {
      const socket = new net.Socket()

      socket.setTimeout(timeoutMs)

      socket.on('connect', () => {
        const finishedAt = new Date()
        const durationMs = finishedAt.getTime() - startedAt.getTime()
        socket.destroy()

        resolve({
          success: true,
          status: 'up',
          startedAt,
          finishedAt,
          durationMs,
          message: `Conexão TCP para ${config.host}:${config.port} estabelecida em ${durationMs}ms`,
          metrics: [{ name: 'connect_time', value: durationMs, unit: 'ms' }],
        })
      })

      socket.on('timeout', () => {
        const finishedAt = new Date()
        const durationMs = finishedAt.getTime() - startedAt.getTime()
        socket.destroy()

        resolve({
          success: false,
          status: 'down',
          startedAt,
          finishedAt,
          durationMs,
          message: `Timeout na conexão TCP para ${config.host}:${config.port} (${timeoutMs}ms)`,
          metrics: [{ name: 'connect_time', value: durationMs, unit: 'ms' }],
        })
      })

      socket.on('error', (err: Error) => {
        const finishedAt = new Date()
        const durationMs = finishedAt.getTime() - startedAt.getTime()
        socket.destroy()

        resolve({
          success: false,
          status: 'down',
          startedAt,
          finishedAt,
          durationMs,
          message: `Erro na conexão TCP para ${config.host}:${config.port}: ${err.message}`,
          metrics: [{ name: 'connect_time', value: durationMs, unit: 'ms' }],
        })
      })

      socket.connect(config.port, config.host)
    })
  }
}
