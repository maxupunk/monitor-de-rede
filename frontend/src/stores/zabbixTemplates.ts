import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { ZabbixTemplateItemSummary } from './devices'

export interface ZabbixTemplate {
  id: number
  zabbixUuid: string | null
  name: string
  description: string | null
  zabbixVersion: string | null
  importedAt: string
  deviceCount: number
  items: ZabbixTemplateItemSummary[]
}

export interface ZabbixImportResult {
  id: number
  name: string
  itemCount: number
  skippedItems: Array<{ name: string; type: string }>
}

export const useZabbixTemplatesStore = defineStore('zabbixTemplates', () => {
  const templates = ref<ZabbixTemplate[]>([])
  const loading = ref(false)
  const importing = ref(false)
  const error = ref<string | null>(null)

  async function fetchTemplates() {
    loading.value = true
    error.value = null
    try {
      templates.value = await apiService.get<ZabbixTemplate[]>('/zabbix-templates')
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar templates Zabbix'
    } finally {
      loading.value = false
    }
  }

  async function importTemplate(content: string): Promise<ZabbixImportResult[] | null> {
    importing.value = true
    error.value = null
    try {
      const result = await apiService.post<{ templates: ZabbixImportResult[] }>(
        '/zabbix-templates',
        {
          content,
        }
      )
      await fetchTemplates()
      return result.templates
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao importar template Zabbix'
      return null
    } finally {
      importing.value = false
    }
  }

  async function deleteTemplate(id: number): Promise<boolean> {
    try {
      await apiService.delete(`/zabbix-templates/${id}`)
      templates.value = templates.value.filter((t) => t.id !== id)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao excluir template Zabbix'
      return false
    }
  }

  return {
    templates,
    loading,
    importing,
    error,
    fetchTemplates,
    importTemplate,
    deleteTemplate,
  }
})
