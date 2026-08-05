import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import type { CheckResult, MonitorChecker } from '../contracts/check_result.js'
import { errorMessage } from '#modules/shared/errors'

const execFileAsync = promisify(execFile)

export interface PingConfig {
  host: string
  packetCount?: number
  packetSize?: number
  timeoutMs?: number
}

export class PingChecker implements MonitorChecker<PingConfig> {
  async execute(config: PingConfig): Promise<CheckResult> {
    const startedAt = new Date()
    const count = config.packetCount || 3
    const timeoutMs = config.timeoutMs || 5000
    const isWindows = process.platform === 'win32'

    const args = isWindows
      ? ['-n', String(count), '-w', String(timeoutMs), config.host]
      : ['-c', String(count), '-W', String(Math.ceil(timeoutMs / 1000)), config.host]

    try {
      const { stdout } = await execFileAsync('ping', args, { timeout: timeoutMs + 2000 })
      const finishedAt = new Date()
      const durationMs = finishedAt.getTime() - startedAt.getTime()

      let latencyMs = durationMs / count
      let packetLoss = 0

      if (isWindows) {
        const lossMatch = stdout.match(/\((\d+)%\s+loss\)/i) || stdout.match(/perda\s+de\s+(\d+)%/i)
        if (lossMatch && lossMatch[1]) {
          packetLoss = Number.parseInt(lossMatch[1], 10)
        }
        const timeMatch =
          stdout.match(/Average\s*=\s*(\d+)ms/i) || stdout.match(/M[eé]dia\s*=\s*(\d+)ms/i)
        if (timeMatch && timeMatch[1]) {
          latencyMs = Number.parseFloat(timeMatch[1])
        }
      } else {
        const lossMatch = stdout.match(/(\d+)%\s+packet loss/i)
        if (lossMatch && lossMatch[1]) {
          packetLoss = Number.parseInt(lossMatch[1], 10)
        }
        // iputils imprime "rtt min/avg/max/mdev = ..." e o BusyBox (Alpine, usado
        // nas imagens Docker) imprime "round-trip min/avg/max = ...".
        const rttMatch =
          stdout.match(/rtt min\/avg\/max\/mdev = [\d.]+\/([\d.]+)\//i) ||
          stdout.match(/round-trip min\/avg\/max\s*=\s*[\d.]+\/([\d.]+)\//i)
        if (rttMatch && rttMatch[1]) {
          latencyMs = Number.parseFloat(rttMatch[1])
        }
      }

      const isUp = packetLoss < 100

      return {
        success: isUp,
        status: isUp ? (packetLoss > 0 ? 'warning' : 'up') : 'down',
        startedAt,
        finishedAt,
        durationMs,
        message: isUp
          ? `Ping para ${config.host} finalizado em ${latencyMs.toFixed(1)}ms (${packetLoss}% perda)`
          : `Host ${config.host} inacessível (100% perda de pacotes)`,
        metrics: [
          { name: 'latency', value: Number(latencyMs.toFixed(2)), unit: 'ms' },
          { name: 'packet_loss', value: packetLoss, unit: '%' },
        ],
      }
    } catch (err: unknown) {
      const finishedAt = new Date()
      const durationMs = finishedAt.getTime() - startedAt.getTime()
      return {
        success: false,
        status: 'down',
        startedAt,
        finishedAt,
        durationMs,
        message: `Falha ao executar ping em ${config.host}: ${errorMessage(err)}`,
        metrics: [
          { name: 'latency', value: 0, unit: 'ms' },
          { name: 'packet_loss', value: 100, unit: '%' },
        ],
      }
    }
  }
}
