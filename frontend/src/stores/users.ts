import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
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

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}

export const useUsersStore = defineStore('users', () => {
  const users = ref<ManagedUser[]>([])
  const loading = ref(false)
  const saving = ref(false)
  const error = ref<string | null>(null)

  async function fetchUsers() {
    loading.value = true
    error.value = null
    try {
      users.value = await apiService.get<ManagedUser[]>('/users')
    } catch (err) {
      error.value = errorMessage(err, 'Não foi possível carregar os usuários.')
    } finally {
      loading.value = false
    }
  }

  async function createUser(payload: CreateUserPayload): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      const created = await apiService.post<ManagedUser>('/users', payload)
      users.value = [...users.value, created].sort((a, b) =>
        a.fullName.localeCompare(b.fullName, 'pt-BR')
      )
      return true
    } catch (err) {
      error.value = errorMessage(err, 'Não foi possível criar o usuário.')
      return false
    } finally {
      saving.value = false
    }
  }

  async function updateUser(id: number, payload: UpdateUserPayload): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      const updated = await apiService.put<ManagedUser>(`/users/${id}`, payload)
      users.value = users.value.map((user) => (user.id === id ? updated : user))
      return true
    } catch (err) {
      error.value = errorMessage(err, 'Não foi possível atualizar o usuário.')
      return false
    } finally {
      saving.value = false
    }
  }

  async function deleteUser(id: number): Promise<boolean> {
    saving.value = true
    error.value = null
    try {
      await apiService.delete(`/users/${id}`)
      users.value = users.value.filter((user) => user.id !== id)
      return true
    } catch (err) {
      error.value = errorMessage(err, 'Não foi possível excluir o usuário.')
      return false
    } finally {
      saving.value = false
    }
  }

  function clearError() {
    error.value = null
  }

  return {
    users,
    loading,
    saving,
    error,
    fetchUsers,
    createUser,
    updateUser,
    deleteUser,
    clearError,
  }
})
