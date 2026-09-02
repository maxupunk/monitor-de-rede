import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { DnsProtocol } from '@/utils/monitorTypes'

/** Origem do ranking exibido: histórico dos monitores ou comparação ao vivo */
export type DnsRankingSource = 'history' | 'benchmark'

export interface DnsRankingEntry {
  server: string
  label: string
  protocol: DnsProtocol
  avgLookupTimeMs: number | null
  minLookupTimeMs: number | null
  maxLookupTimeMs: number | null
  medianLookupTimeMs?: number | null
  /** Percentual (histórico) ou fração de 0 a 1 (comparação ao vivo) */
  successRate: number
  totalChecks?: number
  totalQueries?: number
  failedQueries?: number
  monitorIds?: number[]
  lastCheckedAt?: string | null
  error?: string | null
}

export interface DnsHistoryPoint {
  timestamp: string
  latencyMs: number | null
  status: string
}

export interface DnsSeriesItem {
  server: string
  label: string
  protocol: DnsProtocol
  monitorIds?: number[]
  points: DnsHistoryPoint[]
}

interface PerformanceResponse {
  windowHours: number
  monitorCount: number
  ranking: DnsRankingEntry[]
  series?: DnsSeriesItem[]
}

interface BenchmarkResponse {
  hostnames: string[]
  recordType: string
  measuredAt: string
  ranking: DnsRankingEntry[]
}

export interface DnsBenchmarkRequest {
  servers?: Array<{ server: string; label?: string; protocol?: DnsProtocol }>
  hostnames?: string[]
  timeoutMs?: number
  rounds?: number
}

export interface DnsBatchProvisionServer {
  server: string
  name?: string
  protocol?: DnsProtocol
  dohUrl?: string
}

export interface DnsBatchProvisionRequest {
  servers: DnsBatchProvisionServer[]
  domain?: string
  domains?: string[]
  recordType?: string
  intervalSeconds?: number
  executeNow?: boolean
  includePing?: boolean
}

export interface DnsBatchProvisionResponse {
  createdCount: number
  alreadyMonitoredCount: number
  totalRequested: number
  monitors: any[]
}

export const useDnsPerformanceStore = defineStore('dnsPerformance', () => {
  const ranking = ref<DnsRankingEntry[]>([])
  const series = ref<DnsSeriesItem[]>([])
  const source = ref<DnsRankingSource>('history')
  const windowHours = ref(24)
  const monitorCount = ref(0)
  const measuredAt = ref<string | null>(null)
  const benchmarkHostnames = ref<string[]>([])
  const loading = ref(false)
  const benchmarking = ref(false)
  const provisioning = ref(false)
  const error = ref<string | null>(null)

  /** Servidor mais rápido do ranking atual */
  const fastest = computed(() => ranking.value.find((entry) => entry.avgLookupTimeMs !== null))

  /** Referência para dimensionar as barras comparativas */
  const slowestLatency = computed(() => {
    const values = ranking.value
      .map((entry) => entry.avgLookupTimeMs)
      .filter((value): value is number => value !== null)
    return values.length > 0 ? Math.max(...values) : 0
  })

  async function fetchPerformance(hours = 24): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      const data = await apiService.get<PerformanceResponse>(`/dns/performance?hours=${hours}`)
      ranking.value = data.ranking || []
      series.value = data.series || []
      windowHours.value = data.windowHours
      monitorCount.value = data.monitorCount
      source.value = 'history'
      measuredAt.value = null
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar o ranking de DNS'
      return false
    } finally {
      loading.value = false
    }
  }

  async function runBenchmark(payload: DnsBenchmarkRequest = {}): Promise<boolean> {
    benchmarking.value = true
    error.value = null
    try {
      const data = await apiService.post<BenchmarkResponse>('/dns/benchmark', payload)
      ranking.value = data.ranking || []
      benchmarkHostnames.value = data.hostnames || []
      measuredAt.value = data.measuredAt
      source.value = 'benchmark'
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao comparar os servidores DNS'
      return false
    } finally {
      benchmarking.value = false
    }
  }

  async function provisionMonitors(
    payload: DnsBatchProvisionRequest
  ): Promise<DnsBatchProvisionResponse | null> {
    provisioning.value = true
    error.value = null
    try {
      const data = await apiService.post<DnsBatchProvisionResponse>('/dns/provision', payload)
      return data
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao provisionar monitores DNS'
      return null
    } finally {
      provisioning.value = false
    }
  }

  async function provisionPingMonitors(
    servers?: DnsBatchProvisionServer[],
    intervalSeconds?: number
  ): Promise<DnsBatchProvisionResponse | null> {
    provisioning.value = true
    error.value = null
    try {
      const data = await apiService.post<DnsBatchProvisionResponse>('/dns/provision-ping', {
        servers,
        intervalSeconds,
      })
      return data
    } catch (err: unknown) {
      error.value =
        err instanceof Error ? err.message : 'Erro ao provisionar ping para servidores DNS'
      return null
    } finally {
      provisioning.value = false
    }
  }

  return {
    ranking,
    series,
    source,
    windowHours,
    monitorCount,
    measuredAt,
    benchmarkHostnames,
    loading,
    benchmarking,
    provisioning,
    error,
    fastest,
    slowestLatency,
    fetchPerformance,
    runBenchmark,
    provisionMonitors,
    provisionPingMonitors,
  }
})
