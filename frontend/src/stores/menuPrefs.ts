import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { MenuPrefs } from '@/bindings/MenuPrefs'
import { apiService } from '@/services/apiService'
import { useAuthStore } from '@/stores/auth'

const EMPTY: MenuPrefs = { pinned: [], hidden: [] }

function isPathList(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
}

/** O payload tem a forma de um menu? */
export function parseMenuPrefs(value: unknown): MenuPrefs | null {
  if (typeof value !== 'object' || value === null) return null
  const pinned: unknown = Reflect.get(value, 'pinned')
  const hidden: unknown = Reflect.get(value, 'hidden')
  return isPathList(pinned) && isPathList(hidden) ? { pinned, hidden } : null
}

/**
 * O menu de cada usuário (Fase 16): fixados e ocultos, salvos no servidor.
 *
 * O estado inicial vem com a sessão (`/auth/me`); a mudança feita em outra aba
 * ou aparelho chega pelo SSE (`user:menu_updated`, só a este usuário) — sem
 * polling (AGENTS §9). Salvar é uma ação explícita do usuário.
 */
export const useMenuPrefsStore = defineStore('menuPrefs', () => {
  const auth = useAuthStore()
  const saving = ref(false)

  const prefs = computed<MenuPrefs>(() => parseMenuPrefs(auth.user?.menuPrefs) ?? EMPTY)

  function store(value: MenuPrefs) {
    if (auth.user) auth.updateUser({ ...auth.user, menuPrefs: value })
  }

  /** Grava o menu e já o aplica nesta aba. */
  async function save(value: MenuPrefs): Promise<void> {
    saving.value = true
    try {
      store(await apiService.put<MenuPrefs>('/me/menu', value))
    } finally {
      saving.value = false
    }
  }

  /** `user:menu_updated` (outra aba ou aparelho do mesmo usuário). */
  function apply(payload: unknown) {
    const userId: unknown =
      typeof payload === 'object' && payload !== null ? Reflect.get(payload, 'userId') : null
    const value = parseMenuPrefs(payload)
    if (value && userId === auth.user?.id) store(value)
  }

  return { prefs, saving, save, apply }
})
