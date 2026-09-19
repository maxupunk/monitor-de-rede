/**
 * Utilitários de persistência e leitura de sessão de autenticação.
 * Garante paridade entre `sessionStorage` e `localStorage` em todo o frontend,
 * evitando discrepâncias de cabeçalho e loops de redirecionamento.
 */

export function getStoredToken(): string | null {
  try {
    return sessionStorage.getItem('auth_token') ?? localStorage.getItem('auth_token')
  } catch {
    return null
  }
}

export function getStoredUser<T = unknown>(): T | null {
  try {
    const rawUser = sessionStorage.getItem('auth_user') ?? localStorage.getItem('auth_user')
    if (!rawUser) return null
    return JSON.parse(rawUser) as T
  } catch {
    clearStoredAuth()
    return null
  }
}

export function persistAuthSession(token: string, user: unknown, remember = false): void {
  try {
    const primary = remember ? localStorage : sessionStorage
    const secondary = remember ? sessionStorage : localStorage
    primary.setItem('auth_token', token)
    primary.setItem('auth_user', JSON.stringify(user))
    secondary.removeItem('auth_token')
    secondary.removeItem('auth_user')
  } catch (error) {
    console.error('Erro ao persistir sessão de autenticação:', error)
  }
}

export function clearStoredAuth(): void {
  try {
    sessionStorage.removeItem('auth_token')
    sessionStorage.removeItem('auth_user')
    localStorage.removeItem('auth_token')
    localStorage.removeItem('auth_user')
  } catch (error) {
    console.error('Erro ao limpar sessão de autenticação:', error)
  }
}

export function isRememberMeActive(): boolean {
  try {
    return (
      sessionStorage.getItem('auth_token') === null && localStorage.getItem('auth_token') !== null
    )
  } catch {
    return false
  }
}
