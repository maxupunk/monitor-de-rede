import type {
  CheckMetric,
  CheckResult,
  MonitorChecker,
  MonitorStatus,
} from '../contracts/check_result.js'
import {
  DEFAULT_DNS_TIMEOUT_MS,
  measureDnsLookup,
  type DnsLookupSample,
  type DnsProtocol,
} from '#modules/network_tools/dns/dns_latency_service'
import type { DnsRecordType } from '#modules/network_tools/dns/dns_wire'

export interface DnsConfig {
  /** Hostname único (formato histórico da configuração) */
  domain?: string
  /** Vários hostnames medidos na mesma checagem */
  domains?: string[]
  recordType?: DnsRecordType
  /** IP do servidor DNS alvo, aceita `ip:porta` */
  dnsServer?: string
  /** Transporte usado na consulta. Sem valor, usa UDP quando há servidor alvo */
  protocol?: DnsProtocol
  /** Endpoint DNS over HTTPS (obrigatório quando `protocol` é `doh`) */
  dohUrl?: string
  timeoutMs?: number
  /** Acima deste tempo de resolução o monitor entra em `warning` */
  warningThresholdMs?: number
}

const PROTOCOL_LABELS: Record<DnsProtocol, string> = {
  udp: 'UDP',
  tcp: 'TCP',
  doh: 'DoH',
  system: 'resolvedor do sistema',
}

function average(values: number[]): number {
  if (values.length === 0) return 0
  return Number((values.reduce((total, value) => total + value, 0) / values.length).toFixed(3))
}

/**
 * Checker de resolução DNS focado em desempenho: mede o tempo gasto
 * exclusivamente na etapa de resolução do nome (ver `dns_latency_service`) e
 * publica esse tempo como métrica do monitor.
 */
export class DnsChecker implements MonitorChecker<DnsConfig> {
  async execute(config: DnsConfig): Promise<CheckResult> {
    const startedAt = new Date()

    const hostnames = (config.domains?.length ? config.domains : [config.domain])
      .map((value) => (value || '').trim())
      .filter((value): value is string => value.length > 0)

    const recordType = (config.recordType || 'A') as DnsRecordType
    const timeoutMs = config.timeoutMs || DEFAULT_DNS_TIMEOUT_MS
    const protocol: DnsProtocol =
      config.protocol || (config.dohUrl ? 'doh' : config.dnsServer ? 'udp' : 'system')

    if (hostnames.length === 0) {
      const finishedAt = new Date()
      return {
        success: false,
        status: 'down',
        startedAt,
        finishedAt,
        durationMs: finishedAt.getTime() - startedAt.getTime(),
        message: 'Nenhum hostname configurado para a checagem DNS',
        metrics: [],
      }
    }

    // Em série: consultas simultâneas competiriam pelo enlace e inflariam a medição
    const samples: DnsLookupSample[] = []
    for (const hostname of hostnames) {
      samples.push(
        await measureDnsLookup({
          hostname,
          recordType,
          protocol,
          server: protocol === 'doh' ? undefined : config.dnsServer,
          dohUrl: config.dohUrl,
          timeoutMs,
        })
      )
    }

    const finishedAt = new Date()
    const durationMs = finishedAt.getTime() - startedAt.getTime()

    const successful = samples.filter((sample) => sample.success)
    const lookupTimes = successful.map((sample) => sample.lookupTimeMs)
    const avgLookupTime = average(lookupTimes)
    const serverLabel = samples[0]?.server || 'resolvedor do sistema'
    const protocolLabel = PROTOCOL_LABELS[protocol]

    const status = this.resolveStatus(samples.length, successful.length, avgLookupTime, config)

    return {
      success: status === 'up',
      status,
      startedAt,
      finishedAt,
      durationMs,
      message: this.buildMessage(samples, successful, avgLookupTime, serverLabel, protocolLabel),
      metrics: this.buildMetrics(avgLookupTime, lookupTimes, samples),
      data: {
        server: serverLabel,
        protocol,
        recordType,
        hostnames,
        avgLookupTimeMs: avgLookupTime,
        minLookupTimeMs: lookupTimes.length ? Math.min(...lookupTimes) : null,
        maxLookupTimeMs: lookupTimes.length ? Math.max(...lookupTimes) : null,
        lookups: samples.map((sample) => ({
          hostname: sample.hostname,
          success: sample.success,
          lookupTimeMs: sample.lookupTimeMs,
          addresses: sample.addresses,
          rcodeLabel: sample.rcodeLabel,
          usedTcpFallback: sample.usedTcpFallback,
          error: sample.error,
        })),
      },
    }
  }

  private resolveStatus(
    total: number,
    successCount: number,
    avgLookupTime: number,
    config: DnsConfig
  ): MonitorStatus {
    if (successCount === 0) return 'down'
    if (successCount < total) return 'warning'
    if (config.warningThresholdMs && avgLookupTime > config.warningThresholdMs) return 'warning'
    return 'up'
  }

  private buildMetrics(
    avgLookupTime: number,
    lookupTimes: number[],
    samples: DnsLookupSample[]
  ): CheckMetric[] {
    const metrics: CheckMetric[] = [
      { name: 'dns_lookup_time', value: avgLookupTime, unit: 'ms' },
      // Nome histórico mantido para não quebrar regras de alerta já cadastradas
      { name: 'resolution_time', value: avgLookupTime, unit: 'ms' },
    ]

    if (lookupTimes.length > 0) {
      metrics.push({ name: 'dns_lookup_time_min', value: Math.min(...lookupTimes), unit: 'ms' })
      metrics.push({ name: 'dns_lookup_time_max', value: Math.max(...lookupTimes), unit: 'ms' })
    }

    metrics.push({
      name: 'dns_success_rate',
      value: samples.length ? Number(((lookupTimes.length / samples.length) * 100).toFixed(1)) : 0,
      unit: '%',
    })

    return metrics
  }

  private buildMessage(
    samples: DnsLookupSample[],
    successful: DnsLookupSample[],
    avgLookupTime: number,
    serverLabel: string,
    protocolLabel: string
  ): string {
    const via = `${serverLabel} via ${protocolLabel}`

    if (successful.length === 0) {
      const reason = samples[0]?.error || 'sem resposta'
      return samples.length === 1
        ? `Falha ao resolver ${samples[0]?.hostname} em ${via}: ${reason}`
        : `Falha ao resolver ${samples.length} nomes em ${via}: ${reason}`
    }

    if (successful.length < samples.length) {
      const failed = samples.filter((sample) => !sample.success).map((sample) => sample.hostname)
      return `${successful.length}/${samples.length} nomes resolvidos em ${via} (média ${avgLookupTime}ms) — falhou: ${failed.join(', ')}`
    }

    if (samples.length === 1) {
      const sample = samples[0]!
      const addresses = sample.addresses.length ? ` → ${sample.addresses.join(', ')}` : ''
      return `${sample.hostname} resolvido em ${sample.lookupTimeMs}ms por ${via}${addresses}`
    }

    return `${samples.length} nomes resolvidos por ${via} — média de ${avgLookupTime}ms`
  }
}
