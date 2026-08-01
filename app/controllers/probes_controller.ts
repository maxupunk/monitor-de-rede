import type { HttpContext } from '@adonisjs/core/http'
import Probe from '#models/probe'

export default class ProbesController {
  async index({ response }: HttpContext) {
    const probes = await Probe.all()
    return response.ok(probes)
  }

  async store({ request, response }: HttpContext) {
    const data = request.only(['siteId', 'name', 'tokenHash', 'status', 'version', 'configuration'])
    const probe = await Probe.create(data)
    return response.created(probe)
  }

  async show({ params, response }: HttpContext) {
    const probe = await Probe.findOrFail(params.id)
    return response.ok(probe)
  }

  async update({ params, request, response }: HttpContext) {
    const probe = await Probe.findOrFail(params.id)
    const data = request.only(['siteId', 'name', 'status', 'version', 'configuration'])
    probe.merge(data)
    await probe.save()
    return response.ok(probe)
  }

  async destroy({ params, response }: HttpContext) {
    const probe = await Probe.findOrFail(params.id)
    await probe.delete()
    return response.noContent()
  }

  async revoke({ params, response }: HttpContext) {
    const probe = await Probe.findOrFail(params.id)
    probe.status = 'revoked'
    await probe.save()
    return response.ok(probe)
  }

  async test({ params, response }: HttpContext) {
    return response.ok({ message: `Teste de conectividade enviado para o probe ID ${params.id}` })
  }
}
