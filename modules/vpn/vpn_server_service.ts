import { DateTime } from 'luxon'
import VpnServer from '#models/vpn_server'
import VpnPeer from '#models/vpn_peer'
import Network from '#models/network'
import Site from '#models/site'
import { generateKeyPair } from './key_generator.js'
import { firstUsableAddress, parseCidr } from './cidr.js'
import { ConfigWriter } from './config_writer.js'
import { PeerStatusService } from './peer_status.js'

/**
 * Ciclo de vida do servidor WireGuard: configuração, chaves e sincronização do
 * `wg0.conf`. A v1 opera com um único servidor (uma interface).
 */

export const DEFAULT_VPN_CIDR = '10.8.0.0/24'
export const DEFAULT_LISTEN_PORT = 51820
export const DEFAULT_MTU = 1420
export const DEFAULT_INTERFACE = 'wg0'

export interface VpnServerPayload {
  cidr?: string
  siteId?: number | null
  networkId?: number | null
  listenPort?: number
  publicEndpoint?: string | null
  mtu?: number
  dnsServers?: string | null
  allowPeerToPeer?: boolean
  active?: boolean
}

export interface VpnServerState {
  server: VpnServer | null
  cidr: string | null
  serverAddress: string | null
  peersTotal: number
  peersConnected: number
  bytesRx: number
  bytesTx: number
}

export class VpnServerService {
  constructor(
    private configWriter = new ConfigWriter(),
    private peerStatusService = new PeerStatusService()
  ) {}

  /** Servidor VPN configurado (v1: instância única). */
  async find(): Promise<VpnServer | null> {
    return VpnServer.query().preload('network').orderBy('id', 'asc').first()
  }

  async findOrFail(): Promise<VpnServer> {
    const server = await this.find()
    if (!server) {
      throw new Error('Servidor VPN ainda não foi configurado')
    }
    return server
  }

  /** Endereço do NetMonitor dentro do túnel (primeiro IP utilizável do CIDR). */
  serverAddress(server: VpnServer): string {
    return firstUsableAddress(server.network.cidr)
  }

  /**
   * Traz o `wg show dump` publicado pelo container para dentro do banco.
   *
   * Precisa acontecer antes de qualquer leitura que exiba status ao operador:
   * o scheduler sincroniza em background, mas quem acabou de abrir a tela não
   * pode depender do próximo ciclo dele para ver o túnel que subiu agora.
   * Falha na leitura do arquivo não derruba a resposta — os dados persistidos
   * seguem válidos, só ficam um ciclo atrasados.
   */
  async syncTelemetry(): Promise<void> {
    const server = await this.find()
    if (!server) return

    await this.peerStatusService.syncPeers(server.interfaceName, server.id).catch(() => 0)
  }

  /**
   * Cria a rede da VPN quando ainda não existe. `networks.site_id` é NOT NULL,
   * então é preciso um Site — usamos o informado ou o primeiro cadastrado.
   */
  private async resolveNetwork(payload: VpnServerPayload): Promise<Network> {
    const cidr = payload.cidr || DEFAULT_VPN_CIDR
    parseCidr(cidr) // valida cedo, antes de qualquer escrita

    if (payload.networkId) {
      const existing = await Network.findOrFail(payload.networkId)
      existing.cidr = cidr
      await existing.save()
      return existing
    }

    let siteId = payload.siteId ?? null
    if (!siteId) {
      const site =
        (await Site.query().orderBy('id', 'asc').first()) ??
        (await Site.create({ name: 'Matriz', active: true }))
      siteId = site.id
    }

    return Network.create({
      siteId,
      name: 'VPN WireGuard',
      cidr,
      gateway: firstUsableAddress(cidr),
      scanEnabled: false,
      scanInterval: 3600,
      active: true,
    })
  }

  /**
   * Cria (com par de chaves novo) ou atualiza o servidor e reescreve o
   * `wg0.conf` — o watcher aplica com `syncconf`, sem derrubar túneis.
   */
  async createOrUpdate(payload: VpnServerPayload): Promise<VpnServer> {
    let server = await this.find()

    if (!server) {
      const network = await this.resolveNetwork(payload)
      const keyPair = generateKeyPair()

      server = await VpnServer.create({
        networkId: network.id,
        interfaceName: DEFAULT_INTERFACE,
        listenPort: payload.listenPort ?? DEFAULT_LISTEN_PORT,
        publicEndpoint: payload.publicEndpoint ?? null,
        publicKey: keyPair.publicKey,
        privateKey: keyPair.privateKey,
        allowPeerToPeer: payload.allowPeerToPeer ?? false,
        mtu: payload.mtu ?? DEFAULT_MTU,
        dnsServers: payload.dnsServers ?? null,
        active: payload.active ?? true,
      })
    } else {
      if (payload.cidr && payload.cidr !== server.network?.cidr) {
        const network = await Network.findOrFail(server.networkId)
        parseCidr(payload.cidr)
        network.cidr = payload.cidr
        network.gateway = firstUsableAddress(payload.cidr)
        await network.save()
      }

      server.merge({
        listenPort: payload.listenPort ?? server.listenPort,
        publicEndpoint: payload.publicEndpoint ?? server.publicEndpoint,
        mtu: payload.mtu ?? server.mtu,
        dnsServers: payload.dnsServers ?? server.dnsServers,
        allowPeerToPeer: payload.allowPeerToPeer ?? server.allowPeerToPeer,
        active: payload.active ?? server.active,
      })
      await server.save()
    }

    await server.load('network')
    await this.applyConfiguration(server)

    return server
  }

  /** Reescreve o arquivo de configuração com todos os peers habilitados. */
  async applyConfiguration(server: VpnServer): Promise<string> {
    if (!server.network) {
      await server.load('network')
    }

    const peers = await VpnPeer.query()
      .where('vpnServerId', server.id)
      .where('enabled', true)
      .preload('device')

    const contents = await this.configWriter.writeServerConfig(
      {
        interfaceName: server.interfaceName,
        address: this.serverAddress(server),
        cidr: server.network.cidr,
        listenPort: server.listenPort,
        privateKey: server.privateKey,
        mtu: server.mtu,
        allowPeerToPeer: server.allowPeerToPeer,
      },
      peers.map((peer) => ({
        name: peer.device?.name ?? `peer-${peer.id}`,
        publicKey: peer.publicKey,
        presharedKey: peer.presharedKey,
        ipAddress: peer.device?.ipAddress ?? '',
        enabled: peer.enabled,
      }))
    )

    server.lastSyncedAt = DateTime.now()
    await server.save()

    return contents
  }

  /** Estado agregado exibido no painel (§4.1). */
  async getState(): Promise<VpnServerState> {
    const server = await this.find()
    if (!server) {
      return {
        server: null,
        cidr: null,
        serverAddress: null,
        peersTotal: 0,
        peersConnected: 0,
        bytesRx: 0,
        bytesTx: 0,
      }
    }

    await this.syncTelemetry()

    const peers = await VpnPeer.query().where('vpnServerId', server.id)

    return {
      server,
      cidr: server.network.cidr,
      serverAddress: this.serverAddress(server),
      peersTotal: peers.length,
      peersConnected: peers.filter((peer) => peer.connectionStatus === 'connected').length,
      bytesRx: peers.reduce((total, peer) => total + Number(peer.bytesRx || 0), 0),
      bytesTx: peers.reduce((total, peer) => total + Number(peer.bytesTx || 0), 0),
    }
  }
}
