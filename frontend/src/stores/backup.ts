import { ref } from 'vue'
import { defineStore } from 'pinia'
import { apiService } from '@/services/apiService'

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

export interface BackupCounts {
  tables: Array<{ table: string; rows: number }>
  totalRows: number
}

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

export const useBackupStore = defineStore('backup', () => {
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
