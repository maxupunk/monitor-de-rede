/**
 * Rodapé de consumo de uma resposta: o modelo fica à vista; tokens, cache,
 * velocidade e janela de contexto vão para o detalhe (ⓘ).
 */
import {
  formatCompactCount,
  formatElapsedMs,
  formatPercent,
  formatTokenRate,
} from '@/utils/formatters'
import { shortModelName, tokensPerSecond, type AiUsageInfo } from '@/utils/aiChatStream'

export interface AiUsageSummary {
  /** Nome curto do modelo, sem o prefixo do provedor. */
  model: string | null
  /** Linhas do detalhe, já formatadas; vazio quando não há o que mostrar. */
  details: string[]
}

export function summarizeUsage(usage?: AiUsageInfo | null): AiUsageSummary | null {
  if (!usage) return null
  const rate = tokensPerSecond(usage)
  const hasTokens = usage.promptTokens > 0 || usage.completionTokens > 0
  const context =
    usage.contextTokens && usage.contextWindow
      ? `Contexto: ${formatCompactCount(usage.contextTokens)} de ${formatCompactCount(usage.contextWindow)} tokens (${formatPercent((usage.contextTokens / usage.contextWindow) * 100, 0)}${usage.contextWindowReported ? '' : ', janela estimada'})`
      : null
  const details = [
    usage.model ? `Modelo: ${usage.model}` : null,
    hasTokens ? `Enviados ao provedor (↑): ${formatCompactCount(usage.promptTokens)} tokens` : null,
    usage.cachedTokens
      ? `Servidos do cache do provedor: ${formatCompactCount(usage.cachedTokens)} tokens`
      : null,
    hasTokens
      ? `Gerados na resposta (↓): ${formatCompactCount(usage.completionTokens)} tokens`
      : null,
    rate !== null ? `Velocidade de geração: ${formatTokenRate(rate)}` : null,
    usage.durationMs ? `Tempo total da resposta: ${formatElapsedMs(usage.durationMs)}` : null,
    context,
  ].filter((line): line is string => line !== null)
  return { model: shortModelName(usage.model), details }
}
