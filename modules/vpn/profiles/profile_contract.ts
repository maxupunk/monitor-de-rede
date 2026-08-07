import type { VpnDeviceProfile } from '#models/vpn_peer'

/**
 * Contrato dos geradores de configuração por perfil de equipamento.
 *
 * Adicionar suporte a um novo equipamento significa criar uma nova
 * implementação e registrá-la — nenhuma classe existente é alterada (OCP).
 */

/** Keepalive fixo: sem ele o NAT do cliente expira e o servidor perde o caminho de volta. */
export const PERSISTENT_KEEPALIVE_SECONDS = 25

/**
 * Nome da interface/túnel criado nos clientes que instalam por script.
 *
 * Precisa valer nos dois mundos: no Linux vira `wg-quick@<nome>` e no Windows
 * vira o nome do serviço do túnel — ambos limitados a 15 caracteres do conjunto
 * `[A-Za-z0-9_=+.-]`. Fixo (e não derivado do nome do dispositivo) justamente
 * para nunca cair fora dessa janela.
 */
export const WG_TUNNEL_NAME = 'netmonitor'

/** Placeholder exibido quando a chave privada já foi entregue e descartada. */
export const PRIVATE_KEY_UNAVAILABLE = '<CHAVE-PRIVADA-INDISPONIVEL-ROTACIONE-AS-CHAVES>'

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

/** Par rótulo/valor exibido no resumo "os dados do túnel" antes do script. */
export interface ArtifactSummaryItem {
  label: string
  value: string
}

/**
 * Alternativa de instalação entregue ao lado do artefato principal — o mesmo
 * túnel, porém escrito para outro gerenciador de pacotes (winget, apt, dnf,
 * opkg, apk...). Cada variante é autocontida: instala o cliente, grava o perfil
 * e sobe o túnel.
 */
export interface ArtifactVariant {
  /** Identificador estável usado como chave de aba no frontend. */
  id: string
  label: string
  /** Onde essa variante se aplica (ex.: "Debian, Ubuntu, Mint"). */
  hint: string
  icon: string
  fileName: string
  language: string
  content: string
  instructions: string[]
}

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
  /** Parâmetros do túnel em formato legível — não contém a chave privada. */
  summary: ArtifactSummaryItem[]
  /** Scripts de terminal equivalentes, um por gerenciador de pacotes. */
  variants: ArtifactVariant[]
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

/**
 * Remove acentos e pontuação tipográfica.
 *
 * O console do RouterOS e o do OpenWrt trabalham em ASCII: um `·`, um travessão
 * ou um nome de dispositivo acentuado chegam truncados no editor de linha e
 * podem levar junto o começo do comando seguinte.
 */
export function asciiSafe(text: string): string {
  return text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[‐-―]/g, '-')
    .replace(/[‘’]/g, "'")
    .replace(/[“”]/g, '"')
    .replace(/[^\x20-\x7e]/g, ' ')
    .trim()
}

/**
 * Cabeçalho dos scripts colados em console de equipamento.
 *
 * Quando a chave privada já foi consumida, o script inteiro é inútil — o
 * equipamento responderia apenas "invalid private key". Avisar aqui troca esse
 * erro críptico por uma instrução clara.
 */
export function artifactHeader(context: PeerConfigContext): string[] {
  const title = `# === NetMonitor - WireGuard - ${asciiSafe(context.peerName)} ===`
  if (context.clientPrivateKey !== PRIVATE_KEY_UNAVAILABLE) return [title]

  return [
    title,
    '# ATENCAO: a chave privada deste dispositivo ja foi entregue e nao pode ser',
    '# exibida outra vez. Este script NAO vai funcionar como esta.',
    '# Use "Rotacionar chaves" no NetMonitor e copie o script novo.',
  ]
}

/**
 * Resumo dos parâmetros do túnel, idêntico para todos os perfis: é o que o
 * usuário confere antes de colar o script (e o que ele digita à mão quando o
 * equipamento não é nenhum dos suportados).
 */
export function artifactSummary(context: PeerConfigContext): ArtifactSummaryItem[] {
  const prefixLength = context.vpnCidr.split('/')[1]

  return [
    { label: 'Dispositivo', value: context.peerName },
    { label: 'IP fixo na VPN', value: `${context.peerIpAddress}/${prefixLength}` },
    { label: 'Endpoint do servidor', value: `${context.endpointHost}:${context.endpointPort}` },
    { label: 'Chave pública do servidor', value: context.serverPublicKey },
    { label: 'Rotas no túnel (AllowedIPs)', value: context.vpnCidr },
    { label: 'NetMonitor na VPN', value: context.serverVpnAddress },
    { label: 'MTU', value: String(context.mtu) },
    { label: 'Keepalive', value: `${PERSISTENT_KEEPALIVE_SECONDS}s` },
    ...(context.dnsServers ? [{ label: 'DNS', value: context.dnsServers }] : []),
    {
      label: 'Chave pré-compartilhada',
      value: context.presharedKey ? 'sim (incluída no script)' : 'não',
    },
    ...(context.snmpEnabled
      ? [{ label: 'SNMP', value: `community "${context.snmpCommunity || 'public'}"` }]
      : []),
  ]
}
