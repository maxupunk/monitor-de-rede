import { DateTime } from 'luxon'
import VpnPeer from '#models/vpn_peer'
import Device from '#models/device'
import { AlertManager } from '#modules/alerts/alert_manager'
import { AlertScopeKey } from '#modules/alerts/contracts/alert_evaluation'
import { ALERT_FIELDS } from '#modules/alerts/alert_fields'
import {
  buildVpnPeerDataset,
  describeVpnPeerState,
  hasVpnTransition,
  isVpnRecovery,
} from '#modules/alerts/datasets/vpn_peer_dataset'
import { EventBus } from '#modules/events/event_bus'

/**
 * Observa o estado dos túneis WireGuard.
 *
 * Roda logo depois da sincronização da telemetria (`PeerStatusService`), quando
 * `lastSeenAt`/`lastHandshakeAt` já refletem o que o container publicou. Compara
 * o estado persistido no ciclo anterior com o atual, publica a transição no feed
 * em tempo real e entrega os fatos ao motor de alertas.
 *
 * Não decide o que é alerta: a política ("túnel caído é crítico", "instável só
 * avisa depois de 5 minutos") vive nas regras do catálogo `vpn_*`.
 */
export class VpnPeerStateWatcher {
  constructor(
    private alertManager = new AlertManager(),
    private eventBus = EventBus.getInstance()
  ) {}

  /** Avalia todos os peers habilitados de um servidor. Devolve quantos mudaram de estado. */
  async evaluateServerPeers(vpnServerId: number): Promise<number> {
    const peers = await VpnPeer.query()
      .where('vpnServerId', vpnServerId)
      .where('enabled', true)
      .preload('device')

    let transitions = 0
    for (const peer of peers) {
      if (await this.evaluatePeer(peer)) transitions++
    }

    return transitions
  }

  /** `true` quando o túnel mudou de estado neste ciclo. */
  async evaluatePeer(peer: VpnPeer): Promise<boolean> {
    const device = peer.device ?? (await Device.find(peer.deviceId))
    const peerName = device?.name ?? `Peer #${peer.id}`
    const status = peer.connectionStatus
    const previousStatus = peer.lastConnectionStatus

    const dataset = buildVpnPeerDataset({
      peerName,
      status,
      previousStatus,
      secondsSinceActivity: this.secondsSinceActivity(peer),
    })

    const changed = hasVpnTransition(dataset)
    const message = describeVpnPeerState(dataset)

    // O estado é gravado antes da avaliação: se a notificação falhar, o ciclo
    // seguinte não repete a mesma transição como se fosse nova.
    if (previousStatus !== status) {
      peer.lastConnectionStatus = status
      await peer.save()
    }

    if (!changed) return false

    this.publishTransition(peer, device, dataset, message)

    await this.alertManager.evaluate({
      scope: { siteId: device?.siteId ?? null, deviceId: peer.deviceId, monitorId: null },
      scopeKey: AlertScopeKey.vpnPeer(peer.id),
      targetLabel: peerName,
      dataset,
      message,
      data: {
        eventType: 'vpn_peer_state',
        vpnPeerId: peer.id,
        vpnServerId: peer.vpnServerId,
        ...dataset,
      },
      recovered: isVpnRecovery(dataset),
    })

    return true
  }

  /** Feed em tempo real: a transição observada, independentemente de alertar. */
  private publishTransition(
    peer: VpnPeer,
    device: Device | null,
    dataset: Record<string, unknown>,
    message: string
  ): void {
    this.eventBus.emit('vpn:peer_status_change', {
      vpnPeerId: peer.id,
      vpnServerId: peer.vpnServerId,
      deviceId: peer.deviceId,
      deviceName: device?.name ?? null,
      previousStatus: dataset.vpnPreviousStatus ?? null,
      currentStatus: dataset[ALERT_FIELDS.vpnPeerStatus] ?? null,
      transition: dataset[ALERT_FIELDS.vpnStatusTransition] ?? null,
      message,
    })
  }

  private secondsSinceActivity(peer: VpnPeer): number | null {
    const lastActivity = peer.lastActivityAt
    if (!lastActivity) return null
    return Math.max(0, DateTime.now().diff(lastActivity, 'seconds').seconds)
  }
}
