import type { AiContainerActionMode } from '@/bindings/AiContainerActionMode'

/** Uma opção de escolha única com ícone e a explicação que aparece embaixo. */
export interface AiModeOption<T extends string> {
  value: T
  title: string
  icon: string
  hint: string
}

/**
 * Testes ativos de rede numa escolha só. O contrato da API continua com os
 * dois booleanos (`allowActiveTools` e `requireToolConfirmation`); o modo
 * existe apenas na tela.
 */
export type AiActiveToolsMode = 'off' | 'confirm' | 'auto'

export const ACTIVE_TOOLS_OPTIONS: AiModeOption<AiActiveToolsMode>[] = [
  {
    value: 'off',
    title: 'Desligado',
    icon: 'mdi-cancel',
    hint: 'A IA só lê o que o sistema já coletou; não dispara nenhum teste na rede.',
  },
  {
    value: 'confirm',
    title: 'Pedir permissão',
    icon: 'mdi-hand-back-right-outline',
    hint: 'A IA propõe o teste no chat e só o executa depois do seu clique em Confirmar.',
  },
  {
    value: 'auto',
    title: 'Automático',
    icon: 'mdi-robot-outline',
    hint: 'A IA executa os testes sozinha quando o diagnóstico pede; cada um aparece no chat.',
  },
]

export const CONTAINER_ACTION_OPTIONS: AiModeOption<AiContainerActionMode>[] = [
  {
    value: 'off',
    title: 'Desligado',
    icon: 'mdi-cancel',
    hint: 'A IA só consulta os containers; nunca muda o estado deles.',
  },
  {
    value: 'confirm',
    title: 'Pedir permissão',
    icon: 'mdi-hand-back-right-outline',
    hint: 'A IA propõe a ação no chat e só executa depois do seu clique em Confirmar.',
  },
  {
    value: 'auto',
    title: 'Automático',
    icon: 'mdi-robot-outline',
    hint: 'A IA executa sozinha quando o diagnóstico pede; a ação aparece no chat e fica na auditoria em seu nome.',
  },
]

export function activeToolsMode(
  allowActiveTools: boolean,
  requireToolConfirmation: boolean
): AiActiveToolsMode {
  if (!allowActiveTools) return 'off'
  return requireToolConfirmation ? 'confirm' : 'auto'
}

/**
 * Os dois booleanos do contrato para um modo. Desligar mantém a preferência
 * de confirmação como estava, para religar do mesmo jeito.
 */
export function activeToolsFlags(
  mode: AiActiveToolsMode,
  currentRequireConfirmation: boolean
): { allowActiveTools: boolean; requireToolConfirmation: boolean } {
  if (mode === 'off') {
    return { allowActiveTools: false, requireToolConfirmation: currentRequireConfirmation }
  }
  return { allowActiveTools: true, requireToolConfirmation: mode === 'confirm' }
}
