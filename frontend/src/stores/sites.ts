import { defineStore } from 'pinia'
import { useCrudResource } from './crudResource'

export interface Site {
  id: number
  name: string
  location?: string
  description?: string
  devicesCount?: number
  networksCount?: number
  createdAt?: string
  updatedAt?: string
}

export const useSitesStore = defineStore('sites', () => {
  const resource = useCrudResource<Site>('/sites', {
    fetch: 'Erro ao carregar sites',
    create: 'Erro ao criar site',
    update: 'Erro ao atualizar site',
    delete: 'Erro ao excluir site',
  })

  async function fetchSites() {
    await resource.fetchAll()
  }

  async function createSite(payload: Partial<Site>): Promise<boolean> {
    return (await resource.create(payload)) !== null
  }

  async function updateSite(id: number, payload: Partial<Site>): Promise<boolean> {
    return (await resource.update(id, payload)) !== null
  }

  async function deleteSite(id: number): Promise<boolean> {
    return resource.remove(id)
  }

  return {
    sites: resource.items,
    loading: resource.loading,
    error: resource.error,
    fetchSites,
    createSite,
    updateSite,
    deleteSite,
  }
})
