import type { CheckResult } from '../contracts/check_result.js'
import { SnmpClient } from '#modules/snmp/clients/snmp_client'
import { SystemCollector } from '#modules/snmp/collectors/system_collector'

export interface SnmpCheckerConfig {
  host: string
  version?: 'v1' | 'v2c' | 'v3'
  community?: string
  port?: number
  timeoutMs?: number
  metric?: string
  ifIndex?: number
}

export class SnmpChecker {
  async execute(config: SnmpCheckerConfig): Promise<CheckResult> {
    const startedAt = new Date()
    const startTime = Date.now()

    const host = config.host || '127.0.0.1'
    const version = config.version || 'v2c'
    const community = config.community || 'public'
    const port = config.port || 161
    const timeoutMs = config.timeoutMs || 4000

    const client = new SnmpClient({
      host,
      version,
      community,
      port,
      timeoutMs,
    })

    try {
      const response = await client.get([SystemCollector.OID_SYS_UPTIME])
      const endTime = Date.now()
      const durationMs = endTime - startTime
      const finishedAt = new Date()

      const hasValue =
        response[SystemCollector.OID_SYS_UPTIME] !== null &&
        response[SystemCollector.OID_SYS_UPTIME] !== undefined

      if (hasValue) {
        return {
          success: true,
          status: 'up',
          startedAt,
          finishedAt,
          durationMs,
          message: 'SNMP respondendo com sucesso',
          metrics: [
            {
              name: 'snmp_uptime',
              value: Number(response[SystemCollector.OID_SYS_UPTIME]),
              unit: 'timeticks',
            },
          ],
        }
      }

      return {
        success: false,
        status: 'down',
        startedAt,
        finishedAt,
        durationMs,
        message: 'Sem resposta para OID SNMP de uptime',
        metrics: [],
      }
    } catch (error) {
      const endTime = Date.now()
      const durationMs = endTime - startTime
      const finishedAt = new Date()

      return {
        success: false,
        status: 'down',
        startedAt,
        finishedAt,
        durationMs,
        message: error instanceof Error ? error.message : String(error),
        metrics: [],
      }
    }
  }
}
