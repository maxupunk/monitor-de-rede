import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  clearStoredAuth,
  getStoredToken,
  getStoredUser,
  isRememberMeActive,
  persistAuthSession,
} from '@/utils/authStorage'

function createStorageMock(): Storage {
  let store: Record<string, string> = {}
  return {
    getItem: (key: string) => store[key] ?? null,
    setItem: (key: string, value: string) => {
      store[key] = String(value)
    },
    removeItem: (key: string) => {
      delete store[key]
    },
    clear: () => {
      store = {}
    },
    get length() {
      return Object.keys(store).length
    },
    key: (index: number) => Object.keys(store)[index] ?? null,
  }
}

describe('authStorage', () => {
  beforeEach(() => {
    vi.stubGlobal('sessionStorage', createStorageMock())
    vi.stubGlobal('localStorage', createStorageMock())
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('lê token do sessionStorage com prioridade ou do localStorage como fallback', () => {
    expect(getStoredToken()).toBeNull()

    localStorage.setItem('auth_token', 'token-local')
    expect(getStoredToken()).toBe('token-local')

    sessionStorage.setItem('auth_token', 'token-session')
    expect(getStoredToken()).toBe('token-session')
  })

  it('persiste sessão no sessionStorage quando rememberMe = false e limpa localStorage', () => {
    localStorage.setItem('auth_token', 'stale-token')
    localStorage.setItem('auth_user', '{"id":1}')

    persistAuthSession('new-session-token', { id: 2, email: 'user@test.com' }, false)

    expect(sessionStorage.getItem('auth_token')).toBe('new-session-token')
    expect(sessionStorage.getItem('auth_user')).toContain('user@test.com')
    expect(localStorage.getItem('auth_token')).toBeNull()
    expect(localStorage.getItem('auth_user')).toBeNull()
    expect(isRememberMeActive()).toBe(false)
  })

  it('persiste sessão no localStorage quando rememberMe = true e limpa sessionStorage', () => {
    sessionStorage.setItem('auth_token', 'stale-token')
    sessionStorage.setItem('auth_user', '{"id":1}')

    persistAuthSession('new-local-token', { id: 3, email: 'admin@test.com' }, true)

    expect(localStorage.getItem('auth_token')).toBe('new-local-token')
    expect(localStorage.getItem('auth_user')).toContain('admin@test.com')
    expect(sessionStorage.getItem('auth_token')).toBeNull()
    expect(sessionStorage.getItem('auth_user')).toBeNull()
    expect(isRememberMeActive()).toBe(true)
  })

  it('clearStoredAuth limpa tanto sessionStorage quanto localStorage', () => {
    sessionStorage.setItem('auth_token', 'token-1')
    sessionStorage.setItem('auth_user', '{"id":1}')
    localStorage.setItem('auth_token', 'token-2')
    localStorage.setItem('auth_user', '{"id":2}')

    clearStoredAuth()

    expect(sessionStorage.getItem('auth_token')).toBeNull()
    expect(sessionStorage.getItem('auth_user')).toBeNull()
    expect(localStorage.getItem('auth_token')).toBeNull()
    expect(localStorage.getItem('auth_user')).toBeNull()
    expect(getStoredToken()).toBeNull()
    expect(getStoredUser()).toBeNull()
  })
})
