import { DateTime } from 'luxon'
import VpnPeer from '#models/vpn_peer'
import { FileConfigSink, type VpnConfigSink } from './config_writer.js'

/**
 * Telemetria dos túneis.
 *
 * O container WireGuard publica periodicamente a saída de `wg show <iface> dump`
 * no volume compartilhado (`<iface>.status`). O servidor apenas lê esse arquivo —
 * assim continua sem `NET_ADMIN` e sem acesso ao socket do Docker.
 */

export interface WgPeerStatus {
  publicKey: string
  presharedKey: string | null
  endpoint: string | null
  allowedIps: string[]
  latestHandshakeAt: Date | null
  bytesRx: number
  bytesTx: number
  persistentKeepalive: number
}

const NONE = '(none)'
const OFF = 'off'

/**
 * Parser de `wg show <iface> dump`.
 *
 * A primeira linha descreve a interface (4 campos) e as seguintes descrevem os
 * peers (8 campos, separados por TAB).
 */
export function parseWgDump(dump: string): WgPeerStatus[] {
  const lines = (dump ?? '')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0)

  const peers: WgPeerStatus[] = []

  for (const line of lines) {
    const columns = line.split('\t')
    // Linha da interface (private-key, public-key, listen-port, fwmark).
    if (columns.length < 8) continue

    const [publicKey, presharedKey, endpoint, allowedIps, handshake, rx, tx, keepalive] = columns
    const handshakeSeconds = Number.parseInt(handshake, 10)

    peers.push({
      publicKey,
      presharedKey: presharedKey === NONE ? null : presharedKey,
      endpoint: endpoint === NONE ? null : endpoint,
      allowedIps: allowedIps === NONE ? [] : allowedIps.split(',').filter(Boolean),
      latestHandshakeAt:
        Number.isFinite(handshakeSeconds) && handshakeSeconds > 0
          ? new Date(handshakeSeconds * 1000)
          : null,
      bytesRx: Number.parseInt(rx, 10) || 0,
      bytesTx: Number.parseInt(tx, 10) || 0,
      persistentKeepalive: keepalive === OFF ? 0 : Number.parseInt(keepalive, 10) || 0,
    })
  }

  return peers
}

export class PeerStatusService {
  constructor(private sink: VpnConfigSink = new FileConfigSink()) {}

  /** Lê e interpreta o dump publicado pelo container WireGuard. */
  async readStatus(interfaceName: string): Promise<WgPeerStatus[]> {
    const dump = await this.sink.read(`${interfaceName}.status`)
    if (!dump) return []
    return parseWgDump(dump)
  }

  /** Atualiza handshake e contadores de tráfego dos peers persistidos. */
  async syncPeers(interfaceName: string, vpnServerId: number): Promise<number> {
    const statuses = await this.readStatus(interfaceName)
    if (statuses.length === 0) return 0

    const byPublicKey = new Map(statuses.map((status) => [status.publicKey, status]))
    const peers = await VpnPeer.query().where('vpnServerId', vpnServerId)
    let updated = 0

    for (const peer of peers) {
      const status = byPublicKey.get(peer.publicKey)
      if (!status) continue

      peer.bytesRx = status.bytesRx
      peer.bytesTx = status.bytesTx
      peer.lastHandshakeAt = status.latestHandshakeAt
        ? DateTime.fromJSDate(status.latestHandshakeAt)
        : peer.lastHandshakeAt

      if (peer.$isDirty) {
        await peer.save()
        updated++
      }
    }

    return updated
  }
}
