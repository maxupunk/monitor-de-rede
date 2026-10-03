import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { ApiError, apiService } from '@/services/apiService'
import { usePluginAppsStore } from './pluginApps'
import type { AccessRequest } from '@/bindings/AccessRequest'
import type { AutoAcceptState } from '@/bindings/AutoAcceptState'
import type { CredentialInput } from '@/bindings/CredentialInput'
import type { CredentialView } from '@/bindings/CredentialView'
import type { DevicePluginsView } from '@/bindings/DevicePluginsView'
import type { PluginDetail } from '@/bindings/PluginDetail'
import type { PluginPackage } from '@/bindings/PluginPackage'
import type { PluginPreview } from '@/bindings/PluginPreview'
import type { PluginRunView } from '@/bindings/PluginRunView'
import type { PluginSummary } from '@/bindings/PluginSummary'
import type { RunStarted } from '@/bindings/RunStarted'
import type { TestReport } from '@/bindings/TestReport'
import type { TranscriptEntry } from '@/bindings/TranscriptEntry'
import type { TransportKind } from '@/bindings/TransportKind'

/** Pedido de aprovação de um acesso, publicado pelo SSE (`plugin:approval_required`). */
export interface PluginApprovalRequest {
  key: string
  runId: number
  deviceId: number
  deviceName: string
  pluginName: string
  action: string
  request: AccessRequest
  expiresAt: string
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isRunView(value: unknown): value is PluginRunView {
  return isRecord(value) && typeof value.id === 'number' && typeof value.status === 'string'
}

function isApprovalRequest(value: unknown): value is PluginApprovalRequest {
  return (
    isRecord(value) &&
    typeof value.key === 'string' &&
    typeof value.runId === 'number' &&
    isRecord(value.request)
  )
}

function describe(err: unknown, fallback: string): string {
  if (err instanceof Error) return err.message
  return fallback
}

/** Quantas execuções a aba mantém em memória. */
const RUNS_LIMIT = 20

export const usePluginsStore = defineStore('plugins', () => {
  const library = ref<PluginSummary[]>([])
  const libraryLoading = ref(false)
  const deviceViews = ref<Record<number, DevicePluginsView>>({})
  const deviceLoading = ref(false)
  const pendingApprovals = ref<PluginApprovalRequest[]>([])
  const error = ref<string | null>(null)

  const nextApproval = computed(() => pendingApprovals.value[0] ?? null)

  // --- Biblioteca -----------------------------------------------------------

  async function fetchLibrary(): Promise<void> {
    libraryLoading.value = true
    error.value = null
    try {
      library.value = await apiService.get<PluginSummary[]>('/plugins')
    } catch (err: unknown) {
      error.value = describe(err, 'Falha ao carregar os plugins')
    } finally {
      libraryLoading.value = false
    }
  }

  function fetchDetail(id: number): Promise<PluginDetail> {
    return apiService.get<PluginDetail>(`/plugins/${id}`)
  }

  function upsertSummary(summary: PluginSummary) {
    const index = library.value.findIndex((item) => item.id === summary.id)
    if (index >= 0) library.value.splice(index, 1, summary)
    else library.value.push(summary)
  }

  async function afterChange(
    detail: PluginDetail,
    deviceId?: number | null
  ): Promise<PluginDetail> {
    upsertSummary(detail.summary)
    if (deviceId) await loadDevice(deviceId)
    return detail
  }

  async function createPlugin(
    pkg: PluginPackage,
    deviceId: number | null,
    refreshDevice?: number | null
  ): Promise<PluginDetail> {
    const detail = await apiService.post<PluginDetail>('/plugins', { package: pkg, deviceId })
    return afterChange(detail, refreshDevice)
  }

  async function updatePlugin(
    id: number,
    pkg: PluginPackage,
    refreshDevice?: number | null
  ): Promise<PluginDetail> {
    const detail = await apiService.put<PluginDetail>(`/plugins/${id}`, { package: pkg })
    return afterChange(detail, refreshDevice)
  }

  async function importPlugin(
    pkg: PluginPackage,
    deviceId: number | null,
    refreshDevice?: number | null
  ): Promise<PluginDetail> {
    const detail = await apiService.post<PluginDetail>('/plugins/import', {
      package: pkg,
      deviceId,
    })
    return afterChange(detail, refreshDevice)
  }

  async function deletePlugin(id: number, refreshDevice?: number | null): Promise<void> {
    await apiService.delete(`/plugins/${id}`)
    library.value = library.value.filter((item) => item.id !== id)
    if (refreshDevice) await loadDevice(refreshDevice)
  }

  async function exportPlugin(summary: PluginSummary): Promise<void> {
    const blob = await apiService.download(`/plugins/${summary.id}/export`)
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `${summary.slug}-${summary.version}.nmplugin.json`
    link.click()
    URL.revokeObjectURL(url)
  }

  async function lifecycle(
    id: number,
    step: 'review' | 'promote' | 'enable' | 'disable' | 'duplicate',
    refreshDevice?: number | null
  ): Promise<PluginDetail> {
    const detail = await apiService.post<PluginDetail>(`/plugins/${id}/${step}`, {})
    return afterChange(detail, refreshDevice)
  }

  async function acceptReview(
    id: number,
    acknowledgeRisk: boolean,
    refreshDevice?: number | null
  ): Promise<PluginDetail> {
    const detail = await apiService.post<PluginDetail>(`/plugins/${id}/accept-review`, {
      acknowledgeRisk,
    })
    return afterChange(detail, refreshDevice)
  }

  async function runTests(id: number, refreshDevice?: number | null): Promise<TestReport> {
    const report = await apiService.post<TestReport>(
      `/plugins/${id}/test`,
      {},
      { timeoutMs: 60_000 }
    )
    const detail = await fetchDetail(id)
    await afterChange(detail, refreshDevice)
    return report
  }

  /** A prévia do editor: os testes do pacote que está na tela, sem gravar nada. */
  async function previewPackage(pkg: PluginPackage): Promise<PluginPreview> {
    return apiService.post<PluginPreview>(
      '/plugins/preview',
      { package: pkg },
      { timeoutMs: 60_000 }
    )
  }

  // --- Aba do equipamento ---------------------------------------------------

  /** Carrega a aba (ação do usuário ao abri-la). O que muda depois chega pelo SSE. */
  async function loadDevice(deviceId: number): Promise<void> {
    deviceLoading.value = true
    error.value = null
    try {
      deviceViews.value[deviceId] = await apiService.get<DevicePluginsView>(
        `/devices/${deviceId}/plugins`
      )
    } catch (err: unknown) {
      error.value = describe(err, 'Falha ao carregar os plugins do dispositivo')
    } finally {
      deviceLoading.value = false
    }
  }

  function replaceCredential(deviceId: number, credential: CredentialView) {
    const view = deviceViews.value[deviceId]
    if (!view) return
    const others = view.credentials.filter((item) => item.kind !== credential.kind)
    view.credentials = [...others, credential].sort((a, b) => a.kind.localeCompare(b.kind))
  }

  async function saveCredential(deviceId: number, input: CredentialInput): Promise<CredentialView> {
    const saved = await apiService.put<CredentialView>(`/devices/${deviceId}/credentials`, input)
    replaceCredential(deviceId, saved)
    return saved
  }

  async function deleteCredential(deviceId: number, kind: TransportKind): Promise<void> {
    await apiService.delete(`/devices/${deviceId}/credentials/${kind}`)
    const view = deviceViews.value[deviceId]
    if (view) view.credentials = view.credentials.filter((item) => item.kind !== kind)
  }

  async function openSession(deviceId: number, kind: TransportKind, secret: string) {
    const saved = await apiService.post<CredentialView>(
      `/devices/${deviceId}/credentials/${kind}/session`,
      { secret }
    )
    replaceCredential(deviceId, saved)
  }

  /** Instala o plugin no equipamento: ele ganha a própria aba. */
  async function installPlugin(deviceId: number, pluginId: number): Promise<void> {
    deviceViews.value[deviceId] = await apiService.post<DevicePluginsView>(
      `/devices/${deviceId}/plugins/${pluginId}/install`,
      {}
    )
    // Instalar um desligado o liga: um aplicativo novo pode aparecer no menu.
    await usePluginAppsStore().fetchApps()
  }

  async function uninstallPlugin(deviceId: number, pluginId: number): Promise<void> {
    deviceViews.value[deviceId] = await apiService.delete<DevicePluginsView>(
      `/devices/${deviceId}/plugins/${pluginId}/install`
    )
  }

  /** Salva o ajuste deste equipamento (configuração `device` do plugin). */
  async function saveDeviceSettings(
    deviceId: number,
    pluginId: number,
    value: Record<string, unknown>
  ): Promise<Record<string, unknown>> {
    const saved = await apiService.put<Record<string, unknown>>(
      `/devices/${deviceId}/plugins/${pluginId}/settings`,
      { value }
    )
    const item = deviceViews.value[deviceId]?.plugins.find((entry) => entry.plugin.id === pluginId)
    if (item) item.deviceSettings = saved
    return saved
  }

  /** A execução como o SSE a deixou (início, acessos, fim). */
  function runById(deviceId: number, runId: number | null): PluginRunView | null {
    if (runId === null) return null
    return deviceViews.value[deviceId]?.runs.find((run) => run.id === runId) ?? null
  }

  async function runAction(
    deviceId: number,
    pluginId: number,
    action: string,
    params: Record<string, unknown>,
    confirmWrite: boolean
  ): Promise<number> {
    const started = await apiService.post<RunStarted>(
      `/devices/${deviceId}/plugins/${pluginId}/actions/${encodeURIComponent(action)}`,
      { params, confirmWrite }
    )
    return started.runId
  }

  async function validatePlugin(deviceId: number, pluginId: number): Promise<number> {
    const started = await apiService.post<RunStarted>(
      `/devices/${deviceId}/plugins/${pluginId}/validate`,
      {}
    )
    return started.runId
  }

  async function cancelRun(runId: number): Promise<void> {
    try {
      await apiService.post(`/plugin-runs/${runId}/cancel`, {})
    } catch (err: unknown) {
      // 404: a execução já tinha terminado — nada a cancelar.
      if (!(err instanceof ApiError && err.status === 404)) throw err
    }
  }

  // --- Aprovações -------------------------------------------------------------

  async function respondApproval(key: string, approved: boolean): Promise<void> {
    try {
      await apiService.post(`/plugin-approvals/${encodeURIComponent(key)}`, { approved })
    } finally {
      pendingApprovals.value = pendingApprovals.value.filter((item) => item.key !== key)
    }
  }

  // --- Modo automático da IA ----------------------------------------------------

  function autoAcceptState(conversationKey: string, deviceId: number): Promise<AutoAcceptState> {
    const query = new URLSearchParams({ conversationKey, deviceId: String(deviceId) })
    return apiService.get<AutoAcceptState>(`/plugin-auto-accept?${query.toString()}`)
  }

  function enableAutoAccept(conversationKey: string, deviceId: number): Promise<AutoAcceptState> {
    return apiService.post<AutoAcceptState>('/plugin-auto-accept', {
      conversationKey,
      deviceId,
      acceptTerms: true,
    })
  }

  function disableAutoAccept(conversationKey: string, deviceId: number): Promise<AutoAcceptState> {
    return apiService.delete<AutoAcceptState>('/plugin-auto-accept', { conversationKey, deviceId })
  }

  /** Execuções em andamento de um equipamento (para o "Parar" do modo automático). */
  function runningRuns(deviceId: number): PluginRunView[] {
    return (deviceViews.value[deviceId]?.runs ?? []).filter((run) => run.status === 'running')
  }

  // --- SSE ----------------------------------------------------------------------

  function upsertRun(run: PluginRunView) {
    const view = deviceViews.value[run.deviceId]
    if (!view) return
    const index = view.runs.findIndex((item) => item.id === run.id)
    if (index >= 0) view.runs.splice(index, 1, run)
    else view.runs = [run, ...view.runs].slice(0, RUNS_LIMIT)
  }

  function applyRunStarted(data: Record<string, unknown>) {
    if (isRunView(data.run)) upsertRun(data.run)
  }

  function applyRunOutput(data: Record<string, unknown>) {
    const runId = data.runId
    const deviceId = data.deviceId
    if (typeof runId !== 'number' || typeof deviceId !== 'number' || !isRecord(data.entry)) return
    const run = deviceViews.value[deviceId]?.runs.find((item) => item.id === runId)
    if (!run) return
    const entry = data.entry as TranscriptEntry
    if (!run.transcript.some((item) => item.seq === entry.seq)) {
      run.transcript = [...run.transcript, entry]
    }
  }

  function applyRunFinished(data: Record<string, unknown>) {
    if (!isRunView(data.run)) return
    const finished = data.run
    const view = deviceViews.value[finished.deviceId]
    const previous = view?.runs.find((item) => item.id === finished.id)
    // O nome do plugin só vem no início; o fim pode chegar sem ele.
    upsertRun({ ...finished, pluginName: finished.pluginName ?? previous?.pluginName ?? null })
    // O `detect` pode ter lido o firmware: a compatibilidade depende dele.
    if (view && finished.status === 'succeeded') {
      const firmware = finished.output
      if (isRecord(firmware) && typeof firmware.firmware === 'string') {
        view.firmware = firmware.firmware
      }
    }
  }

  function applyApprovalRequired(data: Record<string, unknown>) {
    if (!isApprovalRequest(data)) return
    if (pendingApprovals.value.some((item) => item.key === data.key)) return
    pendingApprovals.value = [...pendingApprovals.value, data]
  }

  function applyApprovalResolved(data: Record<string, unknown>) {
    if (typeof data.key !== 'string') return
    pendingApprovals.value = pendingApprovals.value.filter((item) => item.key !== data.key)
  }

  return {
    library,
    libraryLoading,
    deviceViews,
    deviceLoading,
    pendingApprovals,
    nextApproval,
    error,
    fetchLibrary,
    fetchDetail,
    createPlugin,
    updatePlugin,
    importPlugin,
    deletePlugin,
    exportPlugin,
    lifecycle,
    acceptReview,
    runTests,
    previewPackage,
    loadDevice,
    saveCredential,
    deleteCredential,
    openSession,
    runAction,
    installPlugin,
    uninstallPlugin,
    saveDeviceSettings,
    runById,
    validatePlugin,
    cancelRun,
    respondApproval,
    autoAcceptState,
    enableAutoAccept,
    disableAutoAccept,
    runningRuns,
    applyRunStarted,
    applyRunOutput,
    applyRunFinished,
    applyApprovalRequired,
    applyApprovalResolved,
  }
})
