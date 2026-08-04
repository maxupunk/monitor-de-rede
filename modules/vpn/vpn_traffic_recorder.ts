import { DateTime } from 'luxon'
import VpnServer from '#models/vpn_server'
import VpnPeer from '#models/vpn_peer'
import Metric from '#models/metric'
import { PeerStatusService } from './peer_status.js'

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
  constructor(private peerStatusService = new PeerStatusService()) {}

  /** Sincroniza o status de todos os servidores ativos e grava um snapshot de tráfego por peer. */
  async recordAll(): Promise<number> {
    const servers = await VpnServer.query().where('active', true)
    let recorded = 0

    for (const server of servers) {
      await this.peerStatusService.syncPeers(server.interfaceName, server.id)
      recorded += await this.recordServerPeers(server.id)
    }

    return recorded
  }

  private async recordServerPeers(vpnServerId: number): Promise<number> {
    const peers = await VpnPeer.query().where('vpnServerId', vpnServerId).where('enabled', true)

    for (const peer of peers) {
      await this.recordPeer(peer)
    }

    return peers.length
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

    const rxBps = this.computeRate(Number(lastRx?.value ?? 0), lastRx?.recordedAt ?? null, peer.bytesRx, now)
    const txBps = this.computeRate(Number(lastTx?.value ?? 0), lastTx?.recordedAt ?? null, peer.bytesTx, now)

    await Metric.createMany([
      { deviceId: peer.deviceId, name: VPN_METRIC_BYTES_RX, value: peer.bytesRx, unit: 'bytes', recordedAt: now },
      { deviceId: peer.deviceId, name: VPN_METRIC_BYTES_TX, value: peer.bytesTx, unit: 'bytes', recordedAt: now },
      { deviceId: peer.deviceId, name: VPN_METRIC_RX_BPS, value: rxBps, unit: 'bps', recordedAt: now },
      { deviceId: peer.deviceId, name: VPN_METRIC_TX_BPS, value: txBps, unit: 'bps', recordedAt: now },
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
