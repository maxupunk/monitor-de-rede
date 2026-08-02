import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'

export interface AlertRule {
  id: number
  name: string
  metric: string
  condition: 'gt' | 'gte' | 'lt' | 'lte' | 'eq' | 'neq'
  threshold: number
  severity: 'info' | 'warning' | 'error' | 'critical'
  channels?: string[]
  isEnabled: boolean
}

export interface AlertEvent {
  id: number
  alertRuleId?: number
  deviceId?: number
  monitorId?: number
  severity: 'info' | 'warning' | 'error' | 'critical'
  status: 'active' | 'acknowledged' | 'silenced' | 'resolved'
  title: string
  message: string
  acknowledgedBy?: string
  silencedUntil?: string
  device?: { id: number; name: string }
  createdAt: string
  resolvedAt?: string
}

export const useAlertsStore = defineStore('alerts', () => {
  const activeAlerts = ref<AlertEvent[]>([])
  const alertRules = ref<AlertRule[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchActiveAlerts() {
    loading.value = true
    error.value = null
    try {
      activeAlerts.value = await apiService.get<AlertEvent[]>('/alerts')
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
      const al = activeAlerts.value.find((a) => a.id === alertId)
      if (al) al.status = 'acknowledged'
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao reconhecer alerta'
      return false
    }
  }

  async function silenceAlert(alertId: number, durationMinutes: number): Promise<boolean> {
    try {
      await apiService.post(`/alerts/${alertId}/silence`, { durationMinutes })
      const al = activeAlerts.value.find((a) => a.id === alertId)
      if (al) al.status = 'silenced'
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao silenciar alerta'
      return false
    }
  }

  async function createAlertRule(payload: Partial<AlertRule>): Promise<boolean> {
    try {
      const created = await apiService.post<AlertRule>('/alert-rules', payload)
      alertRules.value.push(created)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao criar regra de alerta'
      return false
    }
  }

  async function updateAlertRule(id: number, payload: Partial<AlertRule>): Promise<boolean> {
    try {
      const updated = await apiService.put<AlertRule>(`/alert-rules/${id}`, payload)
      const index = alertRules.value.findIndex((r) => r.id === id)
      if (index !== -1) alertRules.value[index] = updated
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao atualizar regra'
      return false
    }
  }

  async function deleteAlertRule(id: number): Promise<boolean> {
    try {
      await apiService.delete(`/alert-rules/${id}`)
      alertRules.value = alertRules.value.filter((r) => r.id !== id)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao remover regra'
      return false
    }
  }

  return {
    activeAlerts,
    alertRules,
    loading,
    error,
    fetchActiveAlerts,
    fetchAlertRules,
    acknowledgeAlert,
    silenceAlert,
    createAlertRule,
    updateAlertRule,
    deleteAlertRule,
  }
})
