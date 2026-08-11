import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { apiService } from '@/services/apiService'

export interface User {
  id: number
  email: string
  fullName?: string
  role?: string
}

export interface LoginResponse {
  token: string
  user: User
}

function loadStoredUser(): User | null {
  const rawUser = localStorage.getItem('auth_user')
  if (!rawUser) return null
  try {
    return JSON.parse(rawUser) as User
  } catch {
    localStorage.removeItem('auth_user')
    return null
  }
}

function persistUser(value: User | null) {
  if (value) {
    localStorage.setItem('auth_user', JSON.stringify(value))
  } else {
    localStorage.removeItem('auth_user')
  }
}

export const useAuthStore = defineStore('auth', () => {
  const token = ref<string | null>(localStorage.getItem('auth_token'))
  const user = ref<User | null>(loadStoredUser())
  const loading = ref(false)
  const error = ref<string | null>(null)

  const isAuthenticated = computed(() => Boolean(token.value))

  async function login(email: string, password: string): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      const res = await apiService.post<LoginResponse>('/auth/login', { email, password })
      token.value = res.token
      user.value = res.user
      localStorage.setItem('auth_token', res.token)
      persistUser(res.user)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Falha na autenticação'
      return false
    } finally {
      loading.value = false
    }
  }

  async function fetchMe() {
    if (!token.value) return
    try {
      const userData = await apiService.get<User | { user: User }>('/auth/me')
      user.value = 'user' in userData ? userData.user : userData
      persistUser(user.value)
    } catch {
      void logout()
    }
  }

  async function logout() {
    try {
      if (token.value) await apiService.post('/auth/logout')
    } catch {
      // O token pode já ter expirado; a limpeza local continua obrigatória.
    } finally {
      token.value = null
      user.value = null
      localStorage.removeItem('auth_token')
      persistUser(null)
    }
  }

  return { token, user, loading, error, isAuthenticated, login, fetchMe, logout }
})
