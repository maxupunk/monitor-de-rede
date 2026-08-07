import { DateTime } from 'luxon'
import VpnServer from '#models/vpn_server'
import VpnPeer from '#models/vpn_peer'
import Metric from '#models/metric'
import Monitor from '#models/monitor'
import { PeerStatusService } from './peer_status.js'
import { VpnPeerStateWatcher } from './vpn_peer_state_watcher.js'
import { computePeerHints } from './peer_hints.js'
import { EventBus } from '#modules/events/event_bus'
import { errorMessage } from '#modules/shared/errors'

/**
 * Histórico de tráfego do túnel WireGuard, persistido em `metrics` — o mesmo
 * modelo já usado para tráfego de interface SNMP. Permite exibir um gráfico de
 * RX/TX ao longo do tempo na aba VPN de `/devices/:id`, e não apenas o
 * contador acumulado exposto por `wg show dump`.
 */

export const VPN_METRIC_BYTES_RX = 'vpn_bytes_rx'
export const VPN_METRIC_BYTES_TX = 'vpn_bytes_tx'
export const VPN_METRIC_RX_BPS = 'vpn_rx_bps'
export const VPN_METRIC_TX_BPS = 'vpn_tx_bps'

export class VpnTrafficRecorder {
  private eventBus = EventBus.getInstance()

  /** Último quadro publicado por servidor, para não repetir um estado idêntico */
  private lastPublished = new Map<number, string>()

  constructor(
    private peerStatusService = new PeerStatusService(),
    private peerStateWatcher = new VpnPeerStateWatcher()
  ) {}

  /**
   * Sincroniza o status de todos os servidores ativos e publica o quadro atual.
   *
   * Separado de `recordAll` de propósito: o status precisa de cadência fina
   * para a tela acompanhar o túnel em tempo real, enquanto o histórico de
   * tráfego não justifica quatro linhas em `metrics` por peer a cada ciclo.
   */
  async syncAll(): Promise<number> {
    const servers = await VpnServer.query().where('active', true)
    let synced = 0

    for (const server of servers) {
      await this.peerStatusService.syncPeers(server.interfaceName, server.id)
      await this.watchPeerState(server.id)
      synced += await this.publishServerPeers(server.id)
    }

    return synced
  }

  /** Sincroniza o status e grava um snapshot de tráfego por peer. */
  async recordAll(): Promise<number> {
    const servers = await VpnServer.query().where('active', true)
    let recorded = 0

    for (const server of servers) {
      await this.peerStatusService.syncPeers(server.interfaceName, server.id)
      await this.watchPeerState(server.id)
      recorded += await this.recordServerPeers(server.id)
    }

    return recorded
  }

  /**
   * A avaliação de alertas não pode derrubar a coleta de telemetria: uma regra
   * mal formada ou uma notificação com falha deixaria o painel de VPN congelado,
   * que é justamente o oposto do que o alerta existe para evitar.
   */
  private async watchPeerState(vpnServerId: number): Promise<void> {
    try {
      await this.peerStateWatcher.evaluateServerPeers(vpnServerId)
    } catch (err: unknown) {
      console.error(
        `[VpnTrafficRecorder] Falha ao avaliar o estado dos túneis do servidor #${vpnServerId}: ${errorMessage(err)}`
      )
    }
  }

  private async recordServerPeers(vpnServerId: number): Promise<number> {
    const peers = await this.enabledPeers(vpnServerId)

    for (const peer of peers) {
      await this.recordPeer(peer)
    }

    await this.publishSnapshot(vpnServerId, peers)
    return peers.length
  }

  private async publishServerPeers(vpnServerId: number): Promise<number> {
    const peers = await this.enabledPeers(vpnServerId)
    await this.publishSnapshot(vpnServerId, peers)
    return peers.length
  }

  private async enabledPeers(vpnServerId: number): Promise<VpnPeer[]> {
    return VpnPeer.query().where('vpnServerId', vpnServerId).where('enabled', true)
  }

  /**
   * Recalcula os mesmos avisos de firewall/ping do `GET /vpn/peers`
   * (via `computePeerHints`) para que o snapshot publicado por aqui nunca
   * fique defasado em relação à carga inicial da tela.
   */
  private async publishSnapshot(vpnServerId: number, peers: VpnPeer[]): Promise<void> {
    if (peers.length === 0) return

    const pingMonitors = await Monitor.query()
      .whereIn(
        'deviceId',
        peers.map((peer) => peer.deviceId)
      )
      .where('type', 'ping')

    const snapshot = peers.map((peer) => {
      const monitor = pingMonitors.find((item) => item.deviceId === peer.deviceId)
      return {
        id: peer.id,
        deviceId: peer.deviceId,
        connectionStatus: peer.connectionStatus,
        lastHandshakeAt: peer.lastHandshakeAt?.toISO() ?? null,
        lastSeenAt: peer.lastSeenAt?.toISO() ?? null,
        bytesRx: peer.bytesRx,
        bytesTx: peer.bytesTx,
        ...computePeerHints(peer, monitor),
      }
    })

    // Um único evento por servidor: a tela de VPN repinta status e contadores
    // sem esperar o operador recarregar a página. Túnel parado repete o mesmo
    // quadro a cada ciclo — nesse caso não há o que repintar.
    const fingerprint = JSON.stringify(snapshot)
    if (this.lastPublished.get(vpnServerId) === fingerprint) return

    this.lastPublished.set(vpnServerId, fingerprint)
    this.eventBus.emit('vpn:peers_updated', { vpnServerId, peers: snapshot })
  }

  private async recordPeer(peer: VpnPeer): Promise<void> {
    const now = DateTime.now()

    const [lastRx, lastTx] = await Promise.all([
      Metric.query()
        .where('deviceId', peer.deviceId)
        .where('name', VPN_METRIC_BYTES_RX)
        .orderBy('recordedAt', 'desc')
        .first(),
      Metric.query()
        .where('deviceId', peer.deviceId)
        .where('name', VPN_METRIC_BYTES_TX)
        .orderBy('recordedAt', 'desc')
        .first(),
    ])

    const rxBps = this.computeRate(
      Number(lastRx?.value ?? 0),
      lastRx?.recordedAt ?? null,
      peer.bytesRx,
      now
    )
    const txBps = this.computeRate(
      Number(lastTx?.value ?? 0),
      lastTx?.recordedAt ?? null,
      peer.bytesTx,
      now
    )

    await Metric.createMany([
      {
        deviceId: peer.deviceId,
        name: VPN_METRIC_BYTES_RX,
        value: peer.bytesRx,
        unit: 'bytes',
        recordedAt: now,
      },
      {
        deviceId: peer.deviceId,
        name: VPN_METRIC_BYTES_TX,
        value: peer.bytesTx,
        unit: 'bytes',
        recordedAt: now,
      },
      {
        deviceId: peer.deviceId,
        name: VPN_METRIC_RX_BPS,
        value: rxBps,
        unit: 'bps',
        recordedAt: now,
      },
      {
        deviceId: peer.deviceId,
        name: VPN_METRIC_TX_BPS,
        value: txBps,
        unit: 'bps',
        recordedAt: now,
      },
    ])
  }

  /** Calcula bps a partir do delta de bytes acumulados, com o mesmo critério de reset usado no `TrafficCollector` SNMP. */
  private computeRate(
    previousValue: number,
    previousAt: DateTime | null,
    currentValue: number,
    now: DateTime
  ): number {
    if (!previousAt) return 0

    const elapsedSeconds = now.diff(previousAt, 'seconds').seconds
    if (elapsedSeconds <= 0) return 0

    let delta = currentValue - previousValue
    if (delta < 0) {
      // Contador reiniciado (ex.: interface WireGuard subiu de novo) — assume que
      // o valor atual é o total acumulado desde o reinício.
      delta = currentValue
    }

    return Math.round((delta * 8) / elapsedSeconds)
  }
}
