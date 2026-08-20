import { defineStore } from 'pinia'
import { ref } from 'vue'
import { useCrudResource } from './crudResource'
import type { UserRole } from '@/utils/access'

export interface ManagedUser {
  id: number
  email: string
  fullName: string
  role: UserRole
  active: boolean
  createdAt: string
  updatedAt: string
}

export interface CreateUserPayload {
  name: string
  email: string
  password: string
  role: UserRole
  active: boolean
}

export interface UpdateUserPayload extends CreateUserPayload {
  password: string
}

export const useUsersStore = defineStore('users', () => {
  const resource = useCrudResource<ManagedUser>('/users', {
    fetch: 'Não foi possível carregar os usuários.',
    create: 'Não foi possível criar o usuário.',
    update: 'Não foi possível atualizar o usuário.',
    delete: 'Não foi possível excluir o usuário.',
  })

  const saving = ref(false)

  async function fetchUsers(): Promise<boolean> {
    const ok = await resource.fetchAll()
    if (ok) {
      resource.items.value.sort((a, b) => a.fullName.localeCompare(b.fullName, 'pt-BR'))
    }
    return ok
  }

  async function createUser(payload: CreateUserPayload): Promise<boolean> {
    saving.value = true
    try {
      const created = await resource.create(payload as unknown as Partial<ManagedUser>)
      if (created) {
        resource.items.value.sort((a, b) => a.fullName.localeCompare(b.fullName, 'pt-BR'))
        return true
      }
      return false
    } finally {
      saving.value = false
    }
  }

  async function updateUser(id: number, payload: UpdateUserPayload): Promise<boolean> {
    saving.value = true
    try {
      const updated = await resource.update(id, payload as unknown as Partial<ManagedUser>)
      return updated !== null
    } finally {
      saving.value = false
    }
  }

  async function deleteUser(id: number): Promise<boolean> {
    saving.value = true
    try {
      return await resource.remove(id)
    } finally {
      saving.value = false
    }
  }

  function clearError() {
    resource.error.value = null
  }

  return {
    users: resource.items,
    loading: resource.loading,
    saving,
    error: resource.error,
    fetchUsers,
    createUser,
    updateUser,
    deleteUser,
    clearError,
  }
})
