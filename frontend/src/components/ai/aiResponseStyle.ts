import type { AiResponseStyle } from '@/bindings/AiResponseStyle'

export interface ResponseStyleOption {
  value: AiResponseStyle
  title: string
  icon: string
  hint: string
}

export const RESPONSE_STYLE_OPTIONS: ResponseStyleOption[] = [
  {
    value: 'concise',
    title: 'Direto',
    icon: 'mdi-lightning-bolt-outline',
    hint: 'Só o diagnóstico e a ação recomendada, em poucas linhas. Usa o mínimo de tokens de resposta.',
  },
  {
    value: 'normal',
    title: 'Normal',
    icon: 'mdi-text-long',
    hint: 'Explica a causa provável, as evidências e os passos de correção. Respostas mais longas.',
  },
]

export function responseStyleOption(style?: AiResponseStyle | null): ResponseStyleOption {
  return (
    RESPONSE_STYLE_OPTIONS.find((option) => option.value === style) ?? RESPONSE_STYLE_OPTIONS[0]
  )
}
