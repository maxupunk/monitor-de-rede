import type { LayaModelLanguage } from '@/bindings/LayaModelLanguage'
import type { LayaModelSpecView } from '@/bindings/LayaModelSpecView'
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

/** Um fato da ficha do modelo, já como chip. */
export interface ModelFact {
  label: string
  color: 'success' | 'warning' | 'info'
  icon: string
}

/** A língua do modelo, com a sigla curta para a lista. */
export function languageFact(language: LayaModelLanguage): ModelFact & { short: string } {
  return language === 'multilingual'
    ? { label: 'Multilíngue (100+ línguas)', short: 'PT', color: 'success', icon: 'mdi-translate' }
    : { label: 'Sem português', short: 'EN', color: 'warning', icon: 'mdi-translate-off' }
}

/**
 * O que pesa na escolha: língua, acerto, velocidade, tamanho, memória e se
 * precisa de GPU. Só o que o Ollaya publica — sem número, sem chip.
 */
export function modelFacts(spec: LayaModelSpecView): ModelFact[] {
  const { label, color, icon } = languageFact(spec.language)
  const facts: ModelFact[] = [{ label, color, icon }]
  if (spec.accuracy != null) {
    facts.push({
      label: `Acerto ${formatPercent(spec.accuracy * 100, 0)}`,
      color: 'info',
      icon: 'mdi-target',
    })
  }
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
export function memoryLabel(spec: LayaModelSpecView): string {
  if (spec.memoryMb) return `aproximadamente ${formatDecimalBytes(spec.memoryMb * MB)}`
  if (spec.downloadMb) return `pelo menos ${formatDecimalBytes(spec.downloadMb * MB)}`
  return 'alguns GB'
}

/**
 * Tempo máximo por decisão ao trocar de modelo. Valor que o usuário não
 * mexeu (o sugerido do modelo anterior, ou o padrão) segue o sugerido do
 * novo; valor ajustado à mão só sobe, nunca desce — um Kev em CPU com o
 * tempo do Laya erraria toda pergunta do chat.
 */
export function timeoutForModel(
  current: number,
  previous: LayaModelSpecView | null | undefined,
  next: LayaModelSpecView | null | undefined,
  fallback: number
): number {
  if (!next) return current
  const untouched = current === (previous?.suggestedTimeoutMs ?? fallback)
  return untouched ? next.suggestedTimeoutMs : Math.max(current, next.suggestedTimeoutMs)
}
