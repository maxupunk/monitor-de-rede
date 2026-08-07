import type { HttpContext } from '@adonisjs/core/http'
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

  async results({ request, response }: HttpContext) {
    const status = request.input('status', 'pending')
    const query = DiscoveryResult.query()
      .where('status', status)
      .preload('discoveryRun', (runQuery) => runQuery.preload('network'))
      .orderBy('id', 'desc')

    // Paginação sob demanda: quem passa `page` recebe o envelope do Lucid, os
    // demais continuam recebendo o array cru (o dashboard e o store legado).
    const pageParam = request.input('page')
    if (pageParam) {
      const page = Number(pageParam) || 1
      const limit = Math.min(Number(request.input('limit', 20)), 100)
      return response.ok(await query.paginate(page, limit))
    }

    return response.ok(await query)
  }

  async accept({ params, response }: HttpContext) {
    const result = await DiscoveryResult.findOrFail(params.id)
    const run = await DiscoveryRun.findOrFail(result.discoveryRunId)

    // O site vem da rede varrida, não do id da rede: usar `networkId` como
    // `siteId` fazia o equipamento nascer vinculado ao site errado (ou a um
    // site inexistente), quebrando o escopo das regras de alerta por site.
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

    result.status = 'accepted'
    await result.save()

    return response.ok({
      message: `Dispositivo ${device.name} criado com sucesso a partir da descoberta`,
      device,
      result,
    })
  }

  async ignore({ params, response }: HttpContext) {
    const result = await DiscoveryResult.findOrFail(params.id)
    result.status = 'ignored'
    await result.save()
    return response.ok(result)
  }

  async merge({ params, request, response }: HttpContext) {
    const result = await DiscoveryResult.findOrFail(params.id)
    const targetDeviceId = request.input('targetDeviceId')

    const device = await Device.findOrFail(targetDeviceId)
    result.status = 'merged'
    await result.save()

    return response.ok({
      message: `Resultado de descoberta mesclado com o dispositivo #${device.id}`,
      device,
      result,
    })
  }
}
