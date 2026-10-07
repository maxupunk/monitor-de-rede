import { ref } from 'vue'
import { defineStore } from 'pinia'
import { apiService } from '@/services/apiService'
import type { BackupCountsResponse } from '@/bindings/BackupCountsResponse'
import type { SystemBackupCopyResponse } from '@/bindings/SystemBackupCopyResponse'
import type { SystemBackupPlanInput } from '@/bindings/SystemBackupPlanInput'
import type { SystemBackupPlanResponse } from '@/bindings/SystemBackupPlanResponse'
import type { SystemBackupRunResponse } from '@/bindings/SystemBackupRunResponse'

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

/** Rótulos em português para os nomes de tabela do arquivo. */
const TABLE_LABELS: Record<string, string> = {
  sites: 'Sites',
  probes: 'Probes',
  networks: 'Redes',
  devices: 'Dispositivos',
  device_interfaces: 'Interfaces',
  device_links: 'Enlaces de topologia',
  monitors: 'Monitores',
  alert_rules: 'Regras de alerta',
  vpn_servers: 'Servidores VPN',
  vpn_peers: 'Peers VPN',
  dns_servers: 'Servidores DNS',
  system_settings: 'Preferências do sistema',
}

export function tableLabel(table: string): string {
  return TABLE_LABELS[table] ?? table
}

/**
 * A restauração trocou sites, redes, dispositivos e monitores por baixo de
 * todas as stores já carregadas. Recarregar a aplicação é mais honesto do que
 * invalidar uma por uma e deixar alguma tela com dado morto.
 */
export function reloadAfterRestore() {
  window.location.reload()
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

  /** Arquivo escolhido pelo operador, já lido e validado como JSON. */
  const pendingFile = ref<BackupFile | null>(null)
  const pendingName = ref<string | null>(null)
  const pendingCounts = ref<BackupCounts | null>(null)

  /** Resultado da última restauração, para a tela dizer o que entrou. */
  const lastRestore = ref<BackupCounts | null>(null)

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
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `netmonitor-backup-${stamp}.json`
      link.click()
      URL.revokeObjectURL(url)
      return true
    } catch (err) {
      error.value = message(err, 'Erro ao exportar as configurações')
      return false
    } finally {
      exporting.value = false
    }
  }

  /**
   * Lê o arquivo escolhido e pergunta ao backend o que há nele.
   *
   * A prévia é o passo que separa "escolhi um arquivo" de "mandei apagar tudo":
   * o operador vê a contagem por tabela antes de confirmar.
   */
  async function loadFile(file: File): Promise<boolean> {
    error.value = null
    pendingFile.value = null
    pendingCounts.value = null
    pendingName.value = file.name
    try {
      const parsed = JSON.parse(await file.text()) as BackupFile
      pendingCounts.value = await apiService.post<BackupCounts>('/backup/preview', parsed)
      pendingFile.value = parsed
      return true
    } catch (err) {
      pendingName.value = null
      error.value =
        err instanceof SyntaxError
          ? 'O arquivo escolhido não é um JSON válido'
          : message(err, 'Erro ao ler o arquivo de backup')
      return false
    }
  }

  function clearFile() {
    pendingFile.value = null
    pendingName.value = null
    pendingCounts.value = null
  }

  /** Aplica o arquivo já carregado, substituindo a configuração atual. */
  async function restoreConfig(): Promise<boolean> {
    if (!pendingFile.value) return false
    restoring.value = true
    error.value = null
    try {
      lastRestore.value = await apiService.post<BackupCounts>('/backup/restore', pendingFile.value)
      clearFile()
      return true
    } catch (err) {
      error.value = message(err, 'Erro ao restaurar as configurações')
      return false
    } finally {
      restoring.value = false
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
    lastRestore,
    exportConfig,
    loadFile,
    clearFile,
    restoreConfig,
  }
})
