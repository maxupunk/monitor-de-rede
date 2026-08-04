import {
  artifactHeader,
  PERSISTENT_KEEPALIVE_SECONDS,
  type GeneratedArtifact,
  type PeerConfigContext,
  type VpnProfileGenerator,
} from './profile_contract.js'
import type { VpnDeviceProfile } from '#models/vpn_peer'

/**
 * Gerador de comandos UCI para OpenWrt. Também é colado no terminal (SSH),
 * já que o LuCI não importa QR Code.
 */
export class OpenWrtProfileGenerator implements VpnProfileGenerator {
  readonly profile: VpnDeviceProfile = 'openwrt'
  readonly label = 'OpenWrt'
  readonly icon = 'mdi-router-wireless'
  readonly supportsQrCode = false

  static readonly INTERFACE_NAME = 'wg_nm'
  static readonly FIREWALL_ZONE = 'vpn_netmonitor'

  firewallHints(_context: PeerConfigContext): string {
    const iface = OpenWrtProfileGenerator.INTERFACE_NAME

    return [
      '# Zona de firewall permitindo o monitoramento do NetMonitor',
      'uci add firewall zone',
      `uci set firewall.@zone[-1].name='${OpenWrtProfileGenerator.FIREWALL_ZONE}'`,
      "uci set firewall.@zone[-1].input='ACCEPT'",
      "uci set firewall.@zone[-1].output='ACCEPT'",
      "uci set firewall.@zone[-1].forward='REJECT'",
      `uci add_list firewall.@zone[-1].network='${iface}'`,
    ].join('\n')
  }

  private buildSnmpSection(context: PeerConfigContext): string[] {
    if (!context.snmpEnabled) return []

    const community = context.snmpCommunity || 'public'
    return [
      '',
      '# SNMP (community cadastrada no NetMonitor)',
      'opkg install snmpd',
      `uci set snmpd.public.community='${community}'`,
      `uci set snmpd.public.source='${context.vpnCidr}'`,
      'uci commit snmpd && /etc/init.d/snmpd restart && /etc/init.d/snmpd enable',
    ]
  }

  generate(context: PeerConfigContext): GeneratedArtifact {
    const iface = OpenWrtProfileGenerator.INTERFACE_NAME
    const prefixLength = context.vpnCidr.split('/')[1]

    const lines = [
      artifactHeader(context.peerName),
      'opkg update && opkg install wireguard-tools luci-proto-wireguard',
      '',
      `uci set network.${iface}=interface`,
      `uci set network.${iface}.proto='wireguard'`,
      `uci set network.${iface}.private_key='${context.clientPrivateKey}'`,
      `uci set network.${iface}.mtu='${context.mtu}'`,
      `uci add_list network.${iface}.addresses='${context.peerIpAddress}/${prefixLength}'`,
      '',
      `uci add network wireguard_${iface}`,
      `uci set network.@wireguard_${iface}[-1].public_key='${context.serverPublicKey}'`,
      ...(context.presharedKey
        ? [`uci set network.@wireguard_${iface}[-1].preshared_key='${context.presharedKey}'`]
        : []),
      `uci set network.@wireguard_${iface}[-1].endpoint_host='${context.endpointHost}'`,
      `uci set network.@wireguard_${iface}[-1].endpoint_port='${context.endpointPort}'`,
      `uci set network.@wireguard_${iface}[-1].persistent_keepalive='${PERSISTENT_KEEPALIVE_SECONDS}'`,
      `uci set network.@wireguard_${iface}[-1].route_allowed_ips='1'`,
      `uci add_list network.@wireguard_${iface}[-1].allowed_ips='${context.vpnCidr}'`,
      '',
      this.firewallHints(context),
      ...this.buildSnmpSection(context),
      '',
      'uci commit network && uci commit firewall',
      '/etc/init.d/network restart && /etc/init.d/firewall restart',
    ]

    return {
      profile: this.profile,
      label: this.label,
      delivery: 'copy',
      fileName: `netmonitor-${context.peerName}.sh`,
      language: 'shell',
      content: `${lines.join('\n')}\n`,
      instructions: [
        'Acesse o roteador por SSH (ex.: ssh root@192.168.1.1).',
        'Cole o bloco completo de comandos e pressione Enter.',
        'A rede reinicia ao final e o túnel sobe automaticamente.',
      ],
      supportsQrCode: false,
    }
  }
}
