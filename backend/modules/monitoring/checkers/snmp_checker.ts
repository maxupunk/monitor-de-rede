import type { CheckResult, MonitorStatus } from '../contracts/check_result.js'
import { SnmpClient } from '#modules/snmp/clients/snmp_client'
import { SystemCollector } from '#modules/snmp/collectors/system_collector'
import { InterfaceCollector } from '#modules/snmp/collectors/interface_collector'
import { TrafficCollector } from '#modules/snmp/collectors/traffic_collector'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import Metric from '#models/metric'
import { SnmpService } from '#modules/snmp/snmp_service'
import { formatSpeed } from '#modules/monitoring/interface_monitoring_service'
import { errorMessage } from '#modules/shared/errors'

export interface SnmpCheckerConfig {
  host: string
  version?: 'v1' | 'v2c' | 'v3'
  community?: string
  port?: number
  timeoutMs?: number
  metric?: string
  ifIndex?: number
  ifName?: string
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
      const device = await Device.query().where('ipAddress', host).orWhere('name', host).first()

      if (device && device.snmpEnabled) {
        await this.snmpService.pollDevice(device, { host, version, community, port, timeoutMs })
      }
    } catch {}

    try {
      if (config.metric === 'interface_traffic' && config.ifIndex !== undefined && config.ifIndex !== null) {
        return await this.executeInterfaceTraffic(client, config, host, startedAt, startTime)
      }

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
            adminStatusCode: Number.isNaN(adminStatusVal) ? null : adminStatusVal,
            adminStatusText: adminLabel,
            operStatusCode: Number.isNaN(operStatusVal) ? null : operStatusVal,
            operStatusText: operLabel,
            speedBps: effectiveSpeedBps > 0 ? effectiveSpeedBps : null,
            speedFormatted,
          },
          metrics: [
            {
              name: 'if_oper_status',
              value: Number.isNaN(operStatusVal) ? 0 : operStatusVal,
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
        message: errorMessage(error),
        metrics: [],
      }
    }
  }

  /**
   * Coleta tráfego (in/out bps) de uma interface específica.
   *
   * Lê os contadores de octets (32-bit e 64-bit) via SNMP, busca a leitura
   * anterior no banco de métricas e calcula a taxa usando `TrafficCollector`.
   * Na primeira leitura (sem dado anterior) retorna 0 bps como baseline.
   */
  private async executeInterfaceTraffic(
    client: SnmpClient,
    config: SnmpCheckerConfig,
    host: string,
    startedAt: Date,
    startTime: number
  ): Promise<CheckResult> {
    const ifIndex = config.ifIndex!

    // OIDs de contadores de octets (32-bit ifTable + 64-bit ifXTable)
    const inOctetsOid = `${TrafficCollector.BASE_IF_TABLE}.${TrafficCollector.COL_IF_IN_OCTETS}.${ifIndex}`
    const outOctetsOid = `${TrafficCollector.BASE_IF_TABLE}.${TrafficCollector.COL_IF_OUT_OCTETS}.${ifIndex}`
    const hcInOctetsOid = `${TrafficCollector.BASE_IF_XTABLE}.${TrafficCollector.COL_IF_HC_IN_OCTETS}.${ifIndex}`
    const hcOutOctetsOid = `${TrafficCollector.BASE_IF_XTABLE}.${TrafficCollector.COL_IF_HC_OUT_OCTETS}.${ifIndex}`

    const response = await client.get([inOctetsOid, outOctetsOid, hcInOctetsOid, hcOutOctetsOid])
    const endTime = Date.now()
    const durationMs = endTime - startTime
    const finishedAt = new Date()

    // Prefere contadores HC (64-bit) quando disponíveis
    const rawIn32 = Number(response[inOctetsOid]) || 0
    const rawOut32 = Number(response[outOctetsOid]) || 0
    const rawInHc = Number(response[hcInOctetsOid]) || 0
    const rawOutHc = Number(response[hcOutOctetsOid]) || 0
    const currentInOctets = rawInHc > 0 ? rawInHc : rawIn32
    const currentOutOctets = rawOutHc > 0 ? rawOutHc : rawOut32

    const noData = currentInOctets === 0 && currentOutOctets === 0
    if (noData) {
      return {
        success: false,
        status: 'down',
        startedAt,
        finishedAt,
        durationMs,
        message: `Interface #${ifIndex} não retornou contadores de tráfego`,
        metrics: [],
      }
    }

    // Buscar leitura anterior no banco para calcular a taxa
    let inBps = 0
    let outBps = 0

    try {
      const device = await Device.query().where('ipAddress', host).orWhere('name', host).first()
      if (device) {
        const targetIface = await DeviceInterface.query()
          .where('deviceId', device.id)
          .where('snmpIndex', ifIndex)
          .first()

        if (targetIface) {
          const lastIn = await Metric.query()
            .where('deviceId', device.id)
            .where('interfaceId', targetIface.id)
            .whereIn('name', ['ifHCInOctets', 'ifInOctets'])
            .orderBy('recordedAt', 'desc')
            .first()

          const lastOut = await Metric.query()
            .where('deviceId', device.id)
            .where('interfaceId', targetIface.id)
            .whereIn('name', ['ifHCOutOctets', 'ifOutOctets'])
            .orderBy('recordedAt', 'desc')
            .first()

          if (lastIn?.recordedAt && lastOut?.recordedAt) {
            const trafficCollector = new TrafficCollector()
            const parseDate = (val: unknown): Date => {
              if (val instanceof Date) return val
              if (val && typeof (val as { toJSDate?: () => Date }).toJSDate === 'function') {
                return (val as { toJSDate: () => Date }).toJSDate()
              }
              if (typeof val === 'string') return new Date(val)
              return new Date()
            }

            const prevTraffic = {
              ifIndex,
              inOctets: Number(lastIn.value) || 0,
              outOctets: Number(lastOut.value) || 0,
              inErrors: 0,
              outErrors: 0,
              recordedAt: parseDate(lastIn.recordedAt),
            }
            const currentTraffic = {
              ifIndex,
              inOctets: currentInOctets,
              outOctets: currentOutOctets,
              inErrors: 0,
              outErrors: 0,
              recordedAt: finishedAt,
            }
            const rates = trafficCollector.calculateRates(prevTraffic, currentTraffic)
            inBps = rates.inBps
            outBps = rates.outBps
          }
        }
      }
    } catch {
      // Falha ao consultar histórico — retorna 0 bps como baseline
    }

    const ifLabel = config.ifName ? `${config.ifName} (#${ifIndex})` : `#${ifIndex}`

    return {
      success: true,
      status: 'up',
      startedAt,
      finishedAt,
      durationMs,
      message: `Interface ${ifLabel}: ↓ ${this.formatBps(inBps)} / ↑ ${this.formatBps(outBps)}`,
      data: {
        ifIndex,
        ifName: config.ifName ?? null,
        inBps,
        outBps,
        inOctets: currentInOctets,
        outOctets: currentOutOctets,
      },
      metrics: [
        { name: 'inBps', value: inBps, unit: 'bps' },
        { name: 'outBps', value: outBps, unit: 'bps' },
        { name: 'ifHCInOctets', value: currentInOctets, unit: 'bytes' },
        { name: 'ifHCOutOctets', value: currentOutOctets, unit: 'bytes' },
      ],
    }
  }

  /** Formata bps em unidade legível (bps, Kbps, Mbps, Gbps) */
  private formatBps(bps: number): string {
    if (bps >= 1_000_000_000) return `${(bps / 1_000_000_000).toFixed(2)} Gbps`
    if (bps >= 1_000_000) return `${(bps / 1_000_000).toFixed(1)} Mbps`
    if (bps >= 1_000) return `${(bps / 1_000).toFixed(0)} Kbps`
    return `${bps} bps`
  }
}
