import type { HttpContext } from '@adonisjs/core/http'
import Network from '#models/network'
import { DiscoveryQueue } from '#modules/discovery/discovery_queue'
import { isScannableCidr, parseCidrRange } from '#modules/discovery/cidr_range'
import { errorMessage } from '#modules/shared/errors'

export default class NetworksController {
  private discoveryQueue = new DiscoveryQueue()

  async index({ response }: HttpContext) {
    const networks = await Network.query().preload('site')

    // A UI mostra o tamanho da faixa e avisa quando ela será truncada — sem
    // isso o operador só descobre o limite olhando os resultados.
    return response.ok(
      networks.map((network) => {
        const json = network.serialize()
        json.scannable = isScannableCidr(network.cidr)
        json.usableHosts = json.scannable ? parseCidrRange(network.cidr).usableHosts : 0
        return json
      })
    )
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

  /**
   * Enfileira a varredura da faixa CIDR da rede.
   *
   * O processo HTTP **não** varre: pingar centenas de endereços dentro de uma
   * request seguraria o servidor por dezenas de segundos e contraria a §4.1 da
   * arquitetura. Aqui só nasce a `DiscoveryRun` pendente; o `scheduler:run` a
   * executa no ciclo seguinte.
   */
  async scan({ params, response }: HttpContext) {
    const network = await Network.findOrFail(params.id)

    if (!isScannableCidr(network.cidr)) {
      return response.unprocessableEntity({
        message: `A rede "${network.name}" não tem uma faixa CIDR válida para varredura.`,
        cidr: network.cidr,
      })
    }

    try {
      const { run, alreadyQueued } = await this.discoveryQueue.enqueueNetworkScan(network)
      const range = parseCidrRange(network.cidr)

      return response.accepted({
        message: alreadyQueued
          ? `Já existe uma varredura em andamento para "${network.name}".`
          : `Varredura da faixa ${network.cidr} enfileirada. Os equipamentos encontrados aparecem em Descoberta.`,
        alreadyQueued,
        run,
        usableHosts: range.usableHosts,
        truncated: range.truncated,
      })
    } catch (err: unknown) {
      return response.unprocessableEntity({ message: errorMessage(err) })
    }
  }
}
