import type { HttpContext } from '@adonisjs/core/http'
import { VpnServerService } from '#modules/vpn/vpn_server_service'
import { PreflightService } from '#modules/vpn/preflight'
import { profileRegistry } from '#modules/vpn/profiles/profile_registry'
import { PERSISTENT_KEEPALIVE_SECONDS } from '#modules/vpn/profiles/profile_contract'
import { errorMessage } from '#modules/shared/errors'

/**
 * Painel do servidor WireGuard: configuração, estado dos túneis, auto-detecção
 * de endpoint e teste de pré-voo (CGNAT).
 */
export default class VpnServersController {
  private serverService = new VpnServerService()
  private preflightService = new PreflightService()

  /** GET /api/vpn/server */
  async show({ response }: HttpContext) {
    const state = await this.serverService.getState()

    return response.ok({
      configured: state.server !== null,
      server: state.server?.serialize() ?? null,
      cidr: state.cidr,
      serverAddress: state.serverAddress,
      peersTotal: state.peersTotal,
      peersConnected: state.peersConnected,
      bytesRx: state.bytesRx,
      bytesTx: state.bytesTx,
      persistentKeepalive: PERSISTENT_KEEPALIVE_SECONDS,
      profiles: profileRegistry.list(),
    })
  }

  /** PUT /api/vpn/server — salva e aplica via arquivo compartilhado + syncconf. */
  async update({ request, response }: HttpContext) {
    const payload = request.only([
      'cidr',
      'siteId',
      'networkId',
      'listenPort',
      'publicEndpoint',
      'mtu',
      'dnsServers',
      'allowPeerToPeer',
      'active',
    ])

    try {
      const server = await this.serverService.createOrUpdate(payload)
      await server.load('network')

      return response.ok({
        message: 'Configuração aplicada sem derrubar os túneis ativos',
        server: server.serialize(),
        cidr: server.network.cidr,
        serverAddress: this.serverService.serverAddress(server),
      })
    } catch (error: unknown) {
      const message = errorMessage(error)
      return response.badRequest({ message })
    }
  }

  /** POST /api/vpn/server/preflight */
  async preflight({ request, response }: HttpContext) {
    const server = await this.serverService.find()
    const endpoint = request.input('publicEndpoint', server?.publicEndpoint ?? null)
    const port = Number(request.input('listenPort', server?.listenPort ?? 51820))

    const result = await this.preflightService.run(endpoint, port)
    return response.ok(result)
  }

  /** POST /api/vpn/server/detect-endpoint */
  async detectEndpoint({ response }: HttpContext) {
    const publicIp = await this.preflightService.detectPublicIp()

    if (!publicIp) {
      return response.ok({
        detected: false,
        publicEndpoint: null,
        message: 'Não foi possível detectar o IP público a partir deste servidor.',
      })
    }

    return response.ok({
      detected: true,
      publicEndpoint: publicIp,
      message: `Endereço público detectado: ${publicIp}`,
    })
  }
}
