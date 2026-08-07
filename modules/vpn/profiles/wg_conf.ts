import {
  artifactSummary,
  PERSISTENT_KEEPALIVE_SECONDS,
  WG_TUNNEL_NAME,
  type ArtifactDelivery,
  type ArtifactVariant,
  type GeneratedArtifact,
  type PeerConfigContext,
  type VpnProfileGenerator,
} from './profile_contract.js'
import { createLinuxBashVariant, createWindowsWingetVariant } from './wg_conf_variants.js'
import { parseCidr } from '../cidr.js'
import type { VpnDeviceProfile } from '#models/vpn_peer'

/** Constrói os scripts de terminal a partir do `.conf` já renderizado. */
type VariantBuilder = (context: PeerConfigContext, confContent: string) => ArtifactVariant

/**
 * Gerador do `.conf` padrão do WireGuard, consumido pelos clientes oficiais
 * (Linux, Windows, Android e iOS). O mesmo formato serve ao QR Code.
 */
export class WgConfProfileGenerator implements VpnProfileGenerator {
  constructor(
    readonly profile: VpnDeviceProfile,
    readonly label: string,
    readonly icon: string,
    private delivery: ArtifactDelivery,
    private instructions: string[],
    readonly supportsQrCode = false,
    private variantBuilders: VariantBuilder[] = []
  ) {}

  firewallHints(_context: PeerConfigContext): string {
    return [
      '# Libera ICMP e SNMP vindos do NetMonitor pela interface WireGuard',
      'iptables -A INPUT -i wg0 -p icmp -j ACCEPT',
      'iptables -A INPUT -i wg0 -p udp --dport 161 -j ACCEPT',
    ].join('\n')
  }

  generate(context: PeerConfigContext): GeneratedArtifact {
    const { prefixLength } = parseCidr(context.vpnCidr)

    const lines = [
      '[Interface]',
      `PrivateKey = ${context.clientPrivateKey}`,
      `Address = ${context.peerIpAddress}/${prefixLength}`,
      `MTU = ${context.mtu}`,
      ...(context.dnsServers ? [`DNS = ${context.dnsServers}`] : []),
      '',
      '[Peer]',
      `PublicKey = ${context.serverPublicKey}`,
      ...(context.presharedKey ? [`PresharedKey = ${context.presharedKey}`] : []),
      // Somente a faixa da VPN: com 0.0.0.0/0 a internet do cliente cairia no túnel.
      `AllowedIPs = ${context.vpnCidr}`,
      `Endpoint = ${context.endpointHost}:${context.endpointPort}`,
      `PersistentKeepalive = ${PERSISTENT_KEEPALIVE_SECONDS}`,
    ]

    const content = `${lines.join('\n')}\n`

    return {
      profile: this.profile,
      label: this.label,
      delivery: this.delivery,
      fileName: `netmonitor-${context.peerName}.conf`,
      language: 'ini',
      content,
      instructions: this.instructions,
      supportsQrCode: this.supportsQrCode,
      summary: artifactSummary(context),
      variants: this.variantBuilders.map((build) => build(context, content)),
    }
  }
}

export function createLinuxGenerator(): WgConfProfileGenerator {
  return new WgConfProfileGenerator(
    'linux',
    'Linux',
    'mdi-linux',
    'download',
    [
      `Salve o arquivo como /etc/wireguard/${WG_TUNNEL_NAME}.conf (chmod 600).`,
      `Suba o túnel com: sudo wg-quick up ${WG_TUNNEL_NAME}.`,
      `Habilite na inicialização com: sudo systemctl enable wg-quick@${WG_TUNNEL_NAME}.`,
    ],
    false,
    [createLinuxBashVariant]
  )
}

export function createWindowsGenerator(): WgConfProfileGenerator {
  return new WgConfProfileGenerator(
    'windows',
    'Windows',
    'mdi-microsoft-windows',
    'download',
    [
      'Instale o aplicativo oficial WireGuard para Windows.',
      'Clique em "Adicionar túnel" e selecione o arquivo baixado.',
      'Clique em "Ativar" para conectar.',
    ],
    false,
    [createWindowsWingetVariant]
  )
}

export function createMobileGenerator(): WgConfProfileGenerator {
  return new WgConfProfileGenerator(
    'mobile',
    'Celular (Android / iOS)',
    'mdi-cellphone',
    'qrcode',
    [
      'Instale o aplicativo WireGuard na loja do seu celular.',
      'Toque em "+" e escolha "Ler a partir do código QR".',
      'Aponte a câmera para o código exibido nesta tela.',
    ],
    true
  )
}
