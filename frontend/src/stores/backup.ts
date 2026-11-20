import { ref } from 'vue'
import { defineStore } from 'pinia'
import { apiService } from '@/services/apiService'
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'
import type { BackupRestoreResponse } from '@/bindings/BackupRestoreResponse'
import type { SystemBackupCopyResponse } from '@/bindings/SystemBackupCopyResponse'
import type { SystemBackupPlanInput } from '@/bindings/SystemBackupPlanInput'
import type { SystemBackupPlanResponse } from '@/bindings/SystemBackupPlanResponse'
import type { SystemBackupRunResponse } from '@/bindings/SystemBackupRunResponse'
import { clearStoredAuth } from '@/utils/authStorage'
import { triggerDownload } from '@/utils/download'

/**
 * Envelope do arquivo de backup (`services::backup::service::BackupFile`).
 *
 * `tables` é intencionalmente opaco aqui: as chaves são nomes de tabela e as
 * linhas vêm no formato do banco. A tela não lê o conteúdo — ela conta linhas,
 * e quem conta é o backend, no `preview`.
 */
export interface BackupFile {
  formatVersion: number
  appVersion: string
  generatedAt: string
  tables: Record<string, unknown[]>
}

export type BackupCounts = BackupCountsResponse

/**
 * A restauração trocou sites, redes, dispositivos e monitores por baixo de
 * todas as stores já carregadas. Recarregar a aplicação é mais honesto do que
 * invalidar uma por uma e deixar alguma tela com dado morto.
 */
export function reloadAfterRestore() {
  window.location.reload()
}

/**
 * Depois de restaurar um arquivo: o backup completo traz os usuários dele, e
 * a conta de quem restaurou pode não existir mais — aí a sessão acabou e o
 * caminho é entrar de novo, com uma conta do backup. Senão, recarrega.
 */
export function finishRestore(result: BackupRestoreResponse) {
  if (result.signInAgain) {
    clearStoredAuth()
    window.location.assign('/login?restaurado=1')
  } else {
    reloadAfterRestore()
  }
}

/** Fase do envio do arquivo: a prévia lê o arquivo inteiro, a restauração de novo. */
export interface UploadProgress {
  phase: 'preview' | 'restore'
  sentBytes: number
  totalBytes: number
}

/**
 * Backup do próprio NetMonitor: o plano (destino, agenda, retenção), as cópias
 * guardadas nos destinos e o caminho por arquivo (baixar e enviar o `.json`).
 *
 * O plano chega por HTTP ao abrir a tela; depois, `backup_plan:updated` no SSE
 * o recarrega — inclusive quando o backup automático roda sozinho.
 */
export const useBackupStore = defineStore('backup', () => {
  const plan = ref<SystemBackupPlanResponse | null>(null)
  const running = ref(false)
  const exporting = ref(false)
  const restoring = ref(false)
  const error = ref<string | null>(null)

  /** Arquivo escolhido pelo operador, já conferido pelo backend na prévia. */
  const pendingFile = ref<File | null>(null)
  const pendingName = ref<string | null>(null)
  const pendingCounts = ref<BackupCounts | null>(null)
  /** Andamento do envio do arquivo (prévia ou restauração). */
  const upload = ref<UploadProgress | null>(null)

  /** Resultado da última restauração, para a tela dizer o que entrou. */
  const lastRestore = ref<BackupRestoreResponse | null>(null)

  /** O backup completo baixando: quantos bytes já chegaram. */
  const fullDownload = ref<{ receivedBytes: number } | null>(null)
  let fullDownloadAbort: AbortController | null = null

  function message(err: unknown, fallback: string): string {
    return err instanceof Error ? err.message : fallback
  }

  async function fetchPlan(): Promise<SystemBackupPlanResponse | null> {
    try {
      plan.value = await apiService.get<SystemBackupPlanResponse>('/backup/system')
    } catch (err) {
      error.value = message(err, 'Erro ao carregar o plano de backup')
    }
    return plan.value
  }

  /** O que o SSE chama: sem efeito para quem nunca abriu a tela (rota de admin). */
  function refreshPlanIfLoaded() {
    if (plan.value) void fetchPlan()
  }

  async function savePlan(input: SystemBackupPlanInput): Promise<SystemBackupPlanResponse> {
    plan.value = await apiService.put<SystemBackupPlanResponse>('/backup/system', input)
    return plan.value
  }

  /** "Fazer backup agora". O resultado — inclusive a falha — vai para o plano. */
  async function runNow(): Promise<SystemBackupRunResponse> {
    running.value = true
    try {
      return await apiService.post<SystemBackupRunResponse>('/backup/system/run')
    } finally {
      running.value = false
      void fetchPlan()
    }
  }

  /** Cópias num destino; sem `storageId`, no destino do plano. */
  function listCopies(storageId?: number | null): Promise<SystemBackupCopyResponse[]> {
    const query = storageId != null ? `?storageDestinationId=${storageId}` : ''
    return apiService.get<SystemBackupCopyResponse[]>(`/backup/system/copies${query}`)
  }

  function previewCopy(storageDestinationId: number, key: string): Promise<BackupCounts> {
    return apiService.post<BackupCounts>('/backup/system/copies/preview', {
      storageDestinationId,
      key,
    })
  }

  function restoreCopy(storageDestinationId: number, key: string): Promise<BackupCounts> {
    return apiService.post<BackupCounts>('/backup/system/copies/restore', {
      storageDestinationId,
      key,
    })
  }

  /**
   * Baixa o backup como arquivo.
   *
   * O `apiService` só devolve JSON parseado, então o download é montado aqui a
   * partir dele — um `<a download>` com o JSON reserializado. Reaproveitar o
   * `Content-Disposition` do backend exigiria um `fetch` cru só para isso.
   */
  async function exportConfig(): Promise<boolean> {
    exporting.value = true
    error.value = null
    try {
      const file = await apiService.get<BackupFile>('/backup/export')
      const stamp = new Date().toISOString().slice(0, 19).replace(/[-:]/g, '').replace('T', '-')
      const blob = new Blob([JSON.stringify(file, null, 2)], { type: 'application/json' })
      triggerDownload(blob, `netmonitor-backup-${stamp}.json`)
      return true
    } catch (err) {
      error.value = message(err, 'Erro ao exportar as configurações')
      return false
    } finally {
      exporting.value = false
    }
  }

  /**
   * Baixa os bancos inteiros (principal e de logs) num `.json.gz`.
   *
   * O servidor transmite em fluxo, sem saber o tamanho final: a tela mostra
   * quanto já chegou. Cancelável pelo `cancelFullDownload`. Devolve o tamanho
   * do arquivo baixado, ou `null` quando não baixou.
   */
  async function downloadFull(): Promise<number | null> {
    if (fullDownload.value) return null
    error.value = null
    fullDownload.value = { receivedBytes: 0 }
    fullDownloadAbort = new AbortController()
    try {
      const { blob, filename } = await apiService.downloadWithProgress(
        '/backup/full',
        (receivedBytes) => {
          if (fullDownload.value) fullDownload.value.receivedBytes = receivedBytes
        },
        fullDownloadAbort.signal
      )
      triggerDownload(blob, filename ?? 'netmonitor-backup-completo.json.gz')
      return blob.size
    } catch (err) {
      error.value = fullDownloadAbort?.signal.aborted
        ? null
        : message(err, 'Erro ao baixar o backup completo')
      return null
    } finally {
      fullDownload.value = null
      fullDownloadAbort = null
    }
  }

  function cancelFullDownload() {
    fullDownloadAbort?.abort()
  }

  /**
   * Envia o arquivo escolhido e pergunta ao backend o que há nele.
   *
   * A prévia é o passo que separa "escolhi um arquivo" de "mandei apagar tudo":
   * o operador vê a contagem por tabela antes de confirmar. O arquivo vai cru
   * (`.json` ou `.json.gz`) e o servidor o lê em fluxo, sem limite de tamanho.
   */
  async function loadFile(file: File): Promise<boolean> {
    error.value = null
    pendingFile.value = null
    pendingCounts.value = null
    pendingName.value = file.name
    upload.value = { phase: 'preview', sentBytes: 0, totalBytes: file.size }
    try {
      pendingCounts.value = await apiService.upload<BackupCounts>(
        '/backup/preview',
        file,
        (sentBytes, totalBytes) => {
          upload.value = { phase: 'preview', sentBytes, totalBytes }
        }
      )
      pendingFile.value = file
      return true
    } catch (err) {
      pendingName.value = null
      error.value = message(err, 'Erro ao ler o arquivo de backup')
      return false
    } finally {
      upload.value = null
    }
  }

  function clearFile() {
    pendingFile.value = null
    pendingName.value = null
    pendingCounts.value = null
  }

  /**
   * Aplica o arquivo já conferido: substitui a configuração atual — ou, com o
   * backup completo, os bancos inteiros. Quem chama decide o que vem depois
   * (`finishRestore`).
   */
  async function restoreFile(): Promise<BackupRestoreResponse | null> {
    const file = pendingFile.value
    if (!file) return null
    restoring.value = true
    error.value = null
    upload.value = { phase: 'restore', sentBytes: 0, totalBytes: file.size }
    try {
      lastRestore.value = await apiService.upload<BackupRestoreResponse>(
        '/backup/restore',
        file,
        (sentBytes, totalBytes) => {
          upload.value = { phase: 'restore', sentBytes, totalBytes }
        }
      )
      clearFile()
      return lastRestore.value
    } catch (err) {
      error.value = message(err, 'Erro ao restaurar o backup')
      return null
    } finally {
      restoring.value = false
      upload.value = null
    }
  }

  return {
    plan,
    running,
    fetchPlan,
    refreshPlanIfLoaded,
    savePlan,
    runNow,
    listCopies,
    previewCopy,
    restoreCopy,
    exporting,
    restoring,
    error,
    pendingFile,
    pendingName,
    pendingCounts,
    upload,
    lastRestore,
    fullDownload,
    exportConfig,
    downloadFull,
    cancelFullDownload,
    loadFile,
    clearFile,
    restoreFile,
  }
})
