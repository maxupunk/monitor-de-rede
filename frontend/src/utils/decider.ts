import type { AiDeciderSettings } from '@/bindings/AiDeciderSettings'
import type { DeciderDecisionsScoreView } from '@/bindings/DeciderDecisionsScoreView'
import type { DeciderModelLanguage } from '@/bindings/DeciderModelLanguage'
import type { DeciderProviderView } from '@/bindings/DeciderProviderView'
import type { DeciderModelSpecView } from '@/bindings/DeciderModelSpecView'
import { formatDecimalBytes, formatLatency, formatPercent } from './formatters'

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

/** O recomendado (`recommended` no catálogo do backend) e o padrão. */
export const RECOMMENDED_MODEL = 'netmonitor/decima-network:v11'

/** Abaixo disso o modelo fica perto do chute nas decisões do NetMonitor. */
const USEFUL_ACCURACY = 0.6
const GOOD_ACCURACY = 0.85

export function percent(value: number): string {
  return formatPercent(value * 100, 0)
}

/** A cor de uma nota (0–1): verde a partir de 85%, azul acima do chute, vermelho perto dele. */
export function accuracyColor(value: number): ModelFact['color'] {
  if (value >= GOOD_ACCURACY) return 'success'
  if (value >= USEFUL_ACCURACY) return 'info'
  return 'error'
}

/** A nota no benchmark das decisões do NetMonitor, com a sigla para a lista. */
export function decisionsFact(score: DeciderDecisionsScoreView): ModelFact & { short: string } {
  return {
    label: `NetMonitor ${percent(score.overall)} · ${formatLatency(score.p95Ms)} (p95, CPU)`,
    short: percent(score.overall),
    color: accuracyColor(score.overall),
    icon: 'mdi-podium',
  }
}

/** A língua do modelo, com a sigla curta para a lista. */
export function languageFact(language: DeciderModelLanguage): ModelFact & { short: string } {
  return language === 'multilingual'
    ? { label: 'Multilíngue (100+ línguas)', short: 'PT', color: 'success', icon: 'mdi-translate' }
    : { label: 'Sem português', short: 'EN', color: 'warning', icon: 'mdi-translate-off' }
}

/**
 * O que pesa na escolha: a nota e a latência no benchmark das decisões do
 * NetMonitor (a única fonte de números), a língua e o tamanho do download.
 */
export function modelFacts(
  spec: DeciderModelSpecView | null,
  decisions?: DeciderDecisionsScoreView | null
): ModelFact[] {
  const facts: ModelFact[] = []
  if (decisions) {
    const { label, color, icon } = decisionsFact(decisions)
    facts.push({ label, color, icon })
  }
  if (!spec) return facts
  const { label, color, icon } = languageFact(spec.language)
  facts.push({ label, color, icon })
  if (spec.downloadMb) {
    facts.push({
      label: `${formatDecimalBytes(spec.downloadMb * MB)} de download`,
      color: 'info',
      icon: 'mdi-download',
    })
  }
  return facts
}

/**
 * Memória do modelo carregado, para o aviso ao ligar: o download é o piso —
 * o modelo não cabe em menos que o próprio arquivo.
 */
export function memoryLabel(spec: DeciderModelSpecView): string {
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

/** Tira espaços e a barra final, como o backend grava. */
function normalizedUrl(url: string | null | undefined): string {
  return (url ?? '').trim().replace(/\/+$/, '')
}

/**
 * A URL foge do padrão do provedor? Vazia não foge: o backend usa o padrão.
 * O "Outro" não tem padrão — qualquer URL é a dele.
 */
export function usesCustomUrl(
  form: AiDeciderSettings,
  provider: DeciderProviderView | null
): boolean {
  const url = normalizedUrl(form.baseUrl)
  const standard = normalizedUrl(provider?.defaultBaseUrl)
  return url !== '' && standard !== '' && url !== standard
}

/**
 * O formulário ao trocar de provedor. Modelo e URL são do provedor: voltam
 * ao padrão dele, ou à URL que o usuário já tinha digitado para ele nesta
 * tela (`rememberedUrl`) — nunca passam de um provedor para outro, senão a
 * URL do Ollaya iria parar no OpenRouter. Chave e ID da conta também não
 * passam; o tempo máximo segue a regra de {@link timeoutForModel}.
 */
export function formForProvider(
  form: AiDeciderSettings,
  previousTimeout: TimeoutSource | null,
  next: DeciderProviderView,
  fallbackTimeout: number,
  rememberedUrl?: string
): AiDeciderSettings {
  return {
    ...form,
    provider: next.id,
    baseUrl: rememberedUrl?.trim() || next.defaultBaseUrl,
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
