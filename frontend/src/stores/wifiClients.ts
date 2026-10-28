import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { WifiDeviceSnapshot } from '@/bindings/WifiDeviceSnapshot'
import type { WifiPingInput } from '@/bindings/WifiPingInput'
import type { WifiPingResult } from '@/bindings/WifiPingResult'
import type { WifiRefreshInput } from '@/bindings/WifiRefreshInput'
import type { WifiRefreshStarted } from '@/bindings/WifiRefreshStarted'
import type { WifiWatchCreate } from '@/bindings/WifiWatchCreate'
import type { WifiWatchUpdate } from '@/bindings/WifiWatchUpdate'
import type { WifiWatchedClient } from '@/bindings/WifiWatchedClient'
import type { WifiWatchedSnapshot } from '@/bindings/WifiWatchedSnapshot'
import { stationRows, type WifiStationRow } from '@/utils/wifiPresentation'

/** Um teste de latência até uma estação. */
export interface WifiPingState {
  loading: boolean
  result: WifiPingResult | null
  error: string | null
}

function messageOf(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}

function pingKey(deviceId: number, mac: string): string {
  return `${deviceId}:${mac}`
}

/**
 * Clientes Wi-Fi de todos os equipamentos.
 *
 * As estações chegam só pelo SSE (`wifi:snapshot`, um snapshot por
 * equipamento — o último de cada um vem ao conectar), e a lista dos aparelhos
 * vigiados também (`wifi:watched`). HTTP é só para pedir uma leitura, medir
 * latência e vigiar ou deixar de vigiar, sempre por ação de quem usa
 * (AGENTS §9).
 */
export const useWifiClientsStore = defineStore('wifiClients', () => {
  const snapshots = ref<Record<number, WifiDeviceSnapshot>>({})
  const pings = ref<Record<string, WifiPingState>>({})
  const requesting = ref(false)
  const error = ref<string | null>(null)
  /** A última leitura de todos — diz se há equipamento com Wi-Fi. */
  const lastFleetRefresh = ref<WifiRefreshStarted | null>(null)
  /** Os aparelhos que deveriam estar sempre conectados (alertam se sumirem). */
  const watched = ref<WifiWatchedClient[]>([])

  const devices = computed(() =>
    Object.values(snapshots.value).sort((a, b) => a.deviceName.localeCompare(b.deviceName))
  )
  const rows = computed<WifiStationRow[]>(() => stationRows(devices.value))
  const reading = computed(() => devices.value.some((device) => device.status === 'reading'))
  const watchedByMac = computed(
    () => new Map(watched.value.map((client) => [client.mac, client] as const))
  )

  function snapshotFor(deviceId: number): WifiDeviceSnapshot | null {
    return snapshots.value[deviceId] ?? null
  }

  function applySnapshot(snapshot: WifiDeviceSnapshot) {
    snapshots.value = { ...snapshots.value, [snapshot.deviceId]: snapshot }
  }

  function applyWatched(snapshot: WifiWatchedSnapshot) {
    watched.value = snapshot.clients
  }

  /** A resposta já traz o aparelho; o SSE confirma para as outras abas. */
  function upsertWatched(client: WifiWatchedClient) {
    const others = watched.value.filter((item) => item.id !== client.id)
    watched.value = [...others, client].sort((a, b) => a.name.localeCompare(b.name))
  }

  async function watch(mac: string, name: string): Promise<boolean> {
    error.value = null
    try {
      const body: WifiWatchCreate = { mac, name }
      upsertWatched(await apiService.post<WifiWatchedClient>('/wifi/watched', body))
      return true
    } catch (err: unknown) {
      error.value = messageOf(err, 'Não foi possível vigiar o aparelho.')
      return false
    }
  }

  async function renameWatched(id: number, name: string): Promise<boolean> {
    error.value = null
    try {
      const body: WifiWatchUpdate = { name }
      upsertWatched(await apiService.patch<WifiWatchedClient>(`/wifi/watched/${id}`, body))
      return true
    } catch (err: unknown) {
      error.value = messageOf(err, 'Não foi possível renomear o aparelho.')
      return false
    }
  }

  async function unwatch(id: number): Promise<boolean> {
    error.value = null
    try {
      await apiService.delete(`/wifi/watched/${id}`)
      watched.value = watched.value.filter((client) => client.id !== id)
      return true
    } catch (err: unknown) {
      error.value = messageOf(err, 'Não foi possível deixar de vigiar o aparelho.')
      return false
    }
  }

  /** Numa leitura de todos, quem ficou de fora perdeu o plugin Wi-Fi. */
  function keepOnly(deviceIds: number[]) {
    snapshots.value = Object.fromEntries(
      Object.entries(snapshots.value).filter(([id]) => deviceIds.includes(Number(id)))
    )
  }

  /** Pede a leitura de um equipamento, ou de todos sem `deviceId`. */
  async function refresh(deviceId?: number): Promise<boolean> {
    requesting.value = true
    error.value = null
    try {
      const body: WifiRefreshInput = deviceId === undefined ? {} : { deviceId }
      const started = await apiService.post<WifiRefreshStarted>('/wifi/refresh', body)
      if (deviceId === undefined) {
        lastFleetRefresh.value = started
        keepOnly(started.deviceIds)
      }
      return true
    } catch (err: unknown) {
      error.value = messageOf(err, 'Não foi possível pedir a leitura do Wi-Fi.')
      return false
    } finally {
      requesting.value = false
    }
  }

  function pingOf(deviceId: number, mac: string): WifiPingState | null {
    return pings.value[pingKey(deviceId, mac)] ?? null
  }

  async function ping(row: WifiStationRow) {
    if (!row.ip) return
    const key = pingKey(row.deviceId, row.mac)
    pings.value[key] = { loading: true, result: null, error: null }
    try {
      const body: WifiPingInput = { deviceId: row.deviceId, target: row.ip }
      const result = await apiService.post<WifiPingResult>('/wifi/ping', body, {
        timeoutMs: 30_000,
      })
      pings.value[key] = { loading: false, result, error: null }
    } catch (err: unknown) {
      pings.value[key] = {
        loading: false,
        result: null,
        error: messageOf(err, 'Não foi possível medir a latência.'),
      }
    }
  }

  return {
    snapshots,
    pings,
    requesting,
    error,
    lastFleetRefresh,
    watched,
    devices,
    rows,
    reading,
    watchedByMac,
    snapshotFor,
    applySnapshot,
    applyWatched,
    watch,
    renameWatched,
    unwatch,
    refresh,
    pingOf,
    ping,
  }
})
