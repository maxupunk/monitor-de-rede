import type { ChatBenchmarkRow } from '@/bindings/ChatBenchmarkRow'
import type { ChatBenchmarkSplitView } from '@/bindings/ChatBenchmarkSplitView'
import type { DeciderBenchmarkRow } from '@/bindings/DeciderBenchmarkRow'
import { percent, type ModelFact } from '@/utils/decider'

/** Aba ativa na página `/ai-benchmark`. */
export type BenchmarkTab = 'chat' | 'decider'

/** Suíte ativa na aba de modelos de chat (`blind` = cenários reais NOC/ISP, `bench` = suíte oficial). */
export type ChatBenchmarkSuite = 'blind' | 'bench'

/**
 * Normaliza o parâmetro `?tab=` da rota para uma aba válida.
 * Padrão: `'chat'` (ou `'decider'` quando pedido explicitamente).
 */
export function resolveBenchmarkTab(raw: unknown): BenchmarkTab {
  if (typeof raw === 'string' && raw.trim().toLowerCase() === 'decider') {
    return 'decider'
  }
  return 'chat'
}

/**
 * Formata o intervalo de confiança de 90% em português (ex.: `95,6%–99,2%`).
 */
export function formatConfidenceInterval(low: number, high: number): string {
  const safeLow = Number.isFinite(low) ? low : 0
  const safeHigh = Number.isFinite(high) ? high : 0
  return `${percent(safeLow)}–${percent(safeHigh)}`
}

/**
 * Formata uma duração em milissegundos para segundos legíveis em português (ex.: `6,5 s`).
 */
export function formatSecondsPt(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return '—'
  const secs = ms / 1000
  const digits = secs >= 10 ? 1 : 1
  return `${secs.toFixed(digits).replace('.', ',')} s`
}

/**
 * Formata a vazão de geração em tokens por segundo (ex.: `18,1 tok/s`).
 */
export function formatTokensPerSecond(tokPerS: number): string {
  if (!Number.isFinite(tokPerS) || tokPerS <= 0) return '—'
  return `${tokPerS.toFixed(1).replace('.', ',')} tok/s`
}

/**
 * Formata o tamanho em MB para MB ou GB em português (ex.: `1,85 GB`, `510 MB`).
 */
export function formatDownloadSizeMb(mb: number | null | undefined): string {
  if (typeof mb !== 'number' || !Number.isFinite(mb) || mb <= 0) return '—'
  if (mb >= 1000) {
    return `${(mb / 1000).toFixed(2).replace('.', ',')} GB`
  }
  return `${Math.round(mb)} MB`
}

/**
 * Retorna a suíte selecionada para uma linha de modelo de chat (`blind` ou `bench`).
 */
export function chatActiveSplit(
  row: ChatBenchmarkRow,
  suite: ChatBenchmarkSuite
): ChatBenchmarkSplitView | null {
  if (suite === 'blind') {
    return row.blind ?? null
  }
  return row.bench
}

/**
 * Nota de um modelo de chat em um domínio operacional específico.
 */
export function chatDomainScoreOf(
  split: ChatBenchmarkSplitView | null | undefined,
  domainId: string
): number | null {
  if (!split || !Array.isArray(split.domains)) return null
  const found = split.domains.find((d) => d.domain === domainId)
  return found && Number.isFinite(found.score) ? found.score : null
}

/** Cores de selos usadas nas tabelas e cartões de benchmark. */
export type BenchmarkChipColor = ModelFact['color'] | 'primary' | 'secondary'

/**
 * Selo de perfil/quantização para um modelo de chat.
 */
export function chatTierFact(row: ChatBenchmarkRow): {
  label: string
  color: BenchmarkChipColor
  icon: string
} {
  if (row.recommended) {
    return { label: 'Máxima precisão · Padrão', color: 'success', icon: 'mdi-star' }
  }
  if (row.bundled && row.quantization === 'Q5_K_M') {
    return { label: 'Equilibrado · -32% RAM', color: 'primary', icon: 'mdi-scale-balance' }
  }
  if (row.bundled && row.quantization === 'Q4_K_M') {
    return { label: 'Ultraleve · Mais rápido', color: 'info', icon: 'mdi-lightning-bolt' }
  }
  return { label: 'Referência sem ajuste', color: 'secondary', icon: 'mdi-cube-outline' }
}

/**
 * Resumo da velocidade de turno completo de um modelo de chat.
 */
export function chatSpeedFact(
  split: ChatBenchmarkSplitView,
  timeoutMs: number
): { label: string; color: ModelFact['color'] } {
  if (split.errors > 0 || split.turnP95Ms > timeoutMs) {
    return {
      label: `p95 ${formatSecondsPt(split.turnP95Ms)} · ${split.errors} timeout(s)`,
      color: 'error',
    }
  }
  const color: ModelFact['color'] = split.turnP95Ms <= 10_000 ? 'success' : 'info'
  return {
    label: `${formatSecondsPt(split.turnP50Ms)} (p95 ${formatSecondsPt(split.turnP95Ms)})`,
    color,
  }
}

/**
 * Veredito detalhado do portão de qualidade para um modelo de chat na suíte ativa.
 */
export function chatVerdictFact(
  row: ChatBenchmarkRow,
  split: ChatBenchmarkSplitView,
  report: {
    minScore: number
    minTools: number
    minSafety: number
    timeoutMs: number
  }
): {
  passed: boolean
  label: string
  color: ModelFact['color']
  reason: string
} {
  if (row.fit && split.fit) {
    return {
      passed: true,
      label: 'Atende ao portão',
      color: 'success',
      reason: 'Aprovado em todos os 6 critérios de precisão, ferramentas, segurança e latência.',
    }
  }
  const reasons: string[] = []
  if (split.overall < report.minScore) {
    reasons.push(`nota ${percent(split.overall)} < ${percent(report.minScore)}`)
  }
  if (split.tools < report.minTools) {
    reasons.push(`ferramentas ${percent(split.tools)} < ${percent(report.minTools)}`)
  }
  if (split.safety < report.minSafety) {
    reasons.push(`segurança ${percent(split.safety)} < ${percent(report.minSafety)}`)
  }
  if (split.turnP95Ms > report.timeoutMs) {
    reasons.push(`p95 ${formatSecondsPt(split.turnP95Ms)} > ${formatSecondsPt(report.timeoutMs)}`)
  }
  if (split.errors > 0) {
    reasons.push(`${split.errors} erro(s)/timeout(s)`)
  }
  return {
    passed: false,
    label: 'Não atende',
    color: 'warning',
    reason:
      reasons.length > 0
        ? `Reprovado no portão: ${reasons.join(' · ')}.`
        : 'Não atingiu todos os critérios mínimos em uma das suítes.',
  }
}

/**
 * Nota de um modelo do Decisor em uma das 6 tarefas.
 */
export function deciderTaskScoreOf(row: DeciderBenchmarkRow, task: string): number {
  return row.tasks.find((item) => item.task === task)?.score ?? 0
}

/**
 * Selos de recomendação/origem para um modelo do Decisor.
 */
export function deciderTagsOf(
  row: DeciderBenchmarkRow
): { label: string; color: ModelFact['color'] }[] {
  const tags: { label: string; color: ModelFact['color'] }[] = []
  if (row.recommended) tags.push({ label: 'Recomendado', color: 'success' })
  if (row.bundled) tags.push({ label: 'Vem com o NetMonitor', color: 'info' })
  return tags
}

/**
 * Resumo da latência em CPU de um modelo do Decisor.
 */
export function deciderSpeedOf(
  row: DeciderBenchmarkRow,
  timeoutMs = 4000
): { label: string; color: ModelFact['color'] } {
  const secs = (ms: number) => `${(ms / 1000).toFixed(1).replace('.', ',')} s`
  const base = `${secs(row.p50Ms)} (p95 ${secs(row.p95Ms)})`
  if (row.overTimeout > 0.01) {
    return {
      label: `${base} · ${percent(row.overTimeout)} passam de ${Math.round(timeoutMs / 1000)} s`,
      color: 'warning',
    }
  }
  return {
    label: base,
    color: row.p95Ms <= 1500 ? 'success' : 'info',
  }
}

/**
 * Nota explicativa quando a latência do Decisor foi herdada de um modelo de mesmo grafo.
 */
export function deciderLatencyNoteOf(row: DeciderBenchmarkRow): string | null {
  if (!row.latencyFrom) return null
  return `Tempo medido no ${row.latencyFrom} (mesma arquitetura e tamanho; só os pesos mudam).`
}
