import { ref, watch, onMounted, getCurrentInstance } from 'vue'
import { useRoute } from 'vue-router'
import { useTheme } from 'vuetify'

/**
 * Cor padrão do App Bar no tema escuro padrão (#212121 é a surface padrão do Vuetify dark).
 */
export const DEFAULT_DARK_APP_BAR_COLOR = '#212121'
export const DEFAULT_LIGHT_APP_BAR_COLOR = '#ffffff'
export const AUTH_SCREEN_COLOR = '#0b0f16'

const currentThemeColor = ref<string>(DEFAULT_DARK_APP_BAR_COLOR)
const customColorOverride = ref<string | null>(null)

/**
 * Atualiza ou cria a meta tag `name="theme-color"` no `<head>` do documento.
 */
export function updateMetaThemeColor(color: string): void {
  if (typeof document === 'undefined') return

  currentThemeColor.value = color

  let meta = document.querySelector('meta[name="theme-color"]') as HTMLMetaElement | null
  if (!meta) {
    meta = document.createElement('meta')
    meta.name = 'theme-color'
    document.head.appendChild(meta)
  }
  meta.setAttribute('content', color)

  // Também sincroniza a variável CSS global no :root
  document.documentElement.style.setProperty('--pwa-app-bar-color', color)
}

/**
 * Composable que gerencia a cor do App Bar no PWA (Android status bar, iOS Safari tab bar,
 * e janela de app desktop). Mantém a barra de sistema idêntica à do aplicativo.
 */
export function usePwaThemeColor() {
  const instance = getCurrentInstance()
  let theme: ReturnType<typeof useTheme> | null = null
  let route: ReturnType<typeof useRoute> | null = null

  if (instance) {
    try {
      theme = useTheme()
    } catch {
      // Caso executado fora do contexto do Vuetify
    }

    try {
      route = useRoute()
    } catch {
      // Caso executado fora do contexto do Vue Router
    }
  }

  function resolveEffectiveColor(): string {
    if (customColorOverride.value) {
      return customColorOverride.value
    }

    // Se estiver em rota de autenticação (Login / Setup)
    if (
      route &&
      (route.name === 'login' ||
        route.name === 'setup' ||
        route.path.startsWith('/login') ||
        route.path.startsWith('/setup'))
    ) {
      return AUTH_SCREEN_COLOR
    }

    // Rota com override explícito de themeColor no meta
    if (route?.meta && typeof route.meta.themeColor === 'string') {
      return route.meta.themeColor
    }

    // Se temos acesso ao tema do Vuetify
    if (theme) {
      const current = theme.current.value
      // O v-app-bar usa a cor de superfície (surface) por padrão
      const surface = current?.colors?.surface
      if (typeof surface === 'string') {
        return surface
      }
      return current?.dark ? DEFAULT_DARK_APP_BAR_COLOR : DEFAULT_LIGHT_APP_BAR_COLOR
    }

    return DEFAULT_DARK_APP_BAR_COLOR
  }

  function syncThemeColor(): void {
    const color = resolveEffectiveColor()
    updateMetaThemeColor(color)
  }

  /**
   * Define manualmente uma cor personalizada para o App Bar / status bar.
   */
  function setThemeColor(color: string): void {
    customColorOverride.value = color
    updateMetaThemeColor(color)
  }

  /**
   * Restaura a cor automática baseada no tema e na rota ativa.
   */
  function resetThemeColor(): void {
    customColorOverride.value = null
    syncThemeColor()
  }

  // Observa mudanças de rota
  if (route) {
    watch(
      () => route?.path,
      () => {
        syncThemeColor()
      }
    )
  }

  // Observa mudanças de tema no Vuetify (se alternar dark/light ou mudar cores)
  if (theme) {
    watch(
      () => [theme?.current.value.dark, theme?.current.value.colors?.surface],
      () => {
        syncThemeColor()
      }
    )
  }

  if (instance) {
    onMounted(() => {
      syncThemeColor()
    })
  } else {
    syncThemeColor()
  }

  return {
    currentThemeColor,
    setThemeColor,
    resetThemeColor,
    syncThemeColor,
  }
}
