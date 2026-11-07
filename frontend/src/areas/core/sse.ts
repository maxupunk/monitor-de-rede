import { useAuthStore } from '@/stores/auth'
import type { SseRoutes } from '@/stores/events'
import type { AiIncidentSummary } from '@/bindings/AiIncidentSummary'
import type { DeciderTriage } from '@/bindings/DeciderTriage'
import { useAreasStore } from '@/stores/areas'
import { useDevicesStore } from '@/stores/devices'
import { useAlertsStore, type AlertEvent } from '@/stores/alerts'
import { useMonitorsStore } from '@/stores/monitors'
import { useDeviceDetailStore } from '@/stores/deviceDetail'
import { useAgentsStore } from '@/stores/agents'
import { useMaintenanceWindowsStore } from '@/stores/maintenanceWindows'
import { useStoragesStore } from '@/stores/storages'
import { useBackupStore } from '@/stores/backup'
import { isDatabaseJobSnapshot, useDatabasesStore } from '@/stores/databases'
import { useLogsStore } from '@/stores/logs'
import { isScheduledTaskEvent, useScheduledTasksStore } from '@/stores/scheduledTasks'
import { isDeciderModelState, useAiDeciderStore } from '@/stores/aiDecider'
import { asApprovalResolved, useAiApprovalsStore } from '@/stores/aiApprovals'
import { useAiStore } from '@/stores/ai'
import { isLogTemplateInfo } from '@/utils/logCategories'

/** Tarefa agendada: só a falha e o "não rodou" são notícia para o feed. */
function isScheduledTaskNews(data: unknown): boolean {
  return isScheduledTaskEvent(data) && (data.status === 'failed' || data.status === 'skipped')
}

function alertId(data: Record<string, unknown>): number {
  return Number(data.id ?? data.alertEventId)
}

/** Núcleo: monitores, dispositivos, alertas, IA, backup, rotinas e as próprias áreas. */
export const coreSse: SseRoutes = {
  'areas:updated': (data) => useAreasStore().apply(data),

  // O acesso deste usuário mudou (ADR 019): o backend só entrega `user:*` à
  // conexão do próprio usuário.
  'user:access_updated': (data) => useAuthStore().applyAccess(data),

  'monitor:result': (data) => {
    const monitorsStore = useMonitorsStore()
    const deviceDetailStore = useDeviceDetailStore()
    monitorsStore.applyRealtimeResult(data)
    monitorsStore.applyRealtimeMetrics(data)
    deviceDetailStore.applyMonitorResult(data)
    deviceDetailStore.applyRecordedMetrics(data)
  },

  'metric:recorded': (data) => {
    useDeviceDetailStore().applyRecordedMetrics(data)
    useMonitorsStore().applyRealtimeMetrics(data)
  },

  'device:status': (data) => {
    useDevicesStore().applyRealtimeStatus(data)
    useDeviceDetailStore().applyDeviceStatus(data)
  },
  'device:updated': (data) => {
    useDevicesStore().applyRealtimeStatus(data)
    useDeviceDetailStore().applyDeviceStatus(data)
  },

  'interface:traffic': {
    ephemeral: true,
    handle: (data) => {
      useDeviceDetailStore().applyInterfaceTraffic(data)
      useMonitorsStore().applyInterfaceTraffic(data)
    },
  },
  'interface:status_change': (data, payload) =>
    useDeviceDetailStore().applyInterfaceChange(payload.type, data),
  'interface:speed_change': (data, payload) =>
    useDeviceDetailStore().applyInterfaceChange(payload.type, data),
  'interface:speed_downgrade': (data, payload) =>
    useDeviceDetailStore().applyInterfaceChange(payload.type, data),

  'alert:triggered': (data, payload, bus) => {
    const alertsStore = useAlertsStore()
    // Traz o registro completo (com relações) sem esperar refetch geral
    bus.scheduleRefresh('alerts', () => alertsStore.fetchActiveAlerts())
    alertsStore.upsertAlertEvent({
      id: alertId(data),
      alertRuleId: (data.alertRuleId as number) ?? null,
      deviceId: (data.deviceId as number) ?? null,
      monitorId: (data.monitorId as number) ?? null,
      severity: (data.severity as 'info' | 'warning' | 'critical') ?? 'warning',
      status: (data.status as AlertEvent['status']) ?? 'active',
      title: String(data.title ?? data.ruleName ?? 'Novo alerta'),
      message: String(data.message ?? ''),
      data: (data.data as AlertEvent['data']) ?? null,
      startedAt: data.startedAt as string,
      createdAt: String(data.createdAt ?? data.startedAt ?? payload.timestamp),
    })
  },
  'alert:decider_triage': (data) => {
    const triage = data.deciderTriage as DeciderTriage | undefined
    if (triage) useAlertsStore().applyDeciderTriage(alertId(data), triage)
  },
  'alert:ai_summary': (data) => {
    const summary = data.aiSummary as AiIncidentSummary | undefined
    if (summary) useAlertsStore().applyAiSummary(alertId(data), summary)
  },
  // Payload é o evento serializado completo: patch em memória, sem refetch
  'alert:updated': (data) => useAlertsStore().upsertAlertEvent(data as never),
  'alert:resolved': (data) =>
    useAlertsStore().patchAlertEvent(alertId(data), {
      status: 'resolved',
      resolvedAt: data.resolvedAt as string,
    }),
  'alert:acknowledged': (data) =>
    useAlertsStore().patchAlertEvent(alertId(data), {
      status: 'acknowledged',
      silencedUntil: (data.silencedUntil as string) ?? undefined,
    }),
  'alert:silenced': (data) =>
    useAlertsStore().patchAlertEvent(alertId(data), {
      status: 'silenced',
      silencedUntil: (data.silencedUntil as string) ?? undefined,
    }),
  'alert_rule:created': (data) => useAlertsStore().upsertAlertRule(data as never),
  'alert_rule:updated': (data) => useAlertsStore().upsertAlertRule(data as never),
  'alert_rule:deleted': (data) => useAlertsStore().removeAlertRule(Number(data.id)),

  // Agentes remotos também são probes (ADR 011): a tela de agentes acompanha.
  'probe:status': (data) => useAgentsStore().applyRealtimeStatus(data),

  'decider:model_state': (data) => {
    if (isDeciderModelState(data)) useAiDeciderStore().applyModelState(data)
  },
  // O payload é o `LogTemplateInfo` inteiro (backend `syslog::templates::info`).
  'logs:template_classified': (data) => {
    if (isLogTemplateInfo(data)) useLogsStore().applyTemplateClassified(data)
  },

  'maintenance_windows:updated': (_data, _payload, bus) => {
    const maintenanceStore = useMaintenanceWindowsStore()
    bus.scheduleRefresh('maintenance_windows', () => maintenanceStore.fetchWindows())
  },
  'backup_plan:updated': (_data, _payload, bus) => {
    const backupStore = useBackupStore()
    bus.scheduleRefresh('backup_plan', () => backupStore.refreshPlanIfLoaded())
  },
  'storages:updated': (_data, _payload, bus) => {
    const storagesStore = useStoragesStore()
    bus.scheduleRefresh('storages', () => storagesStore.refreshIfLoaded())
  },
  'database_connections:updated': (_data, _payload, bus) => {
    const databasesStore = useDatabasesStore()
    bus.scheduleRefresh('database_connections', () => databasesStore.refreshIfLoaded())
  },
  'database_jobs:updated': (data) => {
    if (isDatabaseJobSnapshot(data)) useDatabasesStore().applyJob(data)
  },

  // Tarefa agendada: o começo e o sucesso de uma tarefa de minuto em minuto
  // encheriam o feed.
  'scheduled_task:updated': {
    ephemeral: (data) => !isScheduledTaskNews(data),
    handle: (data) => useScheduledTasksStore().applyUpdated(data),
  },

  'ai:approval_pending': (data) => useAiApprovalsStore().applyPending(data),
  'ai:approval_resolved': (data) => {
    const resolved = asApprovalResolved(data)
    if (!resolved) return
    useAiApprovalsStore().applyResolved(resolved)
    // Decidido em outra aba: o card da conversa aberta acompanha.
    useAiStore().applyApprovalDecision(resolved)
  },
}
