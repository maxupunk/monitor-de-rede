import db from '@adonisjs/lucid/services/db'
import Device from '#models/device'
import Monitor from '#models/monitor'
import VpnPeer, { type VpnDeviceProfile } from '#models/vpn_peer'
import type VpnServer from '#models/vpn_server'
import { generateKeyPair, generatePresharedKey } from './key_generator.js'
import { IpAllocator } from './ip_allocator.js'
import { VpnMonitorProvisioner } from './monitor_provisioner.js'
import { VpnServerService } from './vpn_server_service.js'
import { clientKeyStore } from './secret_store.js'
import { profileRegistry } from './profiles/profile_registry.js'
import { computePeerHints } from './peer_hints.js'
import {
  PERSISTENT_KEEPALIVE_SECONDS,
  PRIVATE_KEY_UNAVAILABLE,
  type GeneratedArtifact,
  type PeerConfigContext,
} from './profiles/profile_contract.js'

import { ResourceCleanupService } from '#services/resource_cleanup_service'

/**
 * Regras de negócio dos peers: criação com provisionamento completo, rotação de
 * chaves, revogação e geração dos artefatos por perfil.
 */

/** Reexportado por compatibilidade — a definição vive junto do contrato de perfis. */
export { PRIVATE_KEY_UNAVAILABLE }

export interface CreatePeerPayload {
  name: string
  profile: VpnDeviceProfile
  ipAddress?: string | null
  siteId?: number | null
  snmpEnabled?: boolean
  snmpCommunity?: string | null
  snmpVersion?: string | null
  description?: string | null
}

export interface PeerListItem {
  peer: VpnPeer
  /** Túnel ativo, mas o ping falha: provável firewall bloqueando na interface WG. */
  needsFirewallHint: boolean
  /**
   * Túnel ativo e ping falhando, mas o monitor não roda no `vpn-probe`: o ICMP
   * sai da máquina da API, que não tem rota para dentro do túnel. O pacote nem
   * chega ao equipamento — acusar o firewall dele seria diagnóstico falso.
   */
  pingOutsideTunnel: boolean
  /** Monitor de ping provisionado automaticamente para o peer (§4.7) — usado para navegar ao histórico de conectividade. */
  pingMonitorId: number | null
}

export class VpnPeerService {
  private cleanupService = new ResourceCleanupService()

  constructor(
    private serverService = new VpnServerService(),
    private ipAllocator = new IpAllocator(),
    private monitorProvisioner = new VpnMonitorProvisioner()
  ) {}

  private assertProfile(profile: string): VpnDeviceProfile {
    if (!profileRegistry.has(profile)) {
      throw new Error(`Perfil de equipamento não suportado: ${profile}`)
    }
    return profile
  }

  /** Monta o contexto consumido pelos geradores por perfil. */
  private buildContext(
    server: VpnServer,
    peer: VpnPeer,
    device: Device,
    clientPrivateKey: string | null
  ): PeerConfigContext {
    return {
      peerName: device.name,
      peerIpAddress: device.ipAddress ?? '',
      vpnCidr: server.network.cidr,
      serverVpnAddress: this.serverService.serverAddress(server),
      clientPrivateKey: clientPrivateKey ?? PRIVATE_KEY_UNAVAILABLE,
      serverPublicKey: server.publicKey,
      presharedKey: peer.presharedKey,
      endpointHost: server.publicEndpoint || 'ENDERECO-PUBLICO-NAO-CONFIGURADO',
      endpointPort: server.listenPort,
      mtu: server.mtu,
      dnsServers: server.dnsServers,
      snmpEnabled: device.snmpEnabled,
      snmpCommunity: device.snmpCommunity,
    }
  }

  private async loadPeer(peerId: number): Promise<{ peer: VpnPeer; server: VpnServer }> {
    const peer = await VpnPeer.query()
      .where('id', peerId)
      .preload('device')
      .preload('vpnServer', (query) => query.preload('network'))
      .firstOrFail()

    return { peer, server: peer.vpnServer }
  }

  async list(): Promise<PeerListItem[]> {
    // Sem isto a lista mostra o que o scheduler gravou no ciclo anterior: o
    // operador que acabou de conectar o túnel precisaria recarregar a tela
    // até o background alcançá-lo.
    await this.serverService.syncTelemetry()

    const peers = await VpnPeer.query().preload('device').orderBy('id', 'asc')
    if (peers.length === 0) return []

    const pingMonitors = await Monitor.query()
      .whereIn(
        'deviceId',
        peers.map((peer) => peer.deviceId)
      )
      .where('type', 'ping')

    return peers.map((peer) => {
      const monitor = pingMonitors.find((item) => item.deviceId === peer.deviceId)
      return { peer, ...computePeerHints(peer, monitor) }
    })
  }

  /**
   * Cria dispositivo, peer e monitores em uma única transação e só então
   * reescreve o `wg0.conf` (§4.7). A chave privada do cliente fica apenas em
   * memória, entregue uma única vez.
   */
  async create(
    payload: CreatePeerPayload
  ): Promise<{ peer: VpnPeer; artifact: GeneratedArtifact }> {
    const profile = this.assertProfile(payload.profile)
    const server = await this.serverService.findOrFail()
    const cidr = server.network.cidr

    if (payload.ipAddress) {
      await this.ipAllocator.assertAvailable(server.networkId, cidr, payload.ipAddress)
    }

    const keyPair = generateKeyPair()
    const presharedKey = generatePresharedKey()

    const runProvisioning = async (ipAddress: string) => {
      return db.transaction(async (trx) => {
        const device = new Device()
        device.useTransaction(trx)
        device.siteId = payload.siteId ?? server.network.siteId
        device.networkId = server.networkId
        device.ipAddress = ipAddress
        device.name = payload.name
        device.type = profile === 'mikrotik' || profile === 'openwrt' ? 'router' : 'host'
        device.description = payload.description ?? 'Dispositivo conectado via VPN WireGuard'
        device.status = 'unknown'
        device.isMonitored = true
        device.snmpEnabled = Boolean(payload.snmpEnabled)
        device.snmpCommunity = payload.snmpEnabled ? (payload.snmpCommunity ?? 'public') : null
        device.snmpVersion = payload.snmpEnabled ? (payload.snmpVersion ?? 'v2c') : null
        await device.save()

        const peer = new VpnPeer()
        peer.useTransaction(trx)
        peer.vpnServerId = server.id
        peer.deviceId = device.id
        peer.publicKey = keyPair.publicKey
        peer.presharedKey = presharedKey
        peer.deviceProfile = profile
        peer.persistentKeepalive = PERSISTENT_KEEPALIVE_SECONDS
        peer.enabled = true
        peer.bytesRx = 0
        peer.bytesTx = 0
        await peer.save()

        await this.monitorProvisioner.provision(device, {
          snmpEnabled: Boolean(payload.snmpEnabled),
          snmpCommunity: device.snmpCommunity,
          snmpVersion: device.snmpVersion,
          trx,
        })

        peer.$setRelated('device', device)
        return peer
      })
    }

    const peer = payload.ipAddress
      ? await runProvisioning(payload.ipAddress)
      : await this.ipAllocator.allocate(server.networkId, cidr, runProvisioning)

    await this.serverService.applyConfiguration(server)

    clientKeyStore.put(this.secretKey(peer.id), keyPair.privateKey)
    const artifact = this.generateArtifact(server, peer, peer.device, keyPair.privateKey)

    return { peer, artifact }
  }

  /**
   * Renomeia o dispositivo do peer.
   *
   * Existe separado do `PUT /api/devices/:id` de propósito: aquele endpoint
   * sincroniza "o primeiro monitor" do dispositivo, e um peer da VPN tem dois
   * (ping e SNMP) — o SNMP perderia community e versão se caísse ali.
   */
  async rename(peerId: number, name: string): Promise<VpnPeer> {
    const newName = name.trim()
    if (!newName) {
      throw new Error('Informe o nome do dispositivo')
    }

    const { peer, server } = await this.loadPeer(peerId)
    const device = peer.device
    const previousName = device.name
    if (previousName === newName) return peer

    await db.transaction(async (trx) => {
      device.useTransaction(trx)
      device.name = newName
      await device.save()

      // Só acompanha os monitores que ainda usam o nome gerado no
      // provisionamento — um monitor renomeado à mão continua como está.
      const monitors = await Monitor.query({ client: trx }).where('deviceId', device.id)
      for (const monitor of monitors) {
        const prefix = monitor.type === 'ping' ? 'Ping' : monitor.type === 'snmp' ? 'SNMP' : null
        if (!prefix || monitor.name !== `${prefix} ${previousName}`) continue

        monitor.name = `${prefix} ${newName}`
        await monitor.save()
      }
    })

    // O `wg0.conf` traz o nome como comentário de cada peer.
    await this.serverService.applyConfiguration(server)

    return peer
  }

  /** Gera novo par de chaves e PSK, invalidando imediatamente os anteriores. */
  async rotateKeys(peerId: number): Promise<{ peer: VpnPeer; artifact: GeneratedArtifact }> {
    const { peer, server } = await this.loadPeer(peerId)
    const keyPair = generateKeyPair()

    peer.publicKey = keyPair.publicKey
    peer.presharedKey = generatePresharedKey()
    await peer.save()

    await this.serverService.applyConfiguration(server)

    clientKeyStore.put(this.secretKey(peer.id), keyPair.privateKey)
    const artifact = this.generateArtifact(server, peer, peer.device, keyPair.privateKey)

    return { peer, artifact }
  }

  /**
   * Artefato de configuração do peer. A chave privada só aparece na primeira
   * leitura após criação/rotação; depois vem o placeholder.
   */
  async buildArtifact(peerId: number): Promise<GeneratedArtifact> {
    const { peer, server } = await this.loadPeer(peerId)
    const privateKey = clientKeyStore.consume(this.secretKey(peer.id))
    return this.generateArtifact(server, peer, peer.device, privateKey)
  }

  /** Regras de firewall do perfil — usadas no diagnóstico de "não responde ao ping". */
  async firewallHints(peerId: number): Promise<{ profile: VpnDeviceProfile; content: string }> {
    const { peer, server } = await this.loadPeer(peerId)
    const generator = profileRegistry.resolve(peer.deviceProfile)
    const context = this.buildContext(server, peer, peer.device, null)

    return { profile: peer.deviceProfile, content: generator.firewallHints(context) }
  }

  /** Revoga o peer, remove o dispositivo (liberando o IP) e reescreve o `wg0.conf`. */
  async revoke(peerId: number): Promise<void> {
    const { peer, server } = await this.loadPeer(peerId)

    const deviceId = peer.deviceId
    await peer.delete()

    if (deviceId) {
      await this.cleanupService.deleteDevice(deviceId)
    }

    clientKeyStore.consume(this.secretKey(peerId))
    await this.serverService.applyConfiguration(server)
  }

  private generateArtifact(
    server: VpnServer,
    peer: VpnPeer,
    device: Device,
    clientPrivateKey: string | null
  ): GeneratedArtifact {
    const generator = profileRegistry.resolve(peer.deviceProfile)
    return generator.generate(this.buildContext(server, peer, device, clientPrivateKey))
  }

  private secretKey(peerId: number): string {
    return `vpn-peer:${peerId}`
  }
}
