import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { BatchStarted } from '@/bindings/BatchStarted'
import type { FleetView } from '@/bindings/FleetView'
import type { PluginApp } from '@/bindings/PluginApp'
import type { PluginBatchView } from '@/bindings/PluginBatchView'
import type { TranscriptEntry } from '@/bindings/TranscriptEntry'
import { usePluginsStore } from '@/stores/plugins'

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isBatch(value: unknown): value is PluginBatchView {
  return (
    isRecord(value) &&
    typeof value.id === 'number' &&
    typeof value.pluginId === 'number' &&
    Array.isArray(value.devices)
  )
}

function isEntry(value: unknown): value is TranscriptEntry {
  return (
    isRecord(value) &&
    typeof value.request === 'string' &&
    typeof value.kind === 'string' &&
    typeof value.durationMs === 'number'
  )
}

/** Lotes mantidos por aplicativo. */
const BATCHES_LIMIT = 15

/**
 * Os valores com que um lote foi lançado desta aba. O servidor não os devolve
 * (podem ter senha), então só esta aba consegue rodar de novo com eles.
 */
interface LaunchedBatch {
  pluginId: number
  action: string
  params: Record<string, unknown>
  deviceParams?: Record<number, Record<string, unknown>>
  /** Os equipamentos que ainda podem rodar de novo com estes valores. */
  devices: Set<number>
}

/**
 * Aplicativos: plugins de frota (vários equipamentos), com a configuração
 * guardada da rede e as ações em lote. O andamento chega pelo SSE
 * (`plugin:batch_updated`), sem consulta repetida.
 */
export const usePluginAppsStore = defineStore('pluginApps', () => {
  const apps = ref<PluginApp[]>([])
  const fleets = ref<Record<number, FleetView>>({})
  const loading = ref(false)
  const error = ref<string | null>(null)
  /** O último acesso de cada execução (o andamento ao vivo de um lote). */
  const lastSteps = ref<Record<number, TranscriptEntry>>({})
  const identifying = ref(false)
  /**
   * Por lote, os valores com que esta aba o lançou. Fora do estado de
   * propósito: só em memória, nunca persistido nem exposto; sai ao fechar a
   * página do aplicativo, ao rodar de novo e quando o lote sai da lista.
   */
  const launched = new Map<number, LaunchedBatch>()

  /** Menu "Aplicativos" — carregado na abertura do layout. */
  async function fetchApps(): Promise<void> {
    try {
      apps.value = await apiService.get<PluginApp[]>('/plugins/apps')
    } catch {
      // Sem permissão ou sem plugins: o menu só não mostra a seção.
      apps.value = []
    }
  }

  async function loadFleet(pluginId: number): Promise<void> {
    loading.value = true
    error.value = null
    try {
      fleets.value[pluginId] = await apiService.get<FleetView>(`/plugins/${pluginId}/fleet`)
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Falha ao carregar o aplicativo'
    } finally {
      loading.value = false
    }
  }

  async function saveFleetSettings(pluginId: number, value: Record<string, unknown>) {
    const saved = await apiService.put<Record<string, unknown>>(`/plugins/${pluginId}/settings`, {
      value,
    })
    const fleet = fleets.value[pluginId]
    if (fleet) fleet.settings = saved
    return saved
  }

  async function saveMemberSettings(
    pluginId: number,
    deviceId: number,
    value: Record<string, unknown>
  ) {
    const saved = await apiService.put<Record<string, unknown>>(
      `/devices/${deviceId}/plugins/${pluginId}/settings`,
      { value }
    )
    const member = fleets.value[pluginId]?.members.find((item) => item.deviceId === deviceId)
    if (member) member.settings = saved
    return saved
  }

  /** Adiciona um equipamento à frota: a mesma instalação da aba do equipamento. */
  async function addMember(pluginId: number, deviceId: number): Promise<void> {
    await usePluginsStore().installPlugin(deviceId, pluginId)
    await loadFleet(pluginId)
  }

  async function removeMember(pluginId: number, deviceId: number): Promise<void> {
    await apiService.delete(`/devices/${deviceId}/plugins/${pluginId}/install`)
    await loadFleet(pluginId)
    await fetchApps()
  }

  async function runAction(
    pluginId: number,
    action: string,
    deviceIds: number[],
    confirmWrite: boolean,
    params: Record<string, unknown> = {},
    /** Parâmetros de cada equipamento, por cima dos comuns. */
    deviceParams?: Record<number, Record<string, unknown>>
  ): Promise<number> {
    const started = await apiService.post<BatchStarted>(
      `/plugins/${pluginId}/fleet/actions/${encodeURIComponent(action)}`,
      { deviceIds, params, confirmWrite, deviceParams }
    )
    launched.set(started.batchId, {
      pluginId,
      action,
      params,
      deviceParams,
      devices: new Set(deviceIds),
    })
    return started.batchId
  }

  /** Esta aba ainda tem os valores com que lançou o lote para este equipamento. */
  function canRerun(batchId: number, deviceId: number): boolean {
    return launched.get(batchId)?.devices.has(deviceId) ?? false
  }

  /**
   * Roda a ação do lote de novo só em `deviceId`, com os mesmos valores (os
   * comuns e os dele). `null` quando esta aba não os tem. Depois de rodar, o
   * lote antigo esquece o equipamento — o lote novo guarda os seus.
   */
  async function rerunDevice(
    batchId: number,
    deviceId: number,
    confirmWrite: boolean
  ): Promise<number | null> {
    const kept = launched.get(batchId)
    if (!kept?.devices.has(deviceId)) return null
    const own = kept.deviceParams?.[deviceId]
    const id = await runAction(
      kept.pluginId,
      kept.action,
      [deviceId],
      confirmWrite,
      kept.params,
      own ? { [deviceId]: own } : undefined
    )
    kept.devices.delete(deviceId)
    if (kept.devices.size === 0) launched.delete(batchId)
    return id
  }

  /** A página do aplicativo fechou: os valores guardados dos lotes dele saem. */
  function forgetLaunched(pluginId: number): void {
    for (const [batchId, kept] of launched) {
      if (kept.pluginId === pluginId) launched.delete(batchId)
    }
  }

  /**
   * "Verificar sistema": vai aos equipamentos (SSH/SNMP; Decisor na dúvida) e
   * devolve a página com a compatibilidade refeita. Sem lista, os em dúvida.
   */
  async function identify(pluginId: number, deviceIds: number[] = []): Promise<FleetView> {
    identifying.value = true
    try {
      const view = await apiService.post<FleetView>(
        `/plugins/${pluginId}/fleet/identify`,
        { deviceIds },
        { timeoutMs: 120_000 }
      )
      fleets.value[pluginId] = view
      return view
    } finally {
      identifying.value = false
    }
  }

  async function cancelBatch(batchId: number): Promise<void> {
    await apiService.post(`/plugin-batches/${batchId}/cancel`, {})
  }

  async function applyPatch(batch: PluginBatchView): Promise<void> {
    const updated = await apiService.post<PluginBatchView>(
      `/plugin-batches/${batch.id}/apply-patch`,
      {}
    )
    upsertBatch(updated)
    await loadFleet(batch.pluginId)
  }

  function batchById(pluginId: number, batchId: number | null): PluginBatchView | null {
    if (batchId === null) return null
    return fleets.value[pluginId]?.batches.find((batch) => batch.id === batchId) ?? null
  }

  /** O último lote concluído de uma ação (ex.: o estado mais recente). */
  function latestBatch(pluginId: number, action: string): PluginBatchView | null {
    return (
      fleets.value[pluginId]?.batches.find(
        (batch) => batch.action === action && batch.finishedAt !== null
      ) ?? null
    )
  }

  function upsertBatch(batch: PluginBatchView) {
    const fleet = fleets.value[batch.pluginId]
    if (!fleet) return
    const index = fleet.batches.findIndex((item) => item.id === batch.id)
    if (index >= 0) {
      fleet.batches.splice(index, 1, batch)
      return
    }
    const all = [batch, ...fleet.batches]
    fleet.batches = all.slice(0, BATCHES_LIMIT)
    for (const dropped of all.slice(BATCHES_LIMIT)) launched.delete(dropped.id)
  }

  function applyBatchUpdated(data: Record<string, unknown>) {
    if (isBatch(data.batch)) upsertBatch(data.batch)
  }

  function applyRunOutput(data: Record<string, unknown>) {
    const runId = Number(data.runId)
    if (Number.isFinite(runId) && isEntry(data.entry)) lastSteps.value[runId] = data.entry
  }

  return {
    apps,
    fleets,
    loading,
    error,
    lastSteps,
    identifying,
    identify,
    applyRunOutput,
    fetchApps,
    loadFleet,
    saveFleetSettings,
    saveMemberSettings,
    addMember,
    removeMember,
    runAction,
    canRerun,
    rerunDevice,
    forgetLaunched,
    cancelBatch,
    applyPatch,
    batchById,
    latestBatch,
    applyBatchUpdated,
  }
})
