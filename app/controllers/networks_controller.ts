import type { HttpContext } from '@adonisjs/core/http'
import Network from '#models/network'

export default class NetworksController {
  async index({ response }: HttpContext) {
    const networks = await Network.all()
    return response.ok(networks)
  }

  async store({ request, response }: HttpContext) {
    const data = request.only([
      'siteId',
      'probeId',
      'name',
      'cidr',
      'gateway',
      'vlan',
      'dnsServers',
      'scanEnabled',
      'scanInterval',
      'active',
    ])
    const network = await Network.create(data)
    return response.created(network)
  }

  async show({ params, response }: HttpContext) {
    const network = await Network.findOrFail(params.id)
    return response.ok(network)
  }

  async update({ params, request, response }: HttpContext) {
    const network = await Network.findOrFail(params.id)
    const data = request.only([
      'siteId',
      'probeId',
      'name',
      'cidr',
      'gateway',
      'vlan',
      'dnsServers',
      'scanEnabled',
      'scanInterval',
      'active',
    ])
    network.merge(data)
    await network.save()
    return response.ok(network)
  }

  async destroy({ params, response }: HttpContext) {
    const network = await Network.findOrFail(params.id)
    await network.delete()
    return response.noContent()
  }

  async scan({ params, response }: HttpContext) {
    return response.ok({ message: `Varredura iniciada para a rede ID ${params.id}` })
  }
}
