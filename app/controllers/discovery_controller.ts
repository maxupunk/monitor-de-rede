import type { HttpContext } from '@adonisjs/core/http'
import { DateTime } from 'luxon'
import DiscoveryRun from '#models/discovery_run'
import DiscoveryResult from '#models/discovery_result'
import Device from '#models/device'
import Monitor from '#models/monitor'
import Network from '#models/network'

export default class DiscoveryController {
  async runs({ request, response }: HttpContext) {
    const query = DiscoveryRun.query()
      .preload('network')
      .preload('probe')
      // A coluna "Dispositivos encontrados" do histórico vinha vazia: o total
      // não é campo da run, é a contagem dos resultados que ela gerou.
      .withCount('results')
      .orderBy('id', 'desc')

    const pageParam = request.input('page')
    if (pageParam) {
      const page = Number(pageParam) || 1
      const limit = Math.min(Number(request.input('limit', 20)), 100)
      const paginated = await query.paginate(page, limit)

      return response.ok({
        data: paginated.all().map((run) => this.serializeRun(run)),
        meta: paginated.toJSON().meta,
      })
    }

    const runs = await query
    return response.ok(runs.map((run) => this.serializeRun(run)))
  }

  private serializeRun(run: DiscoveryRun) {
    const json = run.serialize()
    json.devicesFound = Number(run.$extras.results_count ?? 0)
    json.cidr = (run.configuration?.cidr as string | undefined) ?? run.network?.cidr ?? null
    json.networkName = run.network?.name ?? null
    return json
  }

  async runDetails({ params, response }: HttpContext) {
    const run = await DiscoveryRun.query().where('id', params.id).preload('results').firstOrFail()
    return response.ok(run)
  }

  /**
   * GET /api/discovery/results/latest
   *
   * Retorna os resultados da varredura mais recente, ignorando IPs que já
   * existem na tabela devices. O discovery_result funciona apenas como cache
   * do último scan; a fonte da verdade para "já adicionado" é a tabela
   * devices.
   */
  async latestResults({ response }: HttpContext) {
    const latestRun = await DiscoveryRun.query().orderBy('id', 'desc').first()

    if (!latestRun) {
      return response.ok({
        data: [],
        meta: { currentPage: 1, lastPage: 1, total: 0 },
      })
    }

    const existingIps = await Device.query().select('ip_address')
    const existingIpSet = new Set(existingIps.map((d) => d.ipAddress).filter(Boolean))

    const results = await DiscoveryResult.query()
      .where('discoveryRunId', latestRun.id)
      .preload('discoveryRun', (runQuery) => runQuery.preload('network', (n) => n.preload('site')))
      .orderBy('id', 'desc')

    const filtered = results.filter((r) => !existingIpSet.has(r.ipAddress))

    return response.ok({
      data: filtered,
      meta: { currentPage: 1, lastPage: 1, total: filtered.length },
    })
  }

  /**
   * POST /api/discovery/results/:id/accept
   *
   * Cria um device a partir do resultado e remove o cache do discovery.
   */
  async accept({ params, response }: HttpContext) {
    const result = await DiscoveryResult.findOrFail(params.id)
    const run = await DiscoveryRun.findOrFail(result.discoveryRunId)
    const network = await Network.find(run.networkId)

    const device = await Device.create({
      siteId: network?.siteId ?? null,
      networkId: network?.id ?? null,
      ipAddress: result.ipAddress,
      name: result.mdnsName || result.hostname || result.ipAddress,
      type: result.deviceType || 'unknown',
      vendor: result.vendor || null,
      status: 'online',
      isMonitored: true,
      lastSeenAt: result.lastSeenAt,
    })

    await Monitor.create({
      deviceId: device.id,
      probeId: run.probeId,
      type: 'ping',
      name: `Ping ${device.name}`,
      configuration: { host: result.ipAddress },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      enabled: true,
      status: 'unknown',
    })

    await result.delete()

    return response.ok({
      message: `Dispositivo ${device.name} criado com sucesso a partir da descoberta`,
      device,
    })
  }

  /**
   * POST /api/discovery/results/:id/merge
   *
   * Vincula o resultado a um device existente e remove o cache do discovery.
   */
  async merge({ params, request, response }: HttpContext) {
    const result = await DiscoveryResult.findOrFail(params.id)
    const targetDeviceId = request.input('targetDeviceId')

    const device = await Device.findOrFail(targetDeviceId)
    await result.delete()

    return response.ok({
      message: `Resultado de descoberta mesclado com o dispositivo #${device.id}`,
      device,
    })
  }

  /**
   * DELETE /api/discovery/cleanup?olderThanDays=7
   * Remove runs (e seus resultados em cascata) mais antigas que o prazo.
   * Útil para evitar acúmulo de legados na tabela de descoberta.
   */
  async cleanup({ request, response }: HttpContext) {
    const olderThanDays = Math.max(1, Number(request.input('olderThanDays', 7)))
    const cutoff = DateTime.now().minus({ days: olderThanDays })

    const runsToDelete = await DiscoveryRun.query().where('createdAt', '<=', cutoff.toSQL()).select('id')
    const runIds = runsToDelete.map((run) => run.id)

    if (runIds.length === 0) {
      return response.ok({ removedRuns: 0, removedResults: 0 })
    }

    // A FK já tem ON DELETE CASCADE; deletar as runs limpa os resultados.
    await DiscoveryRun.query().whereIn('id', runIds).delete()

    return response.ok({
      removedRuns: runIds.length,
      message: `${runIds.length} varredura(s) antiga(s) removida(s).`,
    })
  }
}
