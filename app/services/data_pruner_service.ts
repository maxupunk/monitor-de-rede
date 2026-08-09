import { DateTime } from 'luxon'
import EventOutbox from '#models/event_outbox'
import MonitorResult from '#models/monitor_result'
import Metric from '#models/metric'
import DiscoveryRun from '#models/discovery_run'
import env from '#start/env'

export interface PruneStats {
  outboxDeleted: number
  resultsDeleted: number
  metricsDeleted: number
  discoveryDeleted: number
}

export class DataPrunerService {
  /**
   * Executa a purga periódica de dados antigos no banco para manter o footprint
   * de memória e armazenamento reduzido.
   */
  async pruneAll(): Promise<PruneStats> {
    const monitorResultsDays = env.get('RETENTION_MONITOR_RESULTS_DAYS') || 14
    const metricsDays = env.get('RETENTION_METRICS_DAYS') || 30
    const discoveryDays = env.get('RETENTION_DISCOVERY_DAYS') || 7

    const now = DateTime.now()

    const [outboxDeleted, resultsDeleted, metricsDeleted, discoveryDeleted] = await Promise.all([
      // Outbox de eventos com mais de 30 minutos (garante limpeza mesmo sem SSE ativo)
      EventOutbox.query()
        .where('createdAt', '<', now.minus({ minutes: 30 }).toJSDate())
        .delete()
        .then((res) => (Array.isArray(res) ? res.length : Number(res) || 0)),

      // Resultados de monitores com mais de X dias
      MonitorResult.query()
        .where('createdAt', '<', now.minus({ days: monitorResultsDays }).toJSDate())
        .delete()
        .then((res) => (Array.isArray(res) ? res.length : Number(res) || 0)),

      // Métricas históricas com mais de X dias
      Metric.query()
        .where('createdAt', '<', now.minus({ days: metricsDays }).toJSDate())
        .delete()
        .then((res) => (Array.isArray(res) ? res.length : Number(res) || 0)),

      // Scans de descoberta de rede antigos com mais de X dias
      DiscoveryRun.query()
        .where('createdAt', '<', now.minus({ days: discoveryDays }).toJSDate())
        .delete()
        .then((res) => (Array.isArray(res) ? res.length : Number(res) || 0)),
    ])

    return {
      outboxDeleted,
      resultsDeleted,
      metricsDeleted,
      discoveryDeleted,
    }
  }
}
