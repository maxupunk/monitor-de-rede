import type { AiSettings } from '@/bindings/AiSettings'

/**
 * Testes ativos (ping, traceroute, portas, DNS, playbooks) em três estados:
 * desligados, com confirmação a cada execução ou automáticos.
 */
export type ActiveTestsMode = 'off' | 'confirm' | 'auto'

export interface ActiveTestsPresentation {
  mode: ActiveTestsMode
  title: string
  color: 'warning' | 'info' | 'success'
  icon: string
  hint: string
}

const PRESENTATIONS: Record<ActiveTestsMode, Omit<ActiveTestsPresentation, 'mode'>> = {
  off: {
    title: 'Testes: desligado',
    color: 'warning',
    icon: 'mdi-wrench-clock',
    hint: 'A IA não roda ping, traceroute, portas, DNS nem playbooks. Ative em Configurações.',
  },
  confirm: {
    title: 'Testes: pedir permissão',
    color: 'info',
    icon: 'mdi-hand-back-right-outline',
    hint: 'A IA pode rodar ping, traceroute, portas, DNS e playbooks — cada um pede sua confirmação.',
  },
  auto: {
    title: 'Testes: automático',
    color: 'success',
    icon: 'mdi-wrench-check',
    hint: 'A IA roda ping, traceroute, portas, DNS e playbooks sem pedir confirmação.',
  },
}

type ActiveTestsSettings = Pick<AiSettings, 'allowActiveTools' | 'requireToolConfirmation'>

export function activeTestsMode(settings?: ActiveTestsSettings | null): ActiveTestsMode {
  if (!settings?.allowActiveTools) return 'off'
  return settings.requireToolConfirmation ? 'confirm' : 'auto'
}

export function activeTestsPresentation(
  settings?: ActiveTestsSettings | null
): ActiveTestsPresentation {
  const mode = activeTestsMode(settings)
  return { mode, ...PRESENTATIONS[mode] }
}
