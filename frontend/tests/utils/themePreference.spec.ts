import { describe, expect, it } from 'vitest'
import {
  THEME_STORAGE_KEY,
  isPreference,
  loadThemePreference,
  resolveTheme,
  saveThemePreference,
} from '@/utils/themePreference'

function memoryStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial))
  return {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
    data,
  }
}

describe('themePreference', () => {
  it('resolve "Do sistema" pelo sistema operacional e respeita a escolha fixa', () => {
    expect(resolveTheme('system', true)).toBe('dark')
    expect(resolveTheme('system', false)).toBe('light')
    expect(resolveTheme('light', true)).toBe('light')
    expect(resolveTheme('dark', false)).toBe('dark')
  })

  it('sem escolha gravada (ou com lixo), abre no escuro de sempre', () => {
    expect(loadThemePreference(memoryStorage())).toBe('dark')
    expect(loadThemePreference(memoryStorage({ [THEME_STORAGE_KEY]: 'roxo' }))).toBe('dark')
    expect(loadThemePreference(null)).toBe('dark')
  })

  it('grava e relê a escolha', () => {
    const storage = memoryStorage()
    saveThemePreference(storage, 'light')
    expect(storage.data.get(THEME_STORAGE_KEY)).toBe('light')
    expect(loadThemePreference(storage)).toBe('light')
  })

  it('armazenamento bloqueado não derruba a tela', () => {
    const blocked = {
      getItem: () => {
        throw new Error('bloqueado')
      },
      setItem: () => {
        throw new Error('bloqueado')
      },
    }
    expect(loadThemePreference(blocked)).toBe('dark')
    expect(() => saveThemePreference(blocked, 'light')).not.toThrow()
  })

  it('só aceita as três opções', () => {
    expect(isPreference('system')).toBe(true)
    expect(isPreference('auto')).toBe(false)
  })
})
