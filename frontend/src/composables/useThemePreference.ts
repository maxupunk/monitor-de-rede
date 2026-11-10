import { ref } from 'vue'
import { useTheme } from 'vuetify'
import {
  loadThemePreference,
  resolveTheme,
  saveThemePreference,
  systemPrefersDark,
  type ThemePreference,
} from '@/utils/themePreference'

/** Uma preferência só para o app inteiro (o menu e o `App.vue` leem a mesma). */
const preference = ref<ThemePreference>(
  loadThemePreference(typeof localStorage === 'undefined' ? null : localStorage)
)
let listening = false

/**
 * Aplica a preferência de tema no Vuetify e, em "Do sistema", acompanha a
 * troca do sistema operacional pelo evento do `matchMedia` — nada de
 * consultar de tempos em tempos.
 */
export function useThemePreference() {
  const theme = useTheme()

  function apply() {
    theme.change(resolveTheme(preference.value, systemPrefersDark()))
  }

  function choose(next: ThemePreference) {
    preference.value = next
    saveThemePreference(typeof localStorage === 'undefined' ? null : localStorage, next)
    apply()
  }

  if (!listening && typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
    listening = true
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
      if (preference.value === 'system') apply()
    })
  }

  return { preference, choose, apply }
}
