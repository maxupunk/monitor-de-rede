import type { HttpContext } from '@adonisjs/core/http'
import QRCode from 'qrcode'
import type VpnPeer from '#models/vpn_peer'
import type { VpnDeviceProfile, VpnPeerConnectionStatus } from '#models/vpn_peer'
import { PRIVATE_KEY_UNAVAILABLE, VpnPeerService } from '#modules/vpn/vpn_peer_service'
import { VpnServerService } from '#modules/vpn/vpn_server_service'
import { IpAllocator } from '#modules/vpn/ip_allocator'
import { profileRegistry } from '#modules/vpn/profiles/profile_registry'
import type { GeneratedArtifact } from '#modules/vpn/profiles/profile_contract'
import { errorMessage } from '#modules/shared/errors'
import { sensitiveEndpointLimiter, vpnAuditLogger } from '#modules/vpn/access_control'

/**
 * Contrato de resposta de um peer.
 *
 * Declarado campo a campo em vez de espalhar `peer.serialize()`: além de tornar
 * a resposta verificável pelo TypeScript, garante que material sensível — a
 * chave pré-compartilhada — não escape por descuido em nenhuma rota.
 */
/** Artefato entregue ao frontend — perfis móveis já vêm com o QR Code renderizado. */
export type SerializedVpnArtifact = GeneratedArtifact & { qrSvg: string | null }

export interface SerializedVpnPeer {
  id: number
  vpnServerId: number
  deviceId: number
  publicKey: string
  deviceProfile: VpnDeviceProfile
  persistentKeepalive: number
  lastHandshakeAt: string | null
  /** Último keepalive contabilizado — é o sinal de vida que sustenta o status. */
  lastSeenAt: string | null
  bytesRx: number
  bytesTx: number
  enabled: boolean
  createdAt: string | null
  updatedAt: string | null
  connectionStatus: VpnPeerConnectionStatus
}

/**
 * Peers da VPN: listagem com telemetria, wizard de criação, artefatos de
 * configuração, rotação de chaves e revogação.
 */
export default class VpnPeersController {
  private peerService = new VpnPeerService()
  private serverService = new VpnServerService()
  private ipAllocator = new IpAllocator()

  /** Ponto único de serialização do peer — usado por listagem, criação e rotação. */
  private serializePeer(peer: VpnPeer): SerializedVpnPeer {
    return {
      id: peer.id,
      vpnServerId: peer.vpnServerId,
      deviceId: peer.deviceId,
      publicKey: peer.publicKey,
      deviceProfile: peer.deviceProfile,
      persistentKeepalive: peer.persistentKeepalive,
      lastHandshakeAt: peer.lastHandshakeAt?.toISO() ?? null,
      lastSeenAt: peer.lastSeenAt?.toISO() ?? null,
      bytesRx: peer.bytesRx,
      bytesTx: peer.bytesTx,
      enabled: peer.enabled,
      createdAt: peer.createdAt?.toISO() ?? null,
      updatedAt: peer.updatedAt?.toISO() ?? null,
      connectionStatus: peer.connectionStatus,
    }
  }

  /**
   * Renderiza o QR Code junto do artefato.
   *
   * Precisa acontecer na mesma resposta: a chave privada só existe até a
   * primeira leitura (`clientKeyStore.consume`), então buscar o QR Code numa
   * segunda requisição devolveria um código com o placeholder no lugar da chave.
   */
  private async serializeArtifact(artifact: GeneratedArtifact): Promise<SerializedVpnArtifact> {
    // Sem a chave privada o QR Code levaria o celular a um túnel que nunca conecta.
    const usable = artifact.supportsQrCode && !artifact.content.includes(PRIVATE_KEY_UNAVAILABLE)
    if (!usable) return { ...artifact, qrSvg: null }

    const qrSvg = await QRCode.toString(artifact.content, { type: 'svg', margin: 1, width: 320 })
    return { ...artifact, qrSvg }
  }

  /** Identidade usada em rate limit e auditoria dos endpoints sensíveis. */
  private requesterId(ctx: HttpContext): string {
    const userId = (ctx.auth?.user as { id?: number | string } | undefined)?.id
    return userId ? `user:${userId}` : `ip:${ctx.request.ip()}`
  }

  private enforceRateLimit(ctx: HttpContext, scope: string): boolean {
    const decision = sensitiveEndpointLimiter.consume(`${scope}:${this.requesterId(ctx)}`)
    if (decision.allowed) return true

    ctx.response.header('Retry-After', String(decision.retryAfterSeconds))
    ctx.response.tooManyRequests({
      message: `Muitas solicitações de configuração. Tente novamente em ${decision.retryAfterSeconds}s.`,
    })
    return false
  }

  private badRequestFromError(ctx: HttpContext, error: unknown) {
    return ctx.response.badRequest({ message: errorMessage(error) })
  }

  private audit(
    ctx: HttpContext,
    action: Parameters<typeof vpnAuditLogger.log>[0]['action'],
    peerId: number | null,
    details?: Record<string, unknown>
  ): void {
    vpnAuditLogger.log({
      action,
      peerId,
      userId: (ctx.auth?.user as { id?: number | string } | undefined)?.id ?? null,
      ipAddress: ctx.request.ip(),
      details,
    })
  }

  /** GET /api/vpn/peers */
  async index({ response }: HttpContext) {
    const items = await this.peerService.list()

    return response.ok(
      items.map(({ peer, needsFirewallHint, pingOutsideTunnel, pingMonitorId }) => ({
        ...this.serializePeer(peer),
        needsFirewallHint,
        pingOutsideTunnel,
        pingMonitorId,
        device: peer.device?.serialize() ?? null,
      }))
    )
  }

  /** GET /api/vpn/peers/next-ip — sugestão de IP livre para o wizard. */
  async nextIp(ctx: HttpContext) {
    const { response } = ctx
    const server = await this.serverService.find()
    if (!server) {
      return response.badRequest({ message: 'Servidor VPN ainda não foi configurado' })
    }

    try {
      const ipAddress = await this.ipAllocator.findNextFree(server.networkId, server.network.cidr)
      return response.ok({ ipAddress, cidr: server.network.cidr })
    } catch (error: unknown) {
      return this.badRequestFromError(ctx, error)
    }
  }

  /** POST /api/vpn/peers — cria peer, aloca IP e provisiona device + monitores. */
  async store(ctx: HttpContext) {
    const { request, response } = ctx
    const payload = request.only([
      'name',
      'profile',
      'ipAddress',
      'siteId',
      'snmpEnabled',
      'snmpCommunity',
      'snmpVersion',
      'description',
    ])

    if (!payload.name || !payload.profile) {
      return response.badRequest({
        message: 'Informe o nome do dispositivo e o perfil do equipamento',
      })
    }

    try {
      const { peer, artifact } = await this.peerService.create(payload)
      this.audit(ctx, 'peer_created', peer.id, { profile: peer.deviceProfile })

      return response.created({
        peer: this.serializePeer(peer),
        device: peer.device?.serialize() ?? null,
        artifact: await this.serializeArtifact(artifact),
      })
    } catch (error: unknown) {
      return this.badRequestFromError(ctx, error)
    }
  }

  /** PATCH /api/vpn/peers/:id — renomeia o dispositivo do peer. */
  async update(ctx: HttpContext) {
    const { params, request, response } = ctx
    const name = request.input('name')

    if (typeof name !== 'string' || !name.trim()) {
      return response.badRequest({ message: 'Informe o nome do dispositivo' })
    }

    try {
      const peer = await this.peerService.rename(Number(params.id), name)

      return response.ok({
        ...this.serializePeer(peer),
        device: peer.device?.serialize() ?? null,
      })
    } catch (error: unknown) {
      return this.badRequestFromError(ctx, error)
    }
  }

  /** GET /api/vpn/peers/:id/config — 🔒 credencial de acesso à rede. */
  async config(ctx: HttpContext) {
    if (!this.enforceRateLimit(ctx, 'config')) return

    const { params, response } = ctx
    const artifact = await this.peerService.buildArtifact(Number(params.id))
    this.audit(ctx, 'config_download', Number(params.id), { profile: artifact.profile })

    return response.ok(await this.serializeArtifact(artifact))
  }

  /** GET /api/vpn/peers/:id/qrcode — 🔒 apenas perfis móveis. */
  async qrcode(ctx: HttpContext) {
    if (!this.enforceRateLimit(ctx, 'qrcode')) return

    const { params, response } = ctx
    const artifact = await this.peerService.buildArtifact(Number(params.id))

    if (!artifact.supportsQrCode) {
      return response.badRequest({
        message: `O perfil ${artifact.label} não utiliza QR Code — copie o script gerado.`,
      })
    }

    const { qrSvg } = await this.serializeArtifact(artifact)
    if (!qrSvg) {
      return response.conflict({
        message:
          'A chave privada deste dispositivo já foi entregue. Rotacione as chaves para gerar um novo QR Code.',
      })
    }

    this.audit(ctx, 'qrcode_download', Number(params.id), { profile: artifact.profile })

    return response.ok({ profile: artifact.profile, fileName: artifact.fileName, svg: qrSvg })
  }

  /** POST /api/vpn/peers/:id/rotate */
  async rotate(ctx: HttpContext) {
    if (!this.enforceRateLimit(ctx, 'rotate')) return

    const { params, response } = ctx

    try {
      const { peer, artifact } = await this.peerService.rotateKeys(Number(params.id))
      this.audit(ctx, 'key_rotation', peer.id, { profile: peer.deviceProfile })

      return response.ok({
        message: 'Novo par de chaves gerado. A configuração anterior foi invalidada.',
        peer: this.serializePeer(peer),
        artifact: await this.serializeArtifact(artifact),
      })
    } catch (error: unknown) {
      return this.badRequestFromError(ctx, error)
    }
  }

  /** POST /api/vpn/peers/:id/firewall-hints — diagnóstico do erro nº 1. */
  async firewallHints({ params, response }: HttpContext) {
    const hints = await this.peerService.firewallHints(Number(params.id))
    const generator = profileRegistry.resolve(hints.profile)

    return response.ok({
      profile: hints.profile,
      label: generator.label,
      content: hints.content,
      message:
        'Túnel conectado, mas o dispositivo não responde a ping? Copie as regras abaixo e aplique no equipamento.',
    })
  }

  /** DELETE /api/vpn/peers/:id — revoga e libera o IP. */
  async destroy(ctx: HttpContext) {
    const { params, response } = ctx

    await this.peerService.revoke(Number(params.id))
    this.audit(ctx, 'peer_revoked', Number(params.id))

    return response.ok({ message: 'Peer revogado. O acesso foi cortado imediatamente.' })
  }
}
