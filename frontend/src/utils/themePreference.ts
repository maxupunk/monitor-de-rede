/**
 * A preferência de tema do usuário: escuro (padrão), claro ou o do sistema.
 * Fica no navegador — é do aparelho, não da conta: o mesmo operador pode
 * querer claro no notebook e escuro na TV do NOC. Puro e testável; quem
 * aplica no Vuetify é `useThemePreference`.
 */
export type ThemePreference = 'dark' | 'light' | 'system'
export type ThemeName = 'dark' | 'light'

export const THEME_STORAGE_KEY = 'netmonitor.theme'

export const THEME_OPTIONS: { value: ThemePreference; label: string; icon: string }[] = [
  { value: 'dark', label: 'Escuro', icon: 'mdi-weather-night' },
  { value: 'light', label: 'Claro', icon: 'mdi-white-balance-sunny' },
  { value: 'system', label: 'Do sistema', icon: 'mdi-monitor' },
]

export function isPreference(value: unknown): value is ThemePreference {
  return value === 'dark' || value === 'light' || value === 'system'
}

/** O tema que vale agora para uma preferência. */
export function resolveTheme(preference: ThemePreference, systemPrefersDark: boolean): ThemeName {
  if (preference === 'system') return systemPrefersDark ? 'dark' : 'light'
  return preference
}

/** A preferência gravada; sem gravação (ou lixo), o escuro de sempre. */
export function loadThemePreference(storage: Pick<Storage, 'getItem'> | null): ThemePreference {
  try {
    const raw = storage?.getItem(THEME_STORAGE_KEY)
    return isPreference(raw) ? raw : 'dark'
  } catch {
    return 'dark'
  }
}

export function saveThemePreference(
  storage: Pick<Storage, 'setItem'> | null,
  preference: ThemePreference
): void {
  try {
    storage?.setItem(THEME_STORAGE_KEY, preference)
  } catch {
    // Navegação privada sem armazenamento: vale só nesta aba.
  }
}

/** O sistema operacional prefere escuro? (sem `matchMedia`, assume que sim). */
export function systemPrefersDark(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return true
  return window.matchMedia('(prefers-color-scheme: dark)').matches
}

/** O tema para abrir o app já certo, antes de montar (sem piscar o escuro). */
export function initialThemeName(): ThemeName {
  const storage = typeof localStorage === 'undefined' ? null : localStorage
  return resolveTheme(loadThemePreference(storage), systemPrefersDark())
}
