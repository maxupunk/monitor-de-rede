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

export interface SetupStatus {
  needsSetup: boolean
}

/** Cadastro do primeiro usuário: exige o token de instalação do servidor. */
export interface SetupPayload {
  name: string
  email: string
  password: string
  token: string
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

function describeError(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

export const useAuthStore = defineStore('auth', () => {
  const token = ref<string | null>(localStorage.getItem('auth_token'))
  const user = ref<User | null>(loadStoredUser())
  const loading = ref(false)
  const error = ref<string | null>(null)

  /**
   * `null` enquanto o backend ainda não foi consultado — estado distinto de
   * `false`. Sem essa diferença o roteador não saberia se já pode mandar para
   * o login ou se ainda precisa perguntar, e a primeira navegação de uma
   * instalação nova cairia na tela errada.
   */
  const needsSetup = ref<boolean | null>(null)

  const isAuthenticated = computed(() => Boolean(token.value))

  function persistSession(res: LoginResponse) {
    token.value = res.token
    user.value = res.user
    localStorage.setItem('auth_token', res.token)
    persistUser(res.user)
  }

  function clearError() {
    error.value = null
  }

  /**
   * Consulta se a instalação ainda espera o primeiro usuário.
   *
   * Um backend fora do ar responde `false`: é melhor mostrar a tela de login
   * com o erro da tentativa do que prender quem chega numa tela de cadastro
   * que também não vai funcionar.
   */
  async function refreshSetupStatus(): Promise<boolean> {
    try {
      const status = await apiService.get<SetupStatus>('/auth/setup')
      needsSetup.value = status.needsSetup
    } catch {
      needsSetup.value = false
    }
    return needsSetup.value
  }

  /** Consulta uma única vez por sessão de navegação; usada pelo guard. */
  async function ensureSetupStatus(): Promise<boolean> {
    if (needsSetup.value === null) return refreshSetupStatus()
    return needsSetup.value
  }

  async function login(email: string, password: string): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      persistSession(await apiService.post<LoginResponse>('/auth/login', { email, password }))
      needsSetup.value = false
      return true
    } catch (err: unknown) {
      error.value = describeError(err, 'Falha na autenticação')
      return false
    } finally {
      loading.value = false
    }
  }

  /** Cria o primeiro usuário e já entra com ele — o backend devolve a sessão. */
  async function completeSetup(payload: SetupPayload): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      persistSession(await apiService.post<LoginResponse>('/auth/setup', payload))
      needsSetup.value = false
      return true
    } catch (err: unknown) {
      error.value = describeError(err, 'Falha ao concluir a instalação')
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

  return {
    token,
    user,
    loading,
    error,
    needsSetup,
    isAuthenticated,
    clearError,
    refreshSetupStatus,
    ensureSetupStatus,
    login,
    completeSetup,
    fetchMe,
    logout,
  }
})
