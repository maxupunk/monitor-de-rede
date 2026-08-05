import type { HttpContext } from '@adonisjs/core/http'
import vine from '@vinejs/vine'
import { DateTime } from 'luxon'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import {
  DEFAULT_BENCHMARK_HOSTNAMES,
  DEFAULT_DNS_SERVERS,
  benchmarkDnsServers,
  measureDnsLookup,
  sortByLatency,
  type DnsProtocol,
  type DnsServerTarget,
} from '#modules/network_tools/dns/dns_latency_service'
import { DnsServerRegistry } from '#modules/network_tools/dns/dns_server_registry'
import type { DnsRecordType } from '#modules/network_tools/dns/dns_wire'

/** Agregação por servidor exibida no ranking do dashboard */
interface DnsServerPerformance {
  server: string
  label: string
  protocol: DnsProtocol
  avgLookupTimeMs: number | null
  minLookupTimeMs: number | null
  maxLookupTimeMs: number | null
  successRate: number
  totalChecks: number
  monitorIds: number[]
  lastCheckedAt: string | null
}

export default class DnsController {
  private registry = new DnsServerRegistry()

  /**
   * POST /api/dns/benchmark
   * Compara servidores DNS ao vivo, medindo os mesmos hostnames em cada um.
   * Sem `servers` no corpo, usa a lista de resolvedores públicos padrão.
   */
  async benchmark({ request, response }: HttpContext) {
    const schema = vine.object({
      servers: vine
        .array(
          vine.object({
            server: vine.string().trim().minLength(1).maxLength(255),
            label: vine.string().trim().maxLength(80).optional(),
            protocol: vine.enum(['udp', 'tcp', 'doh']).optional(),
          })
        )
        .minLength(1)
        .maxLength(12)
        .optional(),
      hostnames: vine
        .array(vine.string().trim().minLength(1).maxLength(253))
        .minLength(1)
        .maxLength(10)
        .optional(),
      recordType: vine.enum(['A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS']).optional(),
      timeoutMs: vine.number().range([200, 15000]).optional(),
      rounds: vine.number().range([1, 5]).optional(),
    })

    const payload = await vine.validate({ schema, data: request.all() })

    // Sem lista explícita, compara os servidores cadastrados pelo usuário
    const registered = payload.servers?.length ? [] : await this.registry.benchmarkTargets()

    const servers: DnsServerTarget[] = payload.servers?.length
      ? payload.servers.map((item) => ({
          server: item.server,
          label: item.label,
          protocol: item.protocol as DnsProtocol | undefined,
        }))
      : registered.length > 0
        ? registered
        : DEFAULT_DNS_SERVERS

    const ranking = await benchmarkDnsServers({
      servers,
      hostnames: payload.hostnames?.length ? payload.hostnames : DEFAULT_BENCHMARK_HOSTNAMES,
      recordType: payload.recordType as DnsRecordType | undefined,
      timeoutMs: payload.timeoutMs,
      rounds: payload.rounds,
    })

    return response.ok({
      hostnames: payload.hostnames?.length ? payload.hostnames : DEFAULT_BENCHMARK_HOSTNAMES,
      recordType: payload.recordType ?? 'A',
      measuredAt: DateTime.now().toISO(),
      ranking,
    })
  }

  /**
   * POST /api/dns/lookup
   * Medição avulsa de um hostname — usada para testar a configuração de um
   * monitor DNS antes de salvá-lo.
   */
  async lookup({ request, response }: HttpContext) {
    const schema = vine.object({
      hostname: vine.string().trim().minLength(1).maxLength(253),
      server: vine.string().trim().maxLength(255).optional(),
      protocol: vine.enum(['udp', 'tcp', 'doh', 'system']).optional(),
      dohUrl: vine.string().trim().url().optional(),
      recordType: vine.enum(['A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS']).optional(),
      timeoutMs: vine.number().range([200, 15000]).optional(),
    })

    const payload = await vine.validate({ schema, data: request.all() })

    const sample = await measureDnsLookup({
      hostname: payload.hostname,
      server: payload.server,
      protocol: payload.protocol as DnsProtocol | undefined,
      dohUrl: payload.dohUrl,
      recordType: payload.recordType as DnsRecordType | undefined,
      timeoutMs: payload.timeoutMs,
    })

    return response.ok(sample)
  }

  /**
   * GET /api/dns/performance?hours=24
   * Ranking de latência montado a partir do histórico dos monitores DNS já
   * cadastrados — é o que alimenta o card do dashboard sem disparar consultas.
   */
  async performance({ request, response }: HttpContext) {
    const hours = Math.min(Math.max(Number(request.input('hours', 24)) || 24, 1), 168)
    const cutoff = DateTime.now().minus({ hours })

    const monitors = await Monitor.query().where('type', 'dns')

    if (monitors.length === 0) {
      return response.ok({ windowHours: hours, ranking: [], monitorCount: 0 })
    }

    const results = await MonitorResult.query()
      .whereIn(
        'monitorId',
        monitors.map((monitor) => monitor.id)
      )
      .where('startedAt', '>=', cutoff.toSQL()!)
      .orderBy('startedAt', 'desc')

    const monitorById = new Map(monitors.map((monitor) => [monitor.id, monitor]))
    const groups = new Map<string, DnsServerPerformance & { latencies: number[] }>()

    for (const result of results) {
      const monitor = monitorById.get(result.monitorId)
      if (!monitor) continue

      const config = (monitor.configuration || {}) as Record<string, unknown>
      const data = (result.data || {}) as Record<string, unknown>

      // O servidor gravado no resultado vale mais que o da configuração: reflete
      // o que foi realmente consultado quando a checagem rodou.
      const server =
        (data.server as string | undefined) ||
        (config.dohUrl as string | undefined) ||
        (config.dnsServer as string | undefined) ||
        'Resolvedor do sistema'
      const protocol = ((data.protocol as DnsProtocol | undefined) ||
        (config.protocol as DnsProtocol | undefined) ||
        (config.dnsServer ? 'udp' : 'system')) as DnsProtocol

      const key = `${server}|${protocol}`
      let group = groups.get(key)

      if (!group) {
        group = {
          server,
          label: server,
          protocol,
          avgLookupTimeMs: null,
          minLookupTimeMs: null,
          maxLookupTimeMs: null,
          successRate: 0,
          totalChecks: 0,
          monitorIds: [],
          lastCheckedAt: null,
          latencies: [],
        }
        groups.set(key, group)
      }

      group.totalChecks += 1
      if (!group.monitorIds.includes(monitor.id)) group.monitorIds.push(monitor.id)

      const finishedAt = result.finishedAt?.toISO() ?? null
      if (finishedAt && (!group.lastCheckedAt || finishedAt > group.lastCheckedAt)) {
        group.lastCheckedAt = finishedAt
      }

      const lookupTime =
        typeof data.avgLookupTimeMs === 'number' ? data.avgLookupTimeMs : result.latencyMs
      if (result.status === 'up' && typeof lookupTime === 'number') {
        group.latencies.push(lookupTime)
      }
    }

    const ranking: DnsServerPerformance[] = [...groups.values()].map((group) => {
      const { latencies, ...rest } = group
      const successCount = latencies.length

      return {
        ...rest,
        avgLookupTimeMs: successCount
          ? Number((latencies.reduce((total, value) => total + value, 0) / successCount).toFixed(2))
          : null,
        minLookupTimeMs: successCount ? Number(Math.min(...latencies).toFixed(2)) : null,
        maxLookupTimeMs: successCount ? Number(Math.max(...latencies).toFixed(2)) : null,
        successRate: rest.totalChecks
          ? Number(((successCount / rest.totalChecks) * 100).toFixed(1))
          : 0,
      }
    })

    return response.ok({
      windowHours: hours,
      monitorCount: monitors.length,
      ranking: sortByLatency(ranking),
    })
  }
}
