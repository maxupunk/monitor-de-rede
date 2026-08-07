import type Monitor from '#models/monitor'
import type VpnPeer from '#models/vpn_peer'

export interface PeerHints {
  /** Túnel ativo, mas o ping falha: provável firewall bloqueando na interface WG. */
  needsFirewallHint: boolean
  /**
   * Túnel ativo e ping falhando, mas o monitor não roda no `vpn-probe`: o ICMP
   * sai da máquina da API, que não tem rota para dentro do túnel. O pacote nem
   * chega ao equipamento — acusar o firewall dele seria diagnóstico falso.
   */
  pingOutsideTunnel: boolean
  /** Monitor de ping provisionado automaticamente para o peer (§4.7) — usado para navegar ao histórico de conectividade. */
  pingMonitorId: number | null
}

/**
 * Único ponto de cálculo dos avisos derivados de VPN + ping — usado tanto no
 * `GET /vpn/peers` (carga inicial) quanto no snapshot publicado via SSE
 * (`vpn:peers_updated`), para que os dois caminhos nunca divirjam.
 *
 * A régua aqui é `hasFreshProofOfLife`, não `connectionStatus === 'connected'`.
 * O ping falha em segundos e vira `down` no primeiro erro, enquanto a janela de
 * "conectado" tolera minutos de propósito: quem desconectasse o equipamento
 * caía na brecha entre as duas e via "túnel conectado, mas não responde a ping"
 * — afirmando justamente o contrário do que havia acontecido.
 */
export function computePeerHints(peer: VpnPeer, monitor: Monitor | undefined): PeerHints {
  const silentTunnel = peer.hasFreshProofOfLife && monitor?.status === 'down'
  const outsideTunnel = silentTunnel && !monitor?.probeId

  return {
    needsFirewallHint: silentTunnel && !outsideTunnel,
    pingOutsideTunnel: outsideTunnel,
    pingMonitorId: monitor?.id ?? null,
  }
}
