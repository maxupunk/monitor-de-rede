import { ref, type Ref } from 'vue'
import { apiService } from '@/services/apiService'

/**
 * Toda store de cadastro simples (sites, redes, dispositivos, servidores DNS,
 * templates Zabbix) repetia o mesmo par fetch/create/update/delete contra
 * `apiService`, só trocando o path e as mensagens de erro. Esta factory
 * centraliza esse padrão — cada store mantém seu próprio `defineStore` e pode
 * adicionar estado/ações extras por cima do que é devolvido aqui.
 */

export interface CrudMessages {
  fetch?: string
  create?: string
  update?: string
  delete?: string
}

const DEFAULT_MESSAGES: Required<CrudMessages> = {
  fetch: 'Erro ao carregar dados',
  create: 'Erro ao criar registro',
  update: 'Erro ao atualizar registro',
  delete: 'Erro ao excluir registro',
}

export function useCrudResource<T extends { id: number }>(
  basePath: string,
  messages: CrudMessages = {}
) {
  const msg = { ...DEFAULT_MESSAGES, ...messages }

  const items = ref<T[]>([]) as Ref<T[]>
  const loading = ref(false)
  const error = ref<string | null>(null)

  function describeError(err: unknown, fallback: string): string {
    return err instanceof Error ? err.message : fallback
  }

  async function fetchAll(): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      items.value = await apiService.get<T[]>(basePath)
      return true
    } catch (err: unknown) {
      error.value = describeError(err, msg.fetch)
      return false
    } finally {
      loading.value = false
    }
  }

  async function create(payload: Partial<T>): Promise<T | null> {
    error.value = null
    try {
      const created = await apiService.post<T>(basePath, payload)
      items.value.push(created)
      return created
    } catch (err: unknown) {
      error.value = describeError(err, msg.create)
      return null
    }
  }

  async function update(id: number, payload: Partial<T>): Promise<T | null> {
    error.value = null
    try {
      const updated = await apiService.put<T>(`${basePath}/${id}`, payload)
      const index = items.value.findIndex((item) => item.id === id)
      if (index !== -1) items.value[index] = updated
      return updated
    } catch (err: unknown) {
      error.value = describeError(err, msg.update)
      return null
    }
  }

  async function remove(id: number): Promise<boolean> {
    error.value = null
    try {
      await apiService.delete(`${basePath}/${id}`)
      items.value = items.value.filter((item) => item.id !== id)
      return true
    } catch (err: unknown) {
      error.value = describeError(err, msg.delete)
      return false
    }
  }

  return { items, loading, error, fetchAll, create, update, remove }
}
