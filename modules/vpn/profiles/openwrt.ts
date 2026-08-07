import {
  artifactHeader,
  artifactSummary,
  PERSISTENT_KEEPALIVE_SECONDS,
  type ArtifactVariant,
  type GeneratedArtifact,
  type PeerConfigContext,
  type VpnProfileGenerator,
} from './profile_contract.js'
import type { VpnDeviceProfile } from '#models/vpn_peer'

/**
 * Gerenciador de pacotes do OpenWrt. O `opkg` valeu até a 23.05; a partir da
 * 24.10 (e no SNAPSHOT) o sistema migrou para o `apk`, e um firmware não tem os
 * dois. Por isso o script principal detecta qual existe, e cada variante fixa um
 * deles para quem já sabe a versão do equipamento.
 */
type OpenWrtPackageManager = 'auto' | 'opkg' | 'apk'

interface OpenWrtInstallStrategy {
  /** Instala pacotes atualizando os índices antes. */
  install(packages: string[]): string[]
}

const OPKG: OpenWrtInstallStrategy = {
  install: (packages) => [`opkg update && opkg install ${packages.join(' ')}`],
}

const APK: OpenWrtInstallStrategy = {
  install: (packages) => [`apk update && apk add ${packages.join(' ')}`],
}

/** Ramo `if command -v apk ... else opkg ... fi`, usado no script principal. */
const AUTO: OpenWrtInstallStrategy = {
  install: (packages) => [
    'if command -v apk >/dev/null 2>&1; then   # OpenWrt 24.10+ / SNAPSHOT',
    `  apk update && apk add ${packages.join(' ')}`,
    'else                                      # OpenWrt 23.05 e anteriores',
    `  opkg update && opkg install ${packages.join(' ')}`,
    'fi',
  ],
}

const STRATEGIES: Record<OpenWrtPackageManager, OpenWrtInstallStrategy> = {
  auto: AUTO,
  opkg: OPKG,
  apk: APK,
}

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
  static readonly PACKAGES = ['wireguard-tools', 'luci-proto-wireguard']

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

  private buildSnmpSection(context: PeerConfigContext, strategy: OpenWrtInstallStrategy): string[] {
    if (!context.snmpEnabled) return []

    const community = context.snmpCommunity || 'public'
    return [
      '',
      '# SNMP (community cadastrada no NetMonitor)',
      ...strategy.install(['snmpd']),
      `uci set snmpd.public.community='${community}'`,
      `uci set snmpd.public.source='${context.vpnCidr}'`,
      'uci commit snmpd && /etc/init.d/snmpd restart && /etc/init.d/snmpd enable',
    ]
  }

  /** Corpo do script — idêntico entre as variantes, exceto a linha de instalação. */
  private buildScript(context: PeerConfigContext, manager: OpenWrtPackageManager): string {
    const iface = OpenWrtProfileGenerator.INTERFACE_NAME
    const prefixLength = context.vpnCidr.split('/')[1]
    const strategy = STRATEGIES[manager]

    const lines = [
      ...artifactHeader(context),
      ...strategy.install(OpenWrtProfileGenerator.PACKAGES),
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
      ...this.buildSnmpSection(context, strategy),
      '',
      'uci commit network && uci commit firewall',
      '/etc/init.d/network restart && /etc/init.d/firewall restart',
    ]

    return `${lines.join('\n')}\n`
  }

  private buildVariants(context: PeerConfigContext): ArtifactVariant[] {
    const instructions = [
      'Acesse o roteador por SSH (ex.: ssh root@192.168.1.1).',
      'Cole o bloco completo de comandos e pressione Enter.',
      'A rede reinicia ao final e o túnel sobe automaticamente.',
    ]

    return [
      {
        id: 'opkg',
        label: 'opkg',
        hint: 'OpenWrt 23.05, 22.03, 21.02 e anteriores',
        icon: 'mdi-package-variant-closed',
        fileName: `netmonitor-${context.peerName}-opkg.sh`,
        language: 'shell',
        content: this.buildScript(context, 'opkg'),
        instructions,
      },
      {
        id: 'apk',
        label: 'apk',
        hint: 'OpenWrt 24.10+ e SNAPSHOT — o opkg foi substituído pelo apk',
        icon: 'mdi-package-variant',
        fileName: `netmonitor-${context.peerName}-apk.sh`,
        language: 'shell',
        content: this.buildScript(context, 'apk'),
        instructions,
      },
    ]
  }

  generate(context: PeerConfigContext): GeneratedArtifact {
    return {
      profile: this.profile,
      label: this.label,
      delivery: 'copy',
      fileName: `netmonitor-${context.peerName}.sh`,
      language: 'shell',
      content: this.buildScript(context, 'auto'),
      instructions: [
        'Acesse o roteador por SSH (ex.: ssh root@192.168.1.1).',
        'Cole o bloco completo de comandos e pressione Enter.',
        'A rede reinicia ao final e o túnel sobe automaticamente.',
      ],
      supportsQrCode: false,
      summary: artifactSummary(context),
      variants: this.buildVariants(context),
    }
  }
}
