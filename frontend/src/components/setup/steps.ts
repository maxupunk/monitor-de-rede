import type { Area } from '@/bindings/Area'
import type { OnboardingIntentInput } from '@/bindings/OnboardingIntentInput'

export type SetupStepId =
  | 'intent'
  | 'site'
  | 'addresses'
  | 'network'
  | 'dns'
  | 'vpn'
  | 'apps'
  | 'agents'
  | 'general'
  | 'notifications'

export interface SetupStep {
  id: SetupStepId
  title: string
  icon: string
  /** Área dona da etapa: ela só aparece com a área ligada (`core` aparece sempre). */
  area: Area
  /** A etapa some quando esta área está ligada (outra etapa dela já cobre o assunto). */
  hiddenWith?: Area
}

/** A primeira etapa: o que a instalação vai gerenciar. Só o administrador escolhe. */
export const INTENT_STEP: SetupStep = {
  id: 'intent',
  title: 'Áreas',
  icon: 'mdi-view-grid-plus-outline',
  area: 'core',
}

/**
 * As etapas do assistente, na ordem, cada uma com a sua área (ADR 017). O
 * caminho só Servidores fica em Endereços → Servidores → Notificações; o de
 * Rede, nas etapas de sempre.
 */
export const SETUP_STEPS: readonly SetupStep[] = [
  { id: 'site', title: 'Local / Site', icon: 'mdi-domain', area: 'network' },
  { id: 'addresses', title: 'Endereços', icon: 'mdi-server-network', area: 'core' },
  { id: 'network', title: 'Sub-rede', icon: 'mdi-lan', area: 'network' },
  { id: 'dns', title: 'DNS', icon: 'mdi-dns-outline', area: 'network' },
  { id: 'vpn', title: 'VPN', icon: 'mdi-shield-lock-outline', area: 'vpn' },
  { id: 'apps', title: 'Aplicativos', icon: 'mdi-apps', area: 'network' },
  { id: 'agents', title: 'Servidores', icon: 'mdi-server', area: 'servers' },
  { id: 'general', title: 'Geral', icon: 'mdi-bell-ring-outline', area: 'network' },
  {
    id: 'notifications',
    title: 'Notificações',
    icon: 'mdi-bell-ring-outline',
    area: 'core',
    hiddenWith: 'network',
  },
]

/** As etapas que valem para as áreas ligadas, com a escolha das áreas na frente quando pedida. */
export function setupSteps(isEnabled: (area: Area) => boolean, withIntent: boolean): SetupStep[] {
  const steps = SETUP_STEPS.filter(
    (step) => isEnabled(step.area) && !(step.hiddenWith && isEnabled(step.hiddenWith))
  )
  return withIntent ? [INTENT_STEP, ...steps] : steps
}

/** Rede ou Servidores precisa estar ligada: sem nenhuma das duas, não há o que monitorar. */
export function isIntentValid(intent: OnboardingIntentInput): boolean {
  return intent.network || intent.servers
}
