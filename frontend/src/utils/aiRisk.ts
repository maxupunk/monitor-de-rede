import type { AiToolRisk } from '@/bindings/AiToolRisk'

export type { AiToolRisk }

export interface AiRiskPresentation {
  label: string
  color: 'success' | 'warning' | 'error'
  icon: string
}

const PRESENTATIONS: Record<AiToolRisk, AiRiskPresentation> = {
  low: { label: 'Risco baixo', color: 'success', icon: 'mdi-shield-check-outline' },
  medium: { label: 'Risco médio', color: 'warning', icon: 'mdi-shield-alert-outline' },
  high: { label: 'Risco alto', color: 'error', icon: 'mdi-alert-octagon-outline' },
}

const RISKS = Object.keys(PRESENTATIONS) as AiToolRisk[]

export function isAiToolRisk(value: unknown): value is AiToolRisk {
  return typeof value === 'string' && (RISKS as string[]).includes(value)
}

export function aiRiskPresentation(risk: AiToolRisk): AiRiskPresentation {
  return PRESENTATIONS[risk]
}
