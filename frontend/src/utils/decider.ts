import type { AiDeciderSettings } from '@/bindings/AiDeciderSettings'
import type { DeciderBenchmarkView } from '@/bindings/DeciderBenchmarkView'
import type { DeciderModelLanguage } from '@/bindings/DeciderModelLanguage'
import type { DeciderProviderView } from '@/bindings/DeciderProviderView'
import type { DeciderModelSpecView } from '@/bindings/DeciderModelSpecView'
import { formatDecimalBytes, formatElapsedMs, formatPercent } from './formatters'

/**
 * Hosts que significam "o Ollaya roda nesta mesma máquina": o serviço do
 * `docker-compose.yml`, o loopback e os nomes com que um container alcança o
 * próprio host. Qualquer outro endereço é outra máquina — a RAM gasta é dela.
 */
const LOCAL_HOSTS = new Set([
  'ollaya',
  'localhost',
  'host.docker.internal',
  'host.containers.internal',
  '::1',
  '[::1]',
])

/** A URL aponta para um Ollaya que consome a RAM deste servidor? */
export function isLocalOllaya(url: string | null | undefined): boolean {
  const raw = (url ?? '').trim()
  if (!raw) return true
  let host: string
  try {
    host = new URL(raw.includes('://') ? raw : `http://${raw}`).hostname.toLowerCase()
  } catch {
    return false
  }
  return LOCAL_HOSTS.has(host) || host.startsWith('127.')
}

const MB = 1_000_000

/** Quem sugere o tempo máximo por pergunta: um modelo do Ollaya ou um provedor. */
export interface TimeoutSource {
  suggestedTimeoutMs: number
}

/** Um fato da ficha do modelo, já como chip. */
export interface ModelFact {
  label: string
  color: 'success' | 'warning' | 'info' | 'error'
  icon: string
}

/** O teste público publicado em 10/10/2026, de onde vêm os acertos medidos. */
export const BENCHMARK_URL =
  'https://akitaonrails.com/2026/10/10/qual-modelo-de-decisao-classificador-de-ia-tipo-jev-e-melhor/'

/** Abaixo disso o modelo fica perto do chute (~31% no teste) — mesma régua do backend. */
const USEFUL_ACCURACY = 0.6
const GOOD_ACCURACY = 0.85

function percent(value: number): string {
  return formatPercent(value * 100, 0)
}

/** O resultado no teste público: acerto geral e em português, com a sigla para a lista. */
export function benchmarkFact(benchmark: DeciderBenchmarkView): ModelFact & { short: string } {
  let color: ModelFact['color'] = 'error'
  if (benchmark.accuracy >= GOOD_ACCURACY) color = 'success'
  else if (benchmark.accuracy >= USEFUL_ACCURACY) color = 'info'
  return {
    label: `Acerto ${percent(benchmark.accuracy)} · PT ${percent(benchmark.portuguese)}`,
    short: percent(benchmark.accuracy),
    color,
    icon: 'mdi-target',
  }
}

/** Acertou tão pouco no teste público que fica perto de escolher no chute. */
export function isNearChance(
  benchmark: DeciderBenchmarkView | null | undefined
): benchmark is DeciderBenchmarkView {
  return benchmark != null && benchmark.accuracy < USEFUL_ACCURACY
}

/** A língua do modelo, com a sigla curta para a lista. */
export function languageFact(language: DeciderModelLanguage): ModelFact & { short: string } {
  return language === 'multilingual'
    ? { label: 'Multilíngue (100+ línguas)', short: 'PT', color: 'success', icon: 'mdi-translate' }
    : { label: 'Sem português', short: 'EN', color: 'warning', icon: 'mdi-translate-off' }
}

/**
 * O que pesa na escolha: acerto, velocidade, tamanho, memória e se precisa de
 * GPU. O teste público (com português medido) vale antes do número do autor e
 * da língua declarada; sem número publicado, sem chip.
 */
export function modelFacts(
  spec: DeciderModelSpecView | null,
  benchmark?: DeciderBenchmarkView | null
): ModelFact[] {
  const facts: ModelFact[] = []
  if (benchmark) {
    const { label, color, icon } = benchmarkFact(benchmark)
    facts.push({ label, color, icon })
  } else if (spec) {
    const { label, color, icon } = languageFact(spec.language)
    facts.push({ label, color, icon })
    if (spec.accuracy != null) {
      facts.push({
        label: `Acerto ${percent(spec.accuracy)} (teste do autor)`,
        color: 'info',
        icon: 'mdi-target',
      })
    }
  }
  if (!spec) return facts
  facts.push({
    label: `${formatElapsedMs(spec.latencyMs)} por 5 perguntas (${spec.gpuRecommended ? 'RTX 4090' : 'CPU'})`,
    color: 'info',
    icon: 'mdi-timer-outline',
  })
  if (spec.downloadMb) {
    facts.push({
      label: `${formatDecimalBytes(spec.downloadMb * MB)} de download`,
      color: 'info',
      icon: 'mdi-download',
    })
  }
  if (spec.memoryMb) {
    facts.push({
      label: `~${formatDecimalBytes(spec.memoryMb * MB)} de memória`,
      color: 'info',
      icon: 'mdi-memory',
    })
  }
  facts.push(
    spec.gpuRecommended
      ? { label: 'GPU recomendada', color: 'warning', icon: 'mdi-expansion-card-variant' }
      : { label: 'Roda bem em CPU', color: 'success', icon: 'mdi-cpu-64-bit' }
  )
  return facts
}

/**
 * Memória do modelo carregado, para o aviso ao ligar. Sem número publicado,
 * o download é o piso: o modelo não cabe em menos que o próprio arquivo.
 */
export function memoryLabel(spec: DeciderModelSpecView): string {
  if (spec.memoryMb) return `aproximadamente ${formatDecimalBytes(spec.memoryMb * MB)}`
  if (spec.downloadMb) return `pelo menos ${formatDecimalBytes(spec.downloadMb * MB)}`
  return 'alguns GB'
}

/**
 * Tempo máximo por decisão ao trocar de modelo. Valor que o usuário não
 * mexeu (o sugerido do modelo anterior, ou o padrão) segue o sugerido do
 * novo; valor ajustado à mão só sobe, nunca desce — um Kev em CPU com o
 * tempo do Decisor erraria toda pergunta do chat.
 */
export function timeoutForModel(
  current: number,
  previous: TimeoutSource | null | undefined,
  next: TimeoutSource | null | undefined,
  fallback: number
): number {
  if (!next) return current
  const untouched = current === (previous?.suggestedTimeoutMs ?? fallback)
  return untouched ? next.suggestedTimeoutMs : Math.max(current, next.suggestedTimeoutMs)
}

/**
 * O formulário ao trocar de provedor. O modelo é do provedor e volta ao
 * padrão dele; a URL também, a não ser que o usuário a tenha mudado; chave e
 * ID da conta nunca passam de um provedor para outro; o tempo máximo segue a
 * regra de {@link timeoutForModel}.
 */
export function formForProvider(
  form: AiDeciderSettings,
  previous: DeciderProviderView | null,
  previousTimeout: TimeoutSource | null,
  next: DeciderProviderView,
  fallbackTimeout: number
): AiDeciderSettings {
  const url = form.baseUrl.trim()
  const keepUrl = url !== '' && url !== previous?.defaultBaseUrl
  return {
    ...form,
    provider: next.id,
    baseUrl: keepUrl ? form.baseUrl : next.defaultBaseUrl,
    model: next.defaultModel,
    apiKey: undefined,
    accountId: undefined,
    timeoutMs: timeoutForModel(
      Number(form.timeoutMs) || fallbackTimeout,
      previousTimeout,
      next,
      fallbackTimeout
    ),
  }
}

/**
 * O que falta para o provedor responder, em palavras para a tela; `null`
 * quando está tudo lá. A chave mascarada (já gravada) conta como presente.
 */
export function providerMissing(
  form: AiDeciderSettings,
  provider: DeciderProviderView | null
): string | null {
  if (!provider) return null
  const missing: string[] = []
  if (!form.baseUrl.trim() && !provider.defaultBaseUrl) missing.push('a URL')
  if (provider.requiresAccount && !form.accountId?.trim()) missing.push('o ID da conta')
  if (provider.requiresKey && !form.apiKey?.trim()) missing.push('a chave de acesso')
  if (!form.model.trim()) missing.push('o modelo')
  if (!missing.length) return null
  return missing.length === 1
    ? `Falta ${missing[0]}`
    : `Faltam ${missing.slice(0, -1).join(', ')} e ${missing.at(-1)}`
}
