import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { apiService } from '@/services/apiService'
import type { AlertOperator } from '@/utils/alertPresentation'

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
  /** Faz parte do conjunto básico aplicado por padrão */
  recommended: boolean
  /** Já existe regra equivalente: aplicar de novo não cria duplicata */
  applied: boolean
  ruleId: number | null
}

export interface AlertRuleCatalog {
  categories: Record<string, string>
  templates: AlertRuleTemplate[]
}

export interface CatalogApplicationResult {
  created: AlertRule[]
  skipped: Array<{ key: string; reason: 'already_exists' | 'unknown_template' }>
}

export interface AlertEvent {
  id: number
  alertRuleId?: number | null
  deviceId?: number | null
  monitorId?: number | null
  severity: 'info' | 'warning' | 'error' | 'critical'
  status: 'active' | 'acknowledged' | 'silenced' | 'resolved'
  title: string
  message: string
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

  async function fetchAlertRules() {
    loading.value = true
    error.value = null
    try {
      alertRules.value = await apiService.get<AlertRule[]>('/alert-rules')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar regras de alerta'
    } finally {
      loading.value = false
    }
  }

  /** Catálogo de regras pré-configuradas mantido pelo backend */
  async function fetchRuleCatalog(): Promise<boolean> {
    catalogLoading.value = true
    error.value = null
    try {
      const catalog = await apiService.get<AlertRuleCatalog>('/alert-rules/catalog')
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
  async function applyCatalogRules(keys: string[]): Promise<CatalogApplicationResult | null> {
    catalogLoading.value = true
    error.value = null
    try {
      const result = await apiService.post<CatalogApplicationResult>('/alert-rules/catalog', {
        keys,
      })
      result.created.forEach((rule) => upsertAlertRule(rule))
      await fetchRuleCatalog()
      return result
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao aplicar regras pré-configuradas'
      return null
    } finally {
      catalogLoading.value = false
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
