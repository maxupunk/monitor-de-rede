import type { VpnDeviceProfile } from '#models/vpn_peer'

/**
 * Contrato dos geradores de configuração por perfil de equipamento.
 *
 * Adicionar suporte a um novo equipamento significa criar uma nova
 * implementação e registrá-la — nenhuma classe existente é alterada (OCP).
 */

/** Keepalive fixo: sem ele o NAT do cliente expira e o servidor perde o caminho de volta. */
export const PERSISTENT_KEEPALIVE_SECONDS = 25

export interface PeerConfigContext {
  /** Nome do dispositivo, usado em comentários do script. */
  peerName: string
  /** IP fixo do peer dentro da VPN (ex.: 10.8.0.11). */
  peerIpAddress: string
  /** Faixa da VPN (ex.: 10.8.0.0/24) — único valor aceito em AllowedIPs. */
  vpnCidr: string
  /** IP do servidor NetMonitor dentro da VPN (ex.: 10.8.0.1). */
  serverVpnAddress: string
  /** Chave privada do cliente — existe apenas em memória, nunca é persistida. */
  clientPrivateKey: string
  serverPublicKey: string
  presharedKey: string | null
  endpointHost: string
  endpointPort: number
  mtu: number
  dnsServers?: string | null
  snmpEnabled: boolean
  snmpCommunity?: string | null
}

export type ArtifactDelivery = 'copy' | 'download' | 'qrcode'

export interface GeneratedArtifact {
  profile: VpnDeviceProfile
  /** Rótulo amigável do equipamento. */
  label: string
  /** Como o usuário deve consumir o artefato. */
  delivery: ArtifactDelivery
  fileName: string
  /** Dica de linguagem para realce de sintaxe no frontend. */
  language: string
  content: string
  instructions: string[]
  /** Perfis móveis podem exibir QR Code. */
  supportsQrCode: boolean
}

export interface VpnProfileGenerator {
  readonly profile: VpnDeviceProfile
  readonly label: string
  /** Ícone (Material Design Icons) usado nos cards do wizard. */
  readonly icon: string
  /** Apenas perfis móveis entregam a configuração por QR Code. */
  readonly supportsQrCode: boolean
  /** Artefato principal (script ou arquivo de configuração). */
  generate(context: PeerConfigContext): GeneratedArtifact
  /**
   * Regras que liberam ICMP/SNMP na interface da VPN — usadas no diagnóstico
   * "túnel conectado, mas o dispositivo não responde".
   */
  firewallHints(context: PeerConfigContext): string
}

/** Prefixo/sufixo de cabeçalho padrão dos artefatos. */
export function artifactHeader(peerName: string): string {
  return `# === NetMonitor · WireGuard — ${peerName} ===`
}
