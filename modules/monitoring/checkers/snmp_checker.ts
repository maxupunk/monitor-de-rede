import type { CheckResult } from '../contracts/check_result.js'
import { SnmpClient } from '#modules/snmp/clients/snmp_client'
import { SystemCollector } from '#modules/snmp/collectors/system_collector'
import Device from '#models/device'
import { SnmpService } from '#modules/snmp/snmp_service'

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
  private snmpService = new SnmpService()

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

    // Atualização de métricas do dispositivo em segundo plano (se o dispositivo existir cadastrado)
    try {
      const device = await Device.query()
        .where('ipAddress', host)
        .orWhere('name', host)
        .first()

      if (device && device.snmpEnabled) {
        await this.snmpService.pollDevice(device, { host, version, community, port, timeoutMs })
      }
    } catch {}

    try {
      if (config.ifIndex !== undefined && config.ifIndex !== null) {
        const operStatusOid = `1.3.6.1.2.1.2.2.1.8.${config.ifIndex}`
        const speedOid = `1.3.6.1.2.1.2.2.1.5.${config.ifIndex}`
        const ifResponse = await client.get([operStatusOid, speedOid])
        const endTime = Date.now()
        const durationMs = endTime - startTime
        const finishedAt = new Date()

        const operStatusVal = Number(ifResponse[operStatusOid])
        const isOperUp = operStatusVal === 1
        const speedVal = Number(ifResponse[speedOid] || 0)

        return {
          success: isOperUp,
          status: isOperUp ? 'up' : 'down',
          startedAt,
          finishedAt,
          durationMs,
          message: isOperUp
            ? `Interface #${config.ifIndex} operacional (UP)`
            : `Interface #${config.ifIndex} inoperante (DOWN)`,
          metrics: [
            {
              name: 'if_oper_status',
              value: operStatusVal,
              unit: 'status',
            },
            {
              name: 'if_speed',
              value: speedVal,
              unit: 'bps',
            },
          ],
        }
      }

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
