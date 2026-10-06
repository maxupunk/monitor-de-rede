import { ref } from 'vue'
import { defineStore } from 'pinia'
import { apiService } from '@/services/apiService'
import { triggerDownload } from '@/utils/download'
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'
import type { StorageBackupResponse } from '@/bindings/StorageBackupResponse'
import type { StorageBackupRunResponse } from '@/bindings/StorageBackupRunResponse'
import type { StorageBrowseResponse } from '@/bindings/StorageBrowseResponse'
import type { StorageDestinationDetail } from '@/bindings/StorageDestinationDetail'
import type { StorageDestinationInput } from '@/bindings/StorageDestinationInput'
import type { StorageDestinationResponse } from '@/bindings/StorageDestinationResponse'
import type { StoragesMetaResponse } from '@/bindings/StoragesMetaResponse'
import type { StorageTestInput } from '@/bindings/StorageTestInput'
import type { StorageTestResponse } from '@/bindings/StorageTestResponse'

/**
 * Armazenamentos: os destinos das cópias do backup das configurações.
 *
 * A lista é a única coisa que vive aqui como estado; cópias, prévia e
 * explorador são consultas pontuais de um diálogo aberto, e devolvem o
 * resultado a quem pediu. Um `storages:updated` no SSE recarrega a lista —
 * mas só se esta tela já a carregou, porque a rota é de administrador.
 */
export const useStoragesStore = defineStore('storages', () => {
  const storages = ref<StorageDestinationResponse[]>([])
  const meta = ref<StoragesMetaResponse | null>(null)
  const loading = ref(false)
  const loaded = ref(false)
  const error = ref<string | null>(null)
  /** Destinos com "Fazer backup agora" em andamento nesta aba. */
  const running = ref<number[]>([])

  function message(err: unknown, fallback: string): string {
    return err instanceof Error ? err.message : fallback
  }

  async function fetchStorages(): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      storages.value = await apiService.get<StorageDestinationResponse[]>('/storages')
      loaded.value = true
      return true
    } catch (err) {
      error.value = message(err, 'Erro ao carregar os armazenamentos')
      return false
    } finally {
      loading.value = false
    }
  }

  /** O que o SSE chama: sem efeito para quem nunca abriu a tela. */
  function refreshIfLoaded() {
    if (loaded.value) void fetchStorages()
  }

  async function fetchMeta(): Promise<StoragesMetaResponse | null> {
    if (meta.value) return meta.value
    try {
      meta.value = await apiService.get<StoragesMetaResponse>('/storages/meta')
    } catch {
      meta.value = null
    }
    return meta.value
  }

  function fetchDetail(id: number): Promise<StorageDestinationDetail> {
    return apiService.get<StorageDestinationDetail>(`/storages/${id}`)
  }

  function upsert(row: StorageDestinationResponse) {
    const index = storages.value.findIndex((item) => item.id === row.id)
    if (index === -1) {
      storages.value = [...storages.value, row].sort((a, b) => a.name.localeCompare(b.name))
    } else {
      storages.value[index] = row
    }
  }

  async function save(
    id: number | null,
    input: StorageDestinationInput
  ): Promise<StorageDestinationDetail> {
    const saved =
      id == null
        ? await apiService.post<StorageDestinationDetail>('/storages', input)
        : await apiService.put<StorageDestinationDetail>(`/storages/${id}`, input)
    upsert(saved)
    return saved
  }

  async function remove(id: number): Promise<boolean> {
    error.value = null
    try {
      await apiService.delete(`/storages/${id}`)
      storages.value = storages.value.filter((item) => item.id !== id)
      return true
    } catch (err) {
      error.value = message(err, 'Erro ao excluir o armazenamento')
      return false
    }
  }

  function testDraft(input: StorageTestInput): Promise<StorageTestResponse> {
    return apiService.post<StorageTestResponse>('/storages/test', input)
  }

  function testSaved(id: number): Promise<StorageTestResponse> {
    return apiService.post<StorageTestResponse>(`/storages/${id}/test`)
  }

  /**
   * Envia uma cópia agora e relê a lista ao fim — inclusive quando falha,
   * porque a falha também fica gravada no cadastro. As outras abas recebem a
   * mudança pelo SSE (`storages:updated`).
   */
  async function runBackup(id: number): Promise<StorageBackupRunResponse> {
    running.value = [...running.value, id]
    try {
      return await apiService.post<StorageBackupRunResponse>(`/storages/${id}/backups`)
    } finally {
      running.value = running.value.filter((item) => item !== id)
      void fetchStorages()
    }
  }

  function listBackups(id: number): Promise<StorageBackupResponse[]> {
    return apiService.get<StorageBackupResponse[]>(`/storages/${id}/backups`)
  }

  function previewBackup(id: number, key: string): Promise<BackupCountsResponse> {
    return apiService.post<BackupCountsResponse>(`/storages/${id}/backups/preview`, { key })
  }

  function restoreBackup(id: number, key: string): Promise<BackupCountsResponse> {
    return apiService.post<BackupCountsResponse>(`/storages/${id}/backups/restore`, { key })
  }

  function browse(
    id: number,
    path: string,
    cursor?: string | null
  ): Promise<StorageBrowseResponse> {
    const params = new URLSearchParams({ path })
    if (cursor) params.set('cursor', cursor)
    return apiService.get<StorageBrowseResponse>(`/storages/${id}/browse?${params.toString()}`)
  }

  async function downloadObject(id: number, key: string, filename: string): Promise<void> {
    const params = new URLSearchParams({ key })
    const blob = await apiService.download(`/storages/${id}/download?${params.toString()}`)
    triggerDownload(blob, filename)
  }

  async function deleteObject(id: number, key: string, directory: boolean): Promise<void> {
    const params = new URLSearchParams({ key, directory: String(directory) })
    await apiService.delete(`/storages/${id}/objects?${params.toString()}`)
  }

  return {
    storages,
    meta,
    loading,
    loaded,
    error,
    running,
    fetchStorages,
    refreshIfLoaded,
    fetchMeta,
    fetchDetail,
    save,
    remove,
    testDraft,
    testSaved,
    runBackup,
    listBackups,
    previewBackup,
    restoreBackup,
    browse,
    downloadObject,
    deleteObject,
  }
})
