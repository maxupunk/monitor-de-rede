import { describe, it, expect, beforeEach, vi } from 'vitest'
import {
  usePwaThemeColor,
  updateMetaThemeColor,
  DEFAULT_DARK_APP_BAR_COLOR,
  DEFAULT_LIGHT_APP_BAR_COLOR,
  AUTH_SCREEN_COLOR,
} from '../../src/composables/usePwaThemeColor'

describe('usePwaThemeColor', () => {
  beforeEach(() => {
    // Limpa meta tags de teste no DOM
    const metas = document.querySelectorAll('meta[name="theme-color"]')
    metas.forEach((m) => m.remove())
    document.documentElement.style.removeProperty('--pwa-app-bar-color')
  })

  it('cria a meta tag theme-color no head se ela não existir', () => {
    updateMetaThemeColor('#FF5500')

    const meta = document.querySelector('meta[name="theme-color"]') as HTMLMetaElement | null
    expect(meta).not.toBeNull()
    expect(meta?.getAttribute('content')).toBe('#FF5500')
    expect(document.documentElement.style.getPropertyValue('--pwa-app-bar-color')).toBe('#FF5500')
  })

  it('atualiza a meta tag existente quando o valor muda', () => {
    const initialMeta = document.createElement('meta')
    initialMeta.name = 'theme-color'
    initialMeta.content = '#111111'
    document.head.appendChild(initialMeta)

    updateMetaThemeColor('#222222')

    const meta = document.querySelector('meta[name="theme-color"]') as HTMLMetaElement | null
    expect(meta?.getAttribute('content')).toBe('#222222')
    expect(document.documentElement.style.getPropertyValue('--pwa-app-bar-color')).toBe('#222222')
  })

  it('permite definir cor customizada com setThemeColor e restaurar com resetThemeColor', () => {
    const { setThemeColor, resetThemeColor, currentThemeColor } = usePwaThemeColor()

    setThemeColor('#1976D2')
    expect(currentThemeColor.value).toBe('#1976D2')
    let meta = document.querySelector('meta[name="theme-color"]')
    expect(meta?.getAttribute('content')).toBe('#1976D2')

    resetThemeColor()
    expect(currentThemeColor.value).toBe(DEFAULT_DARK_APP_BAR_COLOR)
    meta = document.querySelector('meta[name="theme-color"]')
    expect(meta?.getAttribute('content')).toBe(DEFAULT_DARK_APP_BAR_COLOR)
  })

  it('exporta constantes essenciais de cor para PWA', () => {
    expect(DEFAULT_DARK_APP_BAR_COLOR).toBe('#212121')
    expect(DEFAULT_LIGHT_APP_BAR_COLOR).toBe('#ffffff')
    expect(AUTH_SCREEN_COLOR).toBe('#0b0f16')
  })
})
