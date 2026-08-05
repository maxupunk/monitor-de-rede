import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { apiService } from '@/services/apiService'
import type { AlertOperator } from '@/utils/alertPresentation'

export interface AlertRuleCondition {
  field: string
  operator: AlertOperator
  value: number | string
}

export interface AlertRule {
  id: number
  siteId?: number | null
  deviceId?: number | null
  monitorId?: number | null
  name: string
  type: 'device_offline' | 'latency_high' | 'http_failure' | 'tcp_failure' | 'custom'
  condition: AlertRuleCondition
  severity: 'info' | 'warning' | 'critical'
  durationSeconds: number
  enabled: boolean
  isEnabled: boolean
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
  const loading = ref(false)
  const error = ref<string | null>(null)
  /** Marca a última mutação vinda do SSE, útil para feedback visual */
  const lastRealtimeUpdateAt = ref<string | null>(null)

  /** Somente eventos que ainda demandam atenção do operador */
  const activeAlerts = computed(() => alertEvents.value.filter((a) => a.status !== 'resolved'))

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

  async function acknowledgeAlert(alertId: number): Promise<boolean> {
    try {
      await apiService.post(`/alerts/${alertId}/acknowledge`)
      patchAlertEvent(alertId, { status: 'acknowledged' })
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao reconhecer alerta'
      return false
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
    criticalCount,
    alertRules,
    loading,
    error,
    lastRealtimeUpdateAt,
    fetchActiveAlerts,
    fetchAlertRules,
    acknowledgeAlert,
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
