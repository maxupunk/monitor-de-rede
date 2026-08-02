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

export const useAuthStore = defineStore('auth', () => {
  const token = ref<string | null>(localStorage.getItem('auth_token'))
  const user = ref<User | null>(null)
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
      const userData = await apiService.get<User>('/auth/me')
      user.value = userData
    } catch {
      logout()
    }
  }

  function logout() {
    token.value = null
    user.value = null
    localStorage.removeItem('auth_token')
  }

  return { token, user, loading, error, isAuthenticated, login, fetchMe, logout }
})
