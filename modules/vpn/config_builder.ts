import { parseCidr } from './cidr.js'

/**
 * Montagem do conteúdo do `wg0.conf` do servidor. Função pura: não toca no
 * banco nem no disco, o que a torna trivialmente testável.
 */

export interface ServerInterfaceInput {
  interfaceName: string
  address: string
  cidr: string
  listenPort: number
  privateKey: string
  mtu: number
  /** Quando falso, aplica isolamento entre peers (recomendado p/ monitoramento). */
  allowPeerToPeer: boolean
}

export interface PeerEntryInput {
  name: string
  publicKey: string
  presharedKey?: string | null
  ipAddress: string
  enabled?: boolean
}

export class WireGuardConfigBuilder {
  /**
   * Regras de isolamento do §7 do roadmap. Ficam em PostUp/PostDown porque
   * `wg syncconf` (hot-reload) só aplica peers — o firewall é montado quando a
   * interface sobe.
   */
  private buildIsolationRules(input: ServerInterfaceInput): string[] {
    const { interfaceName, address, allowPeerToPeer } = input

    if (allowPeerToPeer) {
      return [
        `PostUp = iptables -A FORWARD -i ${interfaceName} -o ${interfaceName} -j ACCEPT`,
        `PostDown = iptables -D FORWARD -i ${interfaceName} -o ${interfaceName} -j ACCEPT`,
      ]
    }

    return [
      `PostUp = iptables -A FORWARD -i ${interfaceName} -d ${address} -j ACCEPT`,
      `PostUp = iptables -A FORWARD -i ${interfaceName} -o ${interfaceName} -j DROP`,
      `PostDown = iptables -D FORWARD -i ${interfaceName} -d ${address} -j ACCEPT`,
      `PostDown = iptables -D FORWARD -i ${interfaceName} -o ${interfaceName} -j DROP`,
    ]
  }

  buildInterfaceSection(input: ServerInterfaceInput): string {
    const { prefixLength } = parseCidr(input.cidr)

    return [
      '[Interface]',
      `Address = ${input.address}/${prefixLength}`,
      `ListenPort = ${input.listenPort}`,
      `PrivateKey = ${input.privateKey}`,
      `MTU = ${input.mtu}`,
      ...this.buildIsolationRules(input),
    ].join('\n')
  }

  buildPeerSection(peer: PeerEntryInput): string {
    const lines = [`# ${peer.name}`, '[Peer]', `PublicKey = ${peer.publicKey}`]

    if (peer.presharedKey) {
      lines.push(`PresharedKey = ${peer.presharedKey}`)
    }

    // /32: cada peer só pode originar tráfego do próprio endereço da VPN.
    lines.push(`AllowedIPs = ${peer.ipAddress}/32`)

    return lines.join('\n')
  }

  /** Gera o arquivo completo, ignorando peers desabilitados (revogação imediata). */
  build(server: ServerInterfaceInput, peers: PeerEntryInput[]): string {
    const sections = [
      '# Gerado automaticamente pelo NetMonitor — não editar manualmente.',
      this.buildInterfaceSection(server),
      ...peers.filter((peer) => peer.enabled !== false).map((peer) => this.buildPeerSection(peer)),
    ]

    return `${sections.join('\n\n')}\n`
  }
}
