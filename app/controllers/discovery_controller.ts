import type { HttpContext } from '@adonisjs/core/http'
import DiscoveryRun from '#models/discovery_run'
import DiscoveryResult from '#models/discovery_result'
import Device from '#models/device'
import Monitor from '#models/monitor'

export default class DiscoveryController {
  async runs({ response }: HttpContext) {
    const runs = await DiscoveryRun.query()
      .preload('network')
      .preload('probe')
      .orderBy('id', 'desc')
    return response.ok(runs)
  }

  async runDetails({ params, response }: HttpContext) {
    const run = await DiscoveryRun.query().where('id', params.id).preload('results').firstOrFail()
    return response.ok(run)
  }

  async results({ request, response }: HttpContext) {
    const status = request.input('status', 'pending')
    const results = await DiscoveryResult.query().where('status', status).orderBy('id', 'desc')
    return response.ok(results)
  }

  async accept({ params, response }: HttpContext) {
    const result = await DiscoveryResult.findOrFail(params.id)

    const run = await DiscoveryRun.findOrFail(result.discoveryRunId)

    const device = await Device.create({
      siteId: run.networkId,
      networkId: run.networkId,
      name: result.hostname || result.ipAddress,
      type: result.deviceType || 'unknown',
      vendor: result.vendor || null,
      status: 'online',
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
