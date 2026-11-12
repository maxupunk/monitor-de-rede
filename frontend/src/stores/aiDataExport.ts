import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { AiDataExportSummary } from '@/bindings/AiDataExportSummary'
import { triggerDownload } from '@/utils/download'

const BASE = '/ai/data-export'

/** Período da exportação: últimos N dias, ou `null` para tudo. */
export type AiDataExportPeriod = number | null

function query(days: AiDataExportPeriod): string {
  return days === null ? '' : `?days=${days}`
}

/** `netmonitor-ia-dados-20261112-150405.zip` — o mesmo nome que o servidor dá. */
function filename(now = new Date()): string {
  const stamp = now.toISOString().slice(0, 19).replace(/[-:]/g, '').replace('T', '-')
  return `netmonitor-ia-dados-${stamp}.zip`
}

/**
 * Exportação dos dados do Assistente IA (conversas, comandos e execuções em
 * equipamentos) para treino e revisão. O resumo é pedido quando o card abre e
 * quando o período muda — nunca em ciclo.
 */
export const useAiDataExportStore = defineStore('aiDataExport', () => {
  const days = ref<AiDataExportPeriod>(30)
  const summary = ref<AiDataExportSummary | null>(null)
  const loading = ref(false)
  const downloading = ref(false)
  const error = ref<string | null>(null)

  function describe(err: unknown, fallback: string): string {
    return err instanceof Error ? err.message : fallback
  }

  async function loadSummary() {
    loading.value = true
    error.value = null
    try {
      summary.value = await apiService.get<AiDataExportSummary>(
        `${BASE}/summary${query(days.value)}`
      )
    } catch (err: unknown) {
      summary.value = null
      error.value = describe(err, 'Falha ao contar os dados do assistente')
    } finally {
      loading.value = false
    }
  }

  async function setPeriod(value: AiDataExportPeriod) {
    days.value = value
    await loadSummary()
  }

  /** Baixa o pacote `.zip`; devolve se deu certo. */
  async function download(): Promise<boolean> {
    downloading.value = true
    error.value = null
    try {
      const blob = await apiService.download(`${BASE}${query(days.value)}`)
      triggerDownload(blob, filename())
      return true
    } catch (err: unknown) {
      error.value = describe(err, 'Falha ao gerar o pacote')
      return false
    } finally {
      downloading.value = false
    }
  }

  return { days, summary, loading, downloading, error, loadSummary, setPeriod, download }
})
