import type { CheckResult, MonitorStatus } from '../contracts/check_result.js'
import { SnmpClient } from '#modules/snmp/clients/snmp_client'
import { SystemCollector } from '#modules/snmp/collectors/system_collector'
import { InterfaceCollector } from '#modules/snmp/collectors/interface_collector'
import Device from '#models/device'
import { SnmpService } from '#modules/snmp/snmp_service'
import { formatSpeed } from '#modules/monitoring/interface_monitoring_service'

export interface SnmpCheckerConfig {
  host: string
  version?: 'v1' | 'v2c' | 'v3'
  community?: string
  port?: number
  timeoutMs?: number
  metric?: string
  ifIndex?: number
}

/**
 * Estados de ifOperStatus (RFC 2863 / IF-MIB) — uma interface não é apenas up/down:
 * pode estar em teste, dormente (aguardando evento externo), sem hardware presente
 * (ex: SFP não inserido) ou inativa por dependência de camada inferior (ex: VLAN
 * sobre um link físico que caiu).
 */
const IF_OPER_STATUS_LABELS: Record<number, string> = {
  1: 'Up',
  2: 'Down',
  3: 'Em Teste',
  4: 'Desconhecido',
  5: 'Dormente',
  6: 'Não Presente',
  7: 'Camada Inferior Inativa',
}

const IF_OPER_STATUS_TO_MONITOR_STATUS: Record<number, MonitorStatus> = {
  1: 'up',
  2: 'down',
  3: 'warning',
  4: 'unknown',
  5: 'warning',
  6: 'down',
  7: 'down',
}

const IF_ADMIN_STATUS_LABELS: Record<number, string> = {
  1: 'Habilitada',
  2: 'Desabilitada (Admin Down)',
  3: 'Em Teste (Admin)',
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
        const adminStatusOid = `1.3.6.1.2.1.2.2.1.7.${config.ifIndex}`
        const operStatusOid = `1.3.6.1.2.1.2.2.1.8.${config.ifIndex}`
        const speedOid = `1.3.6.1.2.1.2.2.1.5.${config.ifIndex}`
        const highSpeedOid = `${InterfaceCollector.BASE_IF_XTABLE}.${InterfaceCollector.COL_IF_HIGH_SPEED}.${config.ifIndex}`

        const ifResponse = await client.get([adminStatusOid, operStatusOid, speedOid, highSpeedOid])
        const endTime = Date.now()
        const durationMs = endTime - startTime
        const finishedAt = new Date()

        const adminStatusVal = Number(ifResponse[adminStatusOid])
        const operStatusVal = Number(ifResponse[operStatusOid])
        const speedVal = Number(ifResponse[speedOid] || 0)

        // ifHighSpeed (Mbps) é mais confiável que ifSpeed (32-bit, satura em ~4.29 Gbps)
        // para links de 1G, 2.5G e superiores — usa-o quando disponível.
        const highSpeedRaw = ifResponse[highSpeedOid]
        const highSpeedBps =
          highSpeedRaw !== undefined && highSpeedRaw !== null ? Number(highSpeedRaw) * 1_000_000 : 0
        const effectiveSpeedBps = highSpeedBps > 0 ? highSpeedBps : speedVal

        const isAdminDown = adminStatusVal === 2
        const operLabel = IF_OPER_STATUS_LABELS[operStatusVal] || 'Desconhecido'
        const adminLabel = IF_ADMIN_STATUS_LABELS[adminStatusVal] || null
        const status: MonitorStatus = isAdminDown
          ? 'disabled'
          : IF_OPER_STATUS_TO_MONITOR_STATUS[operStatusVal] || 'unknown'
        const speedFormatted = effectiveSpeedBps > 0 ? formatSpeed(effectiveSpeedBps) : null

        let message: string
        if (isAdminDown) {
          message = `Interface #${config.ifIndex} desabilitada administrativamente`
        } else if (operStatusVal === 1) {
          message = speedFormatted
            ? `Interface #${config.ifIndex} operacional (Up) — ${speedFormatted}`
            : `Interface #${config.ifIndex} operacional (Up)`
        } else {
          message = `Interface #${config.ifIndex} ${operLabel}`
        }

        return {
          success: status === 'up',
          status,
          startedAt,
          finishedAt,
          durationMs,
          message,
          data: {
            ifIndex: config.ifIndex,
            adminStatusCode: isNaN(adminStatusVal) ? null : adminStatusVal,
            adminStatusText: adminLabel,
            operStatusCode: isNaN(operStatusVal) ? null : operStatusVal,
            operStatusText: operLabel,
            speedBps: effectiveSpeedBps > 0 ? effectiveSpeedBps : null,
            speedFormatted,
          },
          metrics: [
            {
              name: 'if_oper_status',
              value: isNaN(operStatusVal) ? 0 : operStatusVal,
              unit: 'status',
            },
            {
              name: 'if_speed',
              value: effectiveSpeedBps,
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
