import type { VpnPeerConnectionStatus } from '#models/vpn_peer'
import { ALERT_FIELDS, VPN_STATUS_TRANSITION } from '../alert_fields.js'
import type { AlertDataset } from '../contracts/alert_evaluation.js'

/**
 * Traduz o estado de um túnel WireGuard (e o que mudou desde o ciclo anterior)
 * para o vocabulário avaliado pelas regras.
 *
 * Só publica fatos: a decisão de alertar — e com qual severidade — pertence às
 * regras cadastradas em "Regras Configuradas", como no dataset de interfaces.
 */

/** Estados em que o túnel é considerado no ar para efeito de transição. */
const HEALTHY: VpnPeerConnectionStatus[] = ['connected']

export interface VpnPeerFacts {
  peerName: string
  status: VpnPeerConnectionStatus
  previousStatus: VpnPeerConnectionStatus | null
  secondsSinceActivity: number | null
}

export function buildVpnPeerDataset(facts: VpnPeerFacts): AlertDataset {
  const dataset: AlertDataset = {
    [ALERT_FIELDS.vpnPeerName]: facts.peerName,
    [ALERT_FIELDS.vpnPeerStatus]: facts.status,
  }

  if (facts.secondsSinceActivity !== null) {
    dataset[ALERT_FIELDS.vpnSecondsSinceActivity] = Math.round(facts.secondsSinceActivity)
  }

  const transition = resolveTransition(facts.previousStatus, facts.status)
  if (transition) {
    dataset[ALERT_FIELDS.vpnStatusTransition] = transition
    dataset.vpnPreviousStatus = facts.previousStatus
  }

  return dataset
}

/**
 * Qual transição o par (anterior, atual) descreve.
 *
 * Sem estado anterior não há transição: o primeiro ciclo depois de criar o peer
 * — ou depois de subir a versão que passou a persistir o estado — só estabelece
 * a linha de base. Alertar ali reportaria como queda um túnel que talvez nunca
 * tenha subido.
 */
function resolveTransition(
  previous: VpnPeerConnectionStatus | null,
  current: VpnPeerConnectionStatus
): string | null {
  if (previous === null || previous === current) return null

  const wasHealthy = HEALTHY.includes(previous)
  const isHealthy = HEALTHY.includes(current)

  if (wasHealthy && current === 'disconnected') return VPN_STATUS_TRANSITION.disconnected
  if (wasHealthy && current === 'unstable') return VPN_STATUS_TRANSITION.destabilized

  // `awaiting ➔ connected` também é retorno: o túnel subiu pela primeira vez e
  // qualquer alerta aberto sobre ele deixou de fazer sentido.
  if (!wasHealthy && isHealthy) return VPN_STATUS_TRANSITION.reconnected

  // Degradação em cadeia (`unstable ➔ disconnected`) conta como queda: quem
  // configurou a regra de queda espera ser avisado, mesmo que o túnel já
  // estivesse claudicando no ciclo anterior.
  if (current === 'disconnected') return VPN_STATUS_TRANSITION.disconnected

  return null
}

/** `true` quando o dataset descreve alguma mudança de estado do túnel. */
export function hasVpnTransition(dataset: AlertDataset): boolean {
  return dataset[ALERT_FIELDS.vpnStatusTransition] !== undefined
}

/** `true` quando o túnel voltou — sinaliza ao motor que os alertas podem fechar. */
export function isVpnRecovery(dataset: AlertDataset): boolean {
  return dataset[ALERT_FIELDS.vpnStatusTransition] === VPN_STATUS_TRANSITION.reconnected
}

const STATUS_LABELS: Record<VpnPeerConnectionStatus, string> = {
  connected: 'conectado',
  unstable: 'instável',
  disconnected: 'desconectado',
  awaiting: 'aguardando primeira conexão',
}

/** Frase legível do que foi observado, usada como mensagem do alerta. */
export function describeVpnPeerState(dataset: AlertDataset): string {
  const name = String(dataset[ALERT_FIELDS.vpnPeerName] ?? 'desconhecido')
  const status = dataset[ALERT_FIELDS.vpnPeerStatus] as VpnPeerConnectionStatus
  const transition = dataset[ALERT_FIELDS.vpnStatusTransition]
  const idle = dataset[ALERT_FIELDS.vpnSecondsSinceActivity] as number | undefined
  const silence = idle !== undefined ? ` Sem sinal há ${formatSeconds(idle)}.` : ''

  switch (transition) {
    case VPN_STATUS_TRANSITION.disconnected:
      return `Túnel VPN de ${name} caiu.${silence}`
    case VPN_STATUS_TRANSITION.destabilized:
      return `Túnel VPN de ${name} ficou instável.${silence}`
    case VPN_STATUS_TRANSITION.reconnected:
      return `Túnel VPN de ${name} voltou a responder.`
    default:
      return `Túnel VPN de ${name} está ${STATUS_LABELS[status] ?? status}.${silence}`
  }
}

function formatSeconds(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes} min`
  const hours = Math.floor(minutes / 60)
  return `${hours}h${String(minutes % 60).padStart(2, '0')}`
}
