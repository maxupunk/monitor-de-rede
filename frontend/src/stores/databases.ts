import { ref } from 'vue'
import { defineStore } from 'pinia'
import { apiService } from '@/services/apiService'
import type { DatabaseBackupResponse } from '@/bindings/DatabaseBackupResponse'
import type { DatabaseConnectionInput } from '@/bindings/DatabaseConnectionInput'
import type { DatabaseConnectionResponse } from '@/bindings/DatabaseConnectionResponse'
import type { DatabaseJobSnapshot } from '@/bindings/DatabaseJobSnapshot'
import type { DatabaseProbeInput } from '@/bindings/DatabaseProbeInput'
import type { DatabaseProbeResponse } from '@/bindings/DatabaseProbeResponse'
import type { DatabaseRestoreInput } from '@/bindings/DatabaseRestoreInput'

/** O payload do SSE tem a forma de um snapshot de andamento? */
export function isDatabaseJobSnapshot(value: unknown): value is DatabaseJobSnapshot {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return (
    typeof candidate.id === 'string' &&
    (candidate.kind === 'backup' || candidate.kind === 'restore') &&
    typeof candidate.connectionId === 'number' &&
    typeof candidate.status === 'string'
  )
}

/** Chave de um andamento: um backup e uma restauração por conexão. */
export function jobKey(kind: DatabaseJobSnapshot['kind'], connectionId: number): string {
  return `${kind}:${connectionId}`
}

/**
 * Conexões de banco e o andamento ao vivo dos backups e restaurações.
 *
 * O andamento chega só pelo SSE (`database_jobs:updated`, snapshot por
 * conexão) e é aplicado aqui por [`applyJob`]; nenhuma consulta periódica.
 */
export const useDatabasesStore = defineStore('databases', () => {
  const connections = ref<DatabaseConnectionResponse[]>([])
  const jobs = ref<Record<string, DatabaseJobSnapshot>>({})
  const loading = ref(false)
  const loaded = ref(false)
  const error = ref<string | null>(null)

  async function fetchConnections(): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      connections.value = await apiService.get<DatabaseConnectionResponse[]>('/databases')
      loaded.value = true
      return true
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar as conexões'
      return false
    } finally {
      loading.value = false
    }
  }

  /** O que o SSE chama: sem efeito para quem nunca abriu a tela (rota de admin). */
  function refreshIfLoaded() {
    if (loaded.value) void fetchConnections()
  }

  /** Snapshot de andamento vindo do SSE ou da resposta `202`. */
  function applyJob(snapshot: DatabaseJobSnapshot) {
    const key = jobKey(snapshot.kind, snapshot.connectionId)
    const current = jobs.value[key]
    // Um snapshot atrasado de uma execução anterior não apaga a atual.
    if (current && current.id !== snapshot.id && current.startedAt > snapshot.startedAt) return
    jobs.value = { ...jobs.value, [key]: snapshot }
  }

  function jobFor(
    kind: DatabaseJobSnapshot['kind'],
    connectionId: number
  ): DatabaseJobSnapshot | null {
    return jobs.value[jobKey(kind, connectionId)] ?? null
  }

  async function save(
    id: number | null,
    input: DatabaseConnectionInput
  ): Promise<DatabaseConnectionResponse> {
    const saved =
      id == null
        ? await apiService.post<DatabaseConnectionResponse>('/databases', input)
        : await apiService.put<DatabaseConnectionResponse>(`/databases/${id}`, input)
    const index = connections.value.findIndex((item) => item.id === saved.id)
    if (index === -1) {
      connections.value = [...connections.value, saved].sort((a, b) => a.name.localeCompare(b.name))
    } else {
      connections.value[index] = saved
    }
    return saved
  }

  async function remove(id: number): Promise<boolean> {
    error.value = null
    try {
      await apiService.delete(`/databases/${id}`)
      connections.value = connections.value.filter((item) => item.id !== id)
      return true
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Erro ao excluir a conexão'
      return false
    }
  }

  function probe(input: DatabaseProbeInput): Promise<DatabaseProbeResponse> {
    return apiService.post<DatabaseProbeResponse>('/databases/probe', input)
  }

  async function runBackup(id: number): Promise<DatabaseJobSnapshot> {
    const snapshot = await apiService.post<DatabaseJobSnapshot>(`/databases/${id}/backups`)
    applyJob(snapshot)
    return snapshot
  }

  function history(id: number): Promise<DatabaseBackupResponse[]> {
    return apiService.get<DatabaseBackupResponse[]>(`/databases/${id}/backups`)
  }

  async function restore(
    backupId: number,
    input: DatabaseRestoreInput
  ): Promise<DatabaseJobSnapshot> {
    const snapshot = await apiService.post<DatabaseJobSnapshot>(
      `/databases/backups/${backupId}/restore`,
      input
    )
    applyJob(snapshot)
    return snapshot
  }

  return {
    connections,
    jobs,
    loading,
    loaded,
    error,
    fetchConnections,
    refreshIfLoaded,
    applyJob,
    jobFor,
    save,
    remove,
    probe,
    runBackup,
    history,
    restore,
  }
})
