import { DateTime } from 'luxon'
import VpnPeer from '#models/vpn_peer'
import { FileConfigSink, resolveConfigDir, type VpnConfigSink } from './config_writer.js'

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

/**
 * Sincronizações em voo por interface.
 *
 * A leitura acontece a cada request que exibe status — várias abas abertas, ou
 * a tela de VPN pedindo servidor e peers ao mesmo tempo, disputariam a mesma
 * escrita e um dos ciclos calcularia o delta de bytes contra um valor já
 * atualizado pelo outro, perdendo o sinal de keepalive.
 */
const inFlightSyncs = new Map<string, Promise<number>>()

/**
 * Interfaces cujo dump já foi reportado como ausente.
 *
 * Um `<iface>.status` ilegível é indistinguível de "nada mudou": o sink devolve
 * `null` e a sincronização vira um no-op. Sem este aviso, um processo sem o
 * volume compartilhado publica telemetria congelada indefinidamente e em
 * silêncio — foi exatamente assim que o scheduler passou despercebido. Guarda
 * o estado para avisar na transição, não a cada ciclo.
 */
const missingDumpWarned = new Set<string>()

export class PeerStatusService {
  constructor(private sink: VpnConfigSink = new FileConfigSink()) {}

  /** Lê e interpreta o dump publicado pelo container WireGuard. */
  async readStatus(interfaceName: string): Promise<WgPeerStatus[]> {
    const dump = await this.sink.read(`${interfaceName}.status`)

    if (!dump) {
      if (!missingDumpWarned.has(interfaceName)) {
        missingDumpWarned.add(interfaceName)
        console.warn(
          `[PeerStatusService] ${interfaceName}.status não pôde ser lido em ${resolveConfigDir()} — ` +
            'a telemetria dos túneis não será atualizada por este processo. ' +
            'Verifique se o volume `wg-config` está montado neste container.'
        )
      }
      return []
    }

    if (missingDumpWarned.delete(interfaceName)) {
      console.info(`[PeerStatusService] ${interfaceName}.status voltou a ser legível.`)
    }

    return parseWgDump(dump)
  }

  /** Atualiza handshake, contadores de tráfego e sinal de vida dos peers persistidos. */
  async syncPeers(interfaceName: string, vpnServerId: number): Promise<number> {
    const key = `${interfaceName}:${vpnServerId}`
    const running = inFlightSyncs.get(key)
    if (running) return running

    const sync = this.runSync(interfaceName, vpnServerId).finally(() => {
      inFlightSyncs.delete(key)
    })

    inFlightSyncs.set(key, sync)
    return sync
  }

  private async runSync(interfaceName: string, vpnServerId: number): Promise<number> {
    const statuses = await this.readStatus(interfaceName)
    if (statuses.length === 0) return 0

    const byPublicKey = new Map(statuses.map((status) => [status.publicKey, status]))
    const peers = await VpnPeer.query().where('vpnServerId', vpnServerId)
    const now = DateTime.now()
    let updated = 0

    for (const peer of peers) {
      const status = byPublicKey.get(peer.publicKey)
      if (!status) continue

      const previousRx = peer.bytesRx
      const previousHandshake = peer.lastHandshakeAt

      peer.bytesRx = status.bytesRx
      peer.bytesTx = status.bytesTx
      peer.lastHandshakeAt = status.latestHandshakeAt
        ? DateTime.fromJSDate(status.latestHandshakeAt)
        : peer.lastHandshakeAt

      // Contador de RX subiu desde a leitura anterior: chegou pelo menos um
      // keepalive, então o túnel está vivo agora — independente de o handshake
      // ser antigo. Queda de contador significa interface reiniciada, não vida.
      const receivedNewBytes = status.bytesRx > previousRx
      const renegotiated =
        peer.lastHandshakeAt !== null &&
        (previousHandshake === null || peer.lastHandshakeAt > previousHandshake)

      if (receivedNewBytes || renegotiated) {
        peer.lastSeenAt = now
      }

      if (peer.$isDirty) {
        await peer.save()
        updated++
      }
    }

    return updated
  }
}
