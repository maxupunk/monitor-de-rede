import {
  artifactHeader,
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

  firewallHints(_context: PeerConfigContext): string {
    const iface = MikrotikProfileGenerator.INTERFACE_NAME

    return [
      '# Libera o monitoramento do NetMonitor na interface WireGuard',
      `/ip/firewall/filter/add chain=input in-interface=${iface} protocol=icmp \\`,
      '    action=accept comment="NetMonitor ICMP" place-before=0',
      `/ip/firewall/filter/add chain=input in-interface=${iface} protocol=udp \\`,
      '    dst-port=161 action=accept comment="NetMonitor SNMP" place-before=0',
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

  generate(context: PeerConfigContext): GeneratedArtifact {
    const iface = MikrotikProfileGenerator.INTERFACE_NAME
    const prefixLength = context.vpnCidr.split('/')[1]

    const lines = [
      artifactHeader(context.peerName),
      `/interface/wireguard/add name=${iface} listen-port=${MikrotikProfileGenerator.LOCAL_LISTEN_PORT} private-key="${context.clientPrivateKey}"`,
      `/ip/address/add address=${context.peerIpAddress}/${prefixLength} interface=${iface}`,
      '',
      `/interface/wireguard/peers/add interface=${iface} \\`,
      `    public-key="${context.serverPublicKey}" \\`,
      ...(context.presharedKey ? [`    preshared-key="${context.presharedKey}" \\`] : []),
      `    endpoint-address=${context.endpointHost} endpoint-port=${context.endpointPort} \\`,
      `    allowed-address=${context.vpnCidr} \\`,
      `    persistent-keepalive=${PERSISTENT_KEEPALIVE_SECONDS}s`,
      '',
      this.firewallHints(context),
      ...this.buildSnmpSection(context),
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
    }
  }
}
