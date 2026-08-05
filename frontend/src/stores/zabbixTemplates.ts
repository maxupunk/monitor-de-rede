import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import { useCrudResource } from './crudResource'
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
  const resource = useCrudResource<ZabbixTemplate>('/zabbix-templates', {
    fetch: 'Erro ao carregar templates Zabbix',
    delete: 'Erro ao excluir template Zabbix',
  })
  const templates = resource.items
  const importing = ref(false)

  async function fetchTemplates() {
    await resource.fetchAll()
  }

  async function importTemplate(content: string): Promise<ZabbixImportResult[] | null> {
    importing.value = true
    resource.error.value = null
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
      resource.error.value = err instanceof Error ? err.message : 'Erro ao importar template Zabbix'
      return null
    } finally {
      importing.value = false
    }
  }

  async function deleteTemplate(id: number): Promise<boolean> {
    return resource.remove(id)
  }

  return {
    templates,
    loading: resource.loading,
    importing,
    error: resource.error,
    fetchTemplates,
    importTemplate,
    deleteTemplate,
  }
})
