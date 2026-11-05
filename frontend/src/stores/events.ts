import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import { getStoredToken } from '@/utils/authStorage'

export interface RealtimeEventPayload {
  /** Nome do evento publicado pelo backend (ex.: `monitor:result`) */
  type: string
  data: Record<string, unknown>
  timestamp: string
}

/** O que um handler de domínio recebe além do evento. */
export interface SseBus {
  /** Agrupa recargas disparadas em rajada numa chamada só por recurso. */
  scheduleRefresh(key: string, action: () => void): void
}

export type SseHandler = (
  data: Record<string, unknown>,
  payload: RealtimeEventPayload,
  bus: SseBus
) => void

export interface SseRoute {
  handle: SseHandler
  /**
   * Telemetria ao vivo (estado, não notícia): fica fora do feed de eventos.
   * Uma função decide por evento.
   */
  ephemeral?: boolean | ((data: Record<string, unknown>) => boolean)
}

/**
 * Rotas de um domínio: o tipo exato (`alert:triggered`) ou um prefixo
 * terminado em `*` (`docker:*`) → handler.
 */
export type SseRoutes = Record<string, SseHandler | SseRoute>

export interface HourlyDistributionBin {
  label: string
  hour: number
  timestamp: string
  critical: number
  warning: number
  info: number
}

export interface HourlyDistributionTotals {
  critical: number
  warning: number
  info: number
  total: number
}

export interface HourlyDistributionResponse {
  hours: number
  bins: HourlyDistributionBin[]
  totals: HourlyDistributionTotals
}

/** Máximo de eventos mantidos no feed em memória */
const FEED_LIMIT = 50
/** Janela de agrupamento para recargas que não podem ser aplicadas em memória */
const REFRESH_DEBOUNCE_MS = 800
const MAX_RECONNECT_DELAY_MS = 30_000
/** Mudanças de interesse em rajada (troca de página) viram uma reconexão só. */
const INTEREST_DEBOUNCE_MS = 300

interface RegisteredRoute {
  pattern: string
  prefix: boolean
  route: SseRoute
}

function matches(entry: RegisteredRoute, type: string): boolean {
  return entry.prefix ? type.startsWith(entry.pattern) : type === entry.pattern
}

function isEphemeral(entry: RegisteredRoute, data: Record<string, unknown>): boolean {
  const { ephemeral } = entry.route
  return typeof ephemeral === 'function' ? ephemeral(data) : ephemeral === true
}

/**
 * Barramento SSE compartilhado (`GET /api/events/stream`).
 *
 * O barramento não conhece domínio nenhum: cada área registra as rotas dos
 * eventos que lhe interessam ({@link registerRoutes}, ver `src/areas/`) e as
 * telas assinam pontualmente com {@link onEvent}. Área desligada não tem rota
 * registrada — o evento passa sem tocar a store dela.
 */
export const useEventsStore = defineStore('events', () => {
  const isConnected = ref(false)
  const recentEvents = ref<RealtimeEventPayload[]>([])
  const lastEventAt = ref<string | null>(null)

  let eventSource: EventSource | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null
  let reconnectAttempts = 0
  const pendingRefreshes = new Map<string, ReturnType<typeof setTimeout>>()

  type EventHandler = (data: Record<string, unknown>, payload: RealtimeEventPayload) => void
  const handlers = new Map<string, Set<EventHandler>>()
  const routes = new Set<RegisteredRoute>()

  /** Interesses declarados pelas telas abertas (contagem de referências). */
  const interests = new Map<string, number>()
  let connectedInterests = ''
  let interestTimer: ReturnType<typeof setTimeout> | null = null

  const bus: SseBus = { scheduleRefresh }

  /**
   * Assinatura pontual usada por telas que mantêm estado local (gráficos, por
   * exemplo). Retorna a função de cancelamento para usar no `onUnmounted`.
   */
  function onEvent(types: string | string[], handler: EventHandler): () => void {
    const list = Array.isArray(types) ? types : [types]
    for (const type of list) {
      if (!handlers.has(type)) handlers.set(type, new Set())
      handlers.get(type)!.add(handler)
    }

    return () => {
      for (const type of list) handlers.get(type)?.delete(handler)
    }
  }

  /**
   * Registra as rotas de um domínio (tipo exato ou prefixo `x:*`). Rodam antes
   * dos assinantes de {@link onEvent}. Retorna a função que as remove.
   */
  function registerRoutes(table: SseRoutes): () => void {
    const added: RegisteredRoute[] = Object.entries(table).map(([key, value]) => {
      const prefix = key.endsWith('*')
      return {
        pattern: prefix ? key.slice(0, -1) : key,
        prefix,
        route: typeof value === 'function' ? { handle: value } : value,
      }
    })
    for (const entry of added) routes.add(entry)
    return () => {
      for (const entry of added) routes.delete(entry)
    }
  }

  /** Algum domínio registrado trata este tipo de evento? */
  function hasRoute(type: string): boolean {
    for (const entry of routes) if (matches(entry, type)) return true
    return false
  }

  function routesFor(type: string): RegisteredRoute[] {
    return [...routes].filter((entry) => matches(entry, type))
  }

  function notifyHandlers(payload: RealtimeEventPayload) {
    for (const handler of handlers.get(payload.type) ?? []) {
      try {
        handler(payload.data, payload)
      } catch {
        // Um assinante com defeito não pode interromper os demais
      }
    }
    // `monitor:result` já é o evento durável da observação. As métricas
    // viajam no mesmo payload e este alias local mantém a assinatura focada
    // dos gráficos sem duplicar linhas no event_outbox.
    if (payload.type === 'monitor:result' && Array.isArray(payload.data.metrics)) {
      const metricPayload = { ...payload, type: 'metric:recorded' }
      for (const handler of handlers.get('metric:recorded') ?? []) {
        try {
          handler(metricPayload.data, metricPayload)
        } catch {
          // Mantém o isolamento entre assinantes também no alias local.
        }
      }
    }
  }

  /**
   * Agrupa recargas disparadas em rajada (uma varredura pode emitir dezenas de
   * eventos por segundo) em uma única chamada por recurso.
   */
  function scheduleRefresh(key: string, action: () => void) {
    const existing = pendingRefreshes.get(key)
    if (existing) clearTimeout(existing)
    pendingRefreshes.set(
      key,
      setTimeout(() => {
        pendingRefreshes.delete(key)
        action()
      }, REFRESH_DEBOUNCE_MS)
    )
  }

  function interestKey(): string {
    return [...interests.keys()].sort().join(',')
  }

  function streamUrl(): string {
    const params: string[] = []
    const token = getStoredToken()
    if (token) params.push(`token=${encodeURIComponent(token)}`)
    if (connectedInterests) params.push(`interest=${connectedInterests}`)
    return params.length ? `/api/events/stream?${params.join('&')}` : '/api/events/stream'
  }

  function connect() {
    if (eventSource) return

    connectedInterests = interestKey()
    eventSource = new EventSource(streamUrl())

    eventSource.onopen = () => {
      isConnected.value = true
      reconnectAttempts = 0
    }

    eventSource.onmessage = (msg) => {
      try {
        const raw = JSON.parse(msg.data) as Record<string, unknown>
        const payload: RealtimeEventPayload = {
          // `event` é aceito por compatibilidade com payloads antigos
          type: String(raw.type ?? raw.event ?? 'unknown'),
          data: (raw.data as Record<string, unknown>) ?? {},
          timestamp: String(raw.timestamp ?? new Date().toISOString()),
        }
        handleIncomingEvent(payload)
      } catch {
        // Mensagem não parseável — ignora
      }
    }

    eventSource.onerror = () => {
      isConnected.value = false
      closeSource()
      scheduleReconnect()
    }
  }

  function scheduleReconnect() {
    if (reconnectTimer) return
    // Backoff exponencial: 1s, 2s, 4s ... até 30s
    const delay = Math.min(1000 * 2 ** reconnectAttempts, MAX_RECONNECT_DELAY_MS)
    reconnectAttempts += 1
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null
      connect()
    }, delay)
  }

  /**
   * O conjunto de interesses mudou: passada a rajada, reabre o stream **uma
   * vez** com o conjunto novo — só se houver stream aberto e o conjunto for
   * outro. Não é polling: sem mudança, nada acontece.
   */
  function scheduleInterestSync() {
    if (interestTimer) clearTimeout(interestTimer)
    interestTimer = setTimeout(() => {
      interestTimer = null
      if (!eventSource || interestKey() === connectedInterests) return
      closeSource()
      connect()
    }, INTEREST_DEBOUNCE_MS)
  }

  /**
   * Declara que uma tela aberta consome um dado ao vivo sob demanda (ex.:
   * `docker`): o backend só o produz enquanto algum stream o declara.
   * Retorna a função que libera o interesse.
   */
  function acquireInterest(name: string): () => void {
    interests.set(name, (interests.get(name) ?? 0) + 1)
    scheduleInterestSync()
    let released = false
    return () => {
      if (released) return
      released = true
      const count = (interests.get(name) ?? 1) - 1
      if (count > 0) interests.set(name, count)
      else interests.delete(name)
      scheduleInterestSync()
    }
  }

  function handleIncomingEvent(payload: RealtimeEventPayload) {
    lastEventAt.value = payload.timestamp

    const data = payload.data || {}
    const matched = routesFor(payload.type)
    const ephemeral = matched.some((entry) => isEphemeral(entry, data))
    if (payload.type !== 'stream:connected' && !ephemeral) {
      recentEvents.value.unshift(payload)
      if (recentEvents.value.length > FEED_LIMIT) {
        recentEvents.value.length = FEED_LIMIT
      }
    }

    for (const entry of matched) {
      try {
        entry.route.handle(data, payload, bus)
      } catch {
        // Um domínio com defeito não pode interromper os demais
      }
    }

    notifyHandlers(payload)
  }

  function closeSource() {
    if (eventSource) {
      eventSource.close()
      eventSource = null
    }
  }

  function disconnect() {
    closeSource()
    if (reconnectTimer) {
      clearTimeout(reconnectTimer)
      reconnectTimer = null
    }
    if (interestTimer) {
      clearTimeout(interestTimer)
      interestTimer = null
    }
    for (const timer of pendingRefreshes.values()) clearTimeout(timer)
    pendingRefreshes.clear()
    reconnectAttempts = 0
    isConnected.value = false
  }

  async function fetchHourlyDistribution(hours = 6): Promise<HourlyDistributionResponse | null> {
    try {
      return await apiService.get<HourlyDistributionResponse>(
        `/events/hourly-distribution?hours=${hours}`
      )
    } catch {
      return null
    }
  }

  return {
    isConnected,
    recentEvents,
    lastEventAt,
    connect,
    disconnect,
    onEvent,
    registerRoutes,
    hasRoute,
    acquireInterest,
    fetchHourlyDistribution,
  }
})
