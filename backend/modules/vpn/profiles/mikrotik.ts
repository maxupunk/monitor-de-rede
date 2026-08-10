import {
  artifactHeader,
  artifactSummary,
  PERSISTENT_KEEPALIVE_SECONDS,
  type GeneratedArtifact,
  type PeerConfigContext,
  type VpnProfileGenerator,
} from './profile_contract.js'
import type { VpnDeviceProfile } from '#models/vpn_peer'

/**
 * Gerador de script RouterOS v7+ (MikroTik).
 * O usuário cola o bloco no terminal do Winbox — MikroTik não lê QR Code.
 */
export class MikrotikProfileGenerator implements VpnProfileGenerator {
  readonly profile: VpnDeviceProfile = 'mikrotik'
  readonly label = 'MikroTik RouterOS v7+'
  readonly icon = 'mdi-router-network'
  readonly supportsQrCode = false

  static readonly INTERFACE_NAME = 'wg-netmonitor'
  /** Porta local padrão do WireGuard no RouterOS. */
  static readonly LOCAL_LISTEN_PORT = 13231
  /**
   * Comentário que marca tudo que o NetMonitor cria.
   *
   * É o que torna o script repetível: `remove [find comment="..."]` não falha
   * quando não há nada para remover, enquanto `find interface=<nome>` explodiria
   * com "input does not match any value of interface" caso a interface não
   * exista — exatamente o erro em cascata de uma primeira execução malsucedida.
   */
  static readonly TAG = 'NetMonitor'

  firewallHints(_context: PeerConfigContext): string {
    const iface = MikrotikProfileGenerator.INTERFACE_NAME
    const tag = MikrotikProfileGenerator.TAG

    return [
      '# Libera o monitoramento do NetMonitor na interface WireGuard',
      `/ip/firewall/filter/remove [find comment="${tag} ICMP"]`,
      `/ip/firewall/filter/remove [find comment="${tag} SNMP"]`,
      `/ip/firewall/filter/add chain=input in-interface=${iface} protocol=icmp \\`,
      `    action=accept comment="${tag} ICMP"`,
      `/ip/firewall/filter/add chain=input in-interface=${iface} protocol=udp \\`,
      `    dst-port=161 action=accept comment="${tag} SNMP"`,
      '# Sobe as duas regras para o topo da chain (move funciona ate com a chain vazia)',
      `/ip/firewall/filter/move [find comment="${tag} SNMP"] destination=0`,
      `/ip/firewall/filter/move [find comment="${tag} ICMP"] destination=0`,
    ].join('\n')
  }

  private buildSnmpSection(context: PeerConfigContext): string[] {
    if (!context.snmpEnabled) return []

    const community = context.snmpCommunity || 'public'
    return [
      '',
      '# SNMP (community cadastrada no NetMonitor)',
      `/snmp/community/set [find default=yes] addresses=${context.vpnCidr} name="${community}"`,
      '/snmp/set enabled=yes contact="NetMonitor"',
    ]
  }

  /**
   * Limpeza do que uma execução anterior tenha deixado para trás. Sem isso, uma
   * segunda tentativa esbarra em "already have interface with such name".
   */
  private buildCleanupSection(context: PeerConfigContext): string[] {
    const iface = MikrotikProfileGenerator.INTERFACE_NAME
    const tag = MikrotikProfileGenerator.TAG
    const prefixLength = context.vpnCidr.split('/')[1]

    return [
      '# Limpa uma instalacao anterior (nao falha se nao houver nada)',
      `/interface/wireguard/peers/remove [find comment="${tag}"]`,
      `/ip/address/remove [find comment="${tag}"]`,
      // Busca também pelo endereço em si: entradas criadas por versões antigas
      // do script não têm o comentário e sobreviveriam à limpeza acima,
      // deixando um IP duplicado na VPN.
      `/ip/address/remove [find address="${context.peerIpAddress}/${prefixLength}"]`,
      `/interface/wireguard/remove [find name="${iface}"]`,
    ]
  }

  generate(context: PeerConfigContext): GeneratedArtifact {
    const iface = MikrotikProfileGenerator.INTERFACE_NAME
    const tag = MikrotikProfileGenerator.TAG
    const prefixLength = context.vpnCidr.split('/')[1]

    const lines = [
      ...artifactHeader(context),
      '',
      ...this.buildCleanupSection(context),
      '',
      '# Interface WireGuard e IP fixo dentro da VPN',
      `/interface/wireguard/add name=${iface} listen-port=${MikrotikProfileGenerator.LOCAL_LISTEN_PORT} \\`,
      `    private-key="${context.clientPrivateKey}" comment="${tag}"`,
      `/ip/address/add address=${context.peerIpAddress}/${prefixLength} interface=${iface} comment="${tag}"`,
      '',
      `/interface/wireguard/peers/add interface=${iface} \\`,
      `    public-key="${context.serverPublicKey}" \\`,
      ...(context.presharedKey ? [`    preshared-key="${context.presharedKey}" \\`] : []),
      `    endpoint-address=${context.endpointHost} endpoint-port=${context.endpointPort} \\`,
      `    allowed-address=${context.vpnCidr} \\`,
      `    persistent-keepalive=${PERSISTENT_KEEPALIVE_SECONDS}s comment="${tag}"`,
      '',
      this.firewallHints(context),
      ...this.buildSnmpSection(context),
      '',
      '# Conferencia: "last-handshake" deve aparecer em poucos segundos',
      `/interface/wireguard/peers/print where interface=${iface}`,
    ]

    return {
      profile: this.profile,
      label: this.label,
      delivery: 'copy',
      fileName: `netmonitor-${context.peerName}.rsc`,
      language: 'routeros',
      content: `${lines.join('\n')}\n`,
      instructions: [
        'Abra o Winbox e clique em "New Terminal".',
        'Cole o script completo e pressione Enter.',
        'O túnel sobe em poucos segundos e o dispositivo aparece como conectado no NetMonitor.',
      ],
      supportsQrCode: false,
      summary: artifactSummary(context),
      variants: [],
    }
  }
}
