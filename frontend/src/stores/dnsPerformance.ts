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

interface PerformanceResponse {
  windowHours: number
  monitorCount: number
  ranking: DnsRankingEntry[]
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

export const useDnsPerformanceStore = defineStore('dnsPerformance', () => {
  const ranking = ref<DnsRankingEntry[]>([])
  const source = ref<DnsRankingSource>('history')
  const windowHours = ref(24)
  const monitorCount = ref(0)
  const measuredAt = ref<string | null>(null)
  const benchmarkHostnames = ref<string[]>([])
  const loading = ref(false)
  const benchmarking = ref(false)
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

  return {
    ranking,
    source,
    windowHours,
    monitorCount,
    measuredAt,
    benchmarkHostnames,
    loading,
    benchmarking,
    error,
    fastest,
    slowestLatency,
    fetchPerformance,
    runBenchmark,
  }
})
