import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { apiService } from '@/services/apiService'
import type { AlertOperator, AlertProblemKind } from '@/utils/alertPresentation'

export interface AlertRuleCondition {
  field: string
  operator: AlertOperator
  value: number | string
}

export type AlertRuleType =
  'device_offline' | 'latency_high' | 'http_failure' | 'tcp_failure' | 'custom'

export interface AlertRule {
  id: number
  siteId?: number | null
  deviceId?: number | null
  monitorId?: number | null
  name: string
  type: AlertRuleType
  /** Item do catálogo que originou a regra; nulo quando criada à mão */
  templateKey?: string | null
  condition: AlertRuleCondition
  severity: 'info' | 'warning' | 'critical'
  durationSeconds: number
  /** Janela de estabilização antes de resolver; 0 = resolve na 1ª checagem ok */
  recoveryWindowSeconds?: number
  /** Recaídas na janela que declaram o alvo oscilando; 0 = detecção desligada */
  flapThreshold?: number
  /** Largura da janela deslizante em que as recaídas são contadas */
  flapWindowSeconds?: number
  /** Intervalo mínimo entre notificações do alvo, mesmo se o alerta reabrir */
  notificationCooldownSeconds?: number
  /** Não notificar quando o equipamento-pai já está em alerta */
  inhibitWhenParentDown?: boolean
  enabled: boolean
  isEnabled: boolean
}

/** Regra pré-configurada do catálogo, já marcada com o que existe no banco */
export interface AlertRuleTemplate {
  key: string
  name: string
  description: string
  category: string
  type: AlertRuleType
  condition: AlertRuleCondition
  severity: AlertRule['severity']
  durationSeconds: number
  /** Janela de estabilização sugerida pelo template (0 = resolução imediata) */
  recoveryWindowSeconds?: number
  /** Limiar de oscilação sugerido pelo template (0 = detecção desligada) */
  flapThreshold?: number
  flapWindowSeconds?: number
  /** Intervalo mínimo entre notificações sugerido pelo template */
  notificationCooldownSeconds?: number
  /** O template silencia o alvo quando o equipamento-pai está em alerta */
  inhibitWhenParentDown?: boolean
  /** Faz parte do conjunto básico aplicado por padrão */
  recommended: boolean
  /** Já existe regra equivalente: aplicar de novo não cria duplicata */
  applied: boolean
  ruleId: number | null
  /**
   * O dispositivo do escopo publica o campo que a condicao compara.
   *
   * Sempre `true` no catalogo global — la ainda nao ha dispositivo escolhido.
   * Opcional para tolerar um backend anterior a esta versao.
   */
  applicable?: boolean
}

export interface AlertRuleCatalog {
  categories: Record<string, string>
  templates: AlertRuleTemplate[]
}

/**
 * Onde uma regra do catalogo nasce.
 *
 * Sem escopo, o catalogo e global — e como `/alerts` se comporta antes de o
 * operador escolher um dispositivo. Abrir o catalogo pela pagina do
 * dispositivo ja preenche `deviceId`, e a partir dai "ja configurada" passa a
 * significar "ja configurada **para este dispositivo**": sem isso, aplicar o
 * mesmo template a um segundo equipamento era recusado em silencio.
 */
export interface AlertRuleScope {
  siteId?: number | null
  deviceId?: number | null
  monitorId?: number | null
  /**
   * Junta ao recorte as regras globais — as que valem para todo o inventário
   * e, por isso, também para este dispositivo. Sem isto, uma regra global
   * criada de dentro do equipamento sumiria da aba em que nasceu.
   */
  includeGlobal?: boolean
}

/** Converte o escopo em query string, omitindo o que nao foi informado. */
function scopeQuery(scope?: AlertRuleScope): string {
  if (!scope) return ''
  const params = new URLSearchParams()
  if (scope.siteId != null) params.set('siteId', String(scope.siteId))
  if (scope.deviceId != null) params.set('deviceId', String(scope.deviceId))
  if (scope.monitorId != null) params.set('monitorId', String(scope.monitorId))
  if (scope.includeGlobal) params.set('includeGlobal', 'true')
  const texto = params.toString()
  return texto ? `?${texto}` : ''
}

export interface CatalogApplicationResult {
  created: AlertRule[]
  skipped: Array<{ key: string; reason: 'already_exists' | 'unknown_template' }>
}

/** Metadados do episódio trafegados no JSON `data` do evento (extensível) */
export interface AlertEventData {
  /** ISO do último problema; cada recaída reinicia a janela de estabilização */
  lastProblemAt?: string
  /** Quantas recaídas o episódio teve desde a abertura */
  recurrenceCount?: number
  /** ISO até quando o evento está silenciado */
  silencedUntil?: string
  /** Tipo do problema que abriu o episódio; valores novos são tolerados */
  problemKind?: AlertProblemKind
  /** ISO de quando o episódio foi declarado oscilante (estado `flapping`) */
  flappingSince?: string
  /** Carimbos ISO das recaídas dentro da janela de detecção de oscilação */
  problemTimeline?: string[]
  [key: string]: unknown
}

/** Quanto um alvo oscilou na janela consultada (`GET /alerts/instability`) */
export interface ScopeInstability {
  /** `monitor:12`, `interface:34`, `vpn_peer:7` */
  scopeKey: string
  /** Quedas na janela: episódios abertos + recaídas acumuladas */
  oscillations: number
  episodes: number
  /** O alvo está declarado oscilante agora */
  flapping: boolean
  lastProblemAt?: string | null
}

/** Evento correlacionado retornado pela análise de causa raiz. */
export interface CorrelatedAlertEvent {
  id: number
  title: string
  deviceId?: number | null
  monitorId?: number | null
  severity: AlertEvent['severity']
  status: AlertEvent['status']
  message?: string | null
  startedAt?: string
}

/** Resultado da correlação temporal de alertas em cascata. */
export interface AlertCorrelation {
  windowSeconds: number
  primaryCause: CorrelatedAlertEvent | null
  relatedEvents: CorrelatedAlertEvent[]
  commonSiteId?: number | null
  commonNetworkId?: number | null
  correlationCount: number
}

export interface AlertEvent {
  id: number
  alertRuleId?: number | null
  deviceId?: number | null
  monitorId?: number | null
  severity: 'info' | 'warning' | 'error' | 'critical'
  /**
   * `recovering` e `flapping` são estados abertos: o primeiro é "voltou, mas
   * ainda dentro da janela de estabilização"; o segundo, "cai e volta demais".
   */
  status: 'active' | 'acknowledged' | 'silenced' | 'recovering' | 'flapping' | 'resolved'
  title: string
  message: string
  data?: AlertEventData | null
  acknowledgedBy?: string
  silencedUntil?: string
  device?: { id: number; name: string } | null
  monitor?: { id: number; name: string } | null
  startedAt?: string
  createdAt: string
  resolvedAt?: string | null
}

export const useAlertsStore = defineStore('alerts', () => {
  const alertEvents = ref<AlertEvent[]>([])
  const alertRules = ref<AlertRule[]>([])
  const ruleTemplates = ref<AlertRuleTemplate[]>([])
  const ruleCategories = ref<Record<string, string>>({})
  const catalogLoading = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)
  /** Marca a última mutação vinda do SSE, útil para feedback visual */
  const lastRealtimeUpdateAt = ref<string | null>(null)

  /** Somente eventos que ainda demandam atenção do operador */
  const activeAlerts = computed(() => alertEvents.value.filter((a) => a.status !== 'resolved'))

  /** Alertas ativos ainda não reconhecidos nem silenciados */
  const pendingAlerts = computed(() => alertEvents.value.filter((a) => a.status === 'active'))

  /** Alertas reconhecidos pelo operador */
  const acknowledgedAlerts = computed(() =>
    alertEvents.value.filter((a) => a.status === 'acknowledged')
  )

  /** Alertas que foram resolvidos */
  const resolvedAlerts = computed(() => alertEvents.value.filter((a) => a.status === 'resolved'))

  const criticalCount = computed(
    () =>
      activeAlerts.value.filter((a) => a.severity === 'critical' || a.severity === 'error').length
  )

  async function fetchActiveAlerts() {
    loading.value = true
    error.value = null
    try {
      alertEvents.value = await apiService.get<AlertEvent[]>('/alerts')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar alertas'
    } finally {
      loading.value = false
    }
  }

  /**
   * As regras, opcionalmente recortadas por escopo.
   *
   * Sem escopo e a lista da Central de Alertas — a fonte unica da verdade. Com
   * `deviceId`, e a mesma lista filtrada, e **nao** um segundo cadastro: a
   * regra que aparece aqui e a mesma linha, com o mesmo `id`, que aparece la.
   */
  async function fetchAlertRules(scope?: AlertRuleScope) {
    loading.value = true
    error.value = null
    try {
      alertRules.value = await apiService.get<AlertRule[]>(`/alert-rules${scopeQuery(scope)}`)
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar regras de alerta'
    } finally {
      loading.value = false
    }
  }

  /** Catálogo de regras pré-configuradas mantido pelo backend */
  async function fetchRuleCatalog(scope?: AlertRuleScope): Promise<boolean> {
    catalogLoading.value = true
    error.value = null
    try {
      const catalog = await apiService.get<AlertRuleCatalog>(
        `/alert-rules/catalog${scopeQuery(scope)}`
      )
      ruleTemplates.value = catalog.templates ?? []
      ruleCategories.value = catalog.categories ?? {}
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar o catálogo de regras'
      return false
    } finally {
      catalogLoading.value = false
    }
  }

  /**
   * Aplica as regras escolhidas. O backend é idempotente: chaves já
   * configuradas voltam em `skipped` em vez de virar regra duplicada.
   */
  async function applyCatalogRules(
    keys: string[],
    scope?: AlertRuleScope
  ): Promise<CatalogApplicationResult | null> {
    catalogLoading.value = true
    error.value = null
    try {
      const result = await apiService.post<CatalogApplicationResult>('/alert-rules/catalog', {
        keys,
        siteId: scope?.siteId ?? null,
        deviceId: scope?.deviceId ?? null,
        monitorId: scope?.monitorId ?? null,
      })
      result.created.forEach((rule) => upsertAlertRule(rule))
      await fetchRuleCatalog(scope)
      return result
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao aplicar regras pré-configuradas'
      return null
    } finally {
      catalogLoading.value = false
    }
  }

  /**
   * Histórico de oscilação por alvo ("este link oscilou 12x nas últimas 24h").
   *
   * Sem `scopeKey` devolve o ranking geral; com ele, só o alvo pedido. Falha de
   * rede devolve lista vazia: é um indicador de apoio, e derrubar a página do
   * monitor por causa dele seria desproporcional.
   */
  async function fetchInstability(
    options: { scopeKey?: string; hours?: number } = {}
  ): Promise<ScopeInstability[]> {
    const params = new URLSearchParams()
    if (options.scopeKey) params.set('scopeKey', options.scopeKey)
    if (options.hours) params.set('hours', String(options.hours))
    const query = params.toString()
    try {
      return await apiService.get<ScopeInstability[]>(
        `/alerts/instability${query ? `?${query}` : ''}`
      )
    } catch {
      return []
    }
  }

  /**
   * Analisa a correlação temporal de um alerta.
   *
   * Devolve o evento mais provável de ser a causa raiz comum e os eventos
   * relacionados numa janela curta. Falhas de rede devolvem estrutura vazia
   * para não travar a tela.
   */
  async function fetchCorrelation(alertId: number): Promise<AlertCorrelation | null> {
    try {
      return await apiService.get<AlertCorrelation>(`/alerts/${alertId}/correlation`)
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Erro ao analisar correlação'
      error.value = msg
      return null
    }
  }

  async function acknowledgeAlert(
    alertId: number
  ): Promise<{ success: boolean; resolved?: boolean; message?: string }> {
    try {
      const res = await apiService.post<{
        event?: AlertEvent
        resolved?: boolean
        message?: string
      }>(`/alerts/${alertId}/acknowledge`)
      if (res.event) {
        upsertAlertEvent(res.event)
      } else {
        patchAlertEvent(alertId, { status: res.resolved ? 'resolved' : 'acknowledged' })
      }
      return { success: true, resolved: res.resolved, message: res.message }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Erro ao reconhecer alerta'
      error.value = msg
      return { success: false, message: msg }
    }
  }

  async function verifyAlert(
    alertId: number
  ): Promise<{ success: boolean; resolved?: boolean; message?: string }> {
    try {
      const res = await apiService.post<{
        event?: AlertEvent
        resolved?: boolean
        message?: string
      }>(`/alerts/${alertId}/verify`)
      if (res.event) {
        upsertAlertEvent(res.event)
      }
      return { success: true, resolved: res.resolved, message: res.message }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Erro ao verificar alerta'
      error.value = msg
      return { success: false, message: msg }
    }
  }

  async function verifyAllAlerts(): Promise<{
    success: boolean
    resolvedCount?: number
    message?: string
  }> {
    loading.value = true
    try {
      const res = await apiService.post<{ message: string; resolvedCount: number }>(
        '/alerts/verify-all'
      )
      await fetchActiveAlerts()
      return { success: true, resolvedCount: res.resolvedCount, message: res.message }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Erro ao verificar alertas'
      error.value = msg
      return { success: false, message: msg }
    } finally {
      loading.value = false
    }
  }

  async function silenceAlert(alertId: number, durationMinutes: number): Promise<boolean> {
    try {
      await apiService.post(`/alerts/${alertId}/silence`, {
        minutes: durationMinutes,
        durationMinutes,
      })
      patchAlertEvent(alertId, { status: 'silenced' })
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao silenciar alerta'
      return false
    }
  }

  async function createAlertRule(payload: Partial<AlertRule>): Promise<boolean> {
    try {
      const created = await apiService.post<AlertRule>('/alert-rules', payload)
      upsertAlertRule(created)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao criar regra de alerta'
      return false
    }
  }

  async function updateAlertRule(id: number, payload: Partial<AlertRule>): Promise<boolean> {
    try {
      const updated = await apiService.put<AlertRule>(`/alert-rules/${id}`, payload)
      upsertAlertRule(updated)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar regra'
      return false
    }
  }

  async function deleteAlertRule(id: number): Promise<boolean> {
    try {
      await apiService.delete(`/alert-rules/${id}`)
      removeAlertRule(id)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao remover regra'
      return false
    }
  }

  // --- Mutações aplicadas pelo fluxo SSE (sem refetch da lista inteira) ---

  function upsertAlertEvent(event: AlertEvent) {
    const index = alertEvents.value.findIndex((a) => a.id === event.id)
    if (index === -1) {
      alertEvents.value.unshift(event)
    } else {
      alertEvents.value[index] = { ...alertEvents.value[index], ...event }
    }
    lastRealtimeUpdateAt.value = new Date().toISOString()
  }

  function patchAlertEvent(id: number, patch: Partial<AlertEvent>) {
    const current = alertEvents.value.find((a) => a.id === id)
    if (!current) return
    Object.assign(current, patch)
    lastRealtimeUpdateAt.value = new Date().toISOString()
  }

  function upsertAlertRule(rule: AlertRule) {
    const normalized: AlertRule = { ...rule, isEnabled: rule.isEnabled ?? rule.enabled }
    const index = alertRules.value.findIndex((r) => r.id === normalized.id)
    if (index === -1) {
      alertRules.value.push(normalized)
    } else {
      alertRules.value[index] = normalized
    }
    lastRealtimeUpdateAt.value = new Date().toISOString()
  }

  function removeAlertRule(id: number) {
    alertRules.value = alertRules.value.filter((r) => r.id !== id)
    lastRealtimeUpdateAt.value = new Date().toISOString()
  }

  return {
    alertEvents,
    activeAlerts,
    pendingAlerts,
    acknowledgedAlerts,
    resolvedAlerts,
    criticalCount,
    alertRules,
    ruleTemplates,
    ruleCategories,
    catalogLoading,
    loading,
    error,
    lastRealtimeUpdateAt,
    fetchActiveAlerts,
    fetchAlertRules,
    fetchRuleCatalog,
    applyCatalogRules,
    fetchInstability,
    fetchCorrelation,
    acknowledgeAlert,
    verifyAlert,
    verifyAllAlerts,
    silenceAlert,
    createAlertRule,
    updateAlertRule,
    deleteAlertRule,
    upsertAlertEvent,
    patchAlertEvent,
    upsertAlertRule,
    removeAlertRule,
  }
})
