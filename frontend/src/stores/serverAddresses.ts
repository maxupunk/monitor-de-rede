import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { ServerAddressEntry } from '@/bindings/ServerAddressEntry'
import type { ServerAddressesResponse } from '@/bindings/ServerAddressesResponse'

export type { ServerAddressEntry }

/**
 * Por onde os equipamentos alcançam este servidor.
 *
 * Um servidor, várias portas de entrada: quem está na mesma rede usa o IP da
 * LAN, quem chega pelo túnel usa o endereço da VPN, quem vem de outro local usa
 * o IP público ou o DDNS. A lista é montada pelo servidor — três entradas são
 * detectadas e o operador só acrescenta ou corrige o que faltar.
 */
export const useServerAddressesStore = defineStore('serverAddresses', () => {
  const entries = ref<ServerAddressEntry[]>([])
  const preferredId = ref<string | null>(null)
  const loading = ref(false)
  const saving = ref(false)
  const loaded = ref(false)
  const error = ref('')

  /** Só as entradas que têm endereço — as outras não podem ser oferecidas. */
  const usable = computed(() => entries.value.filter((entrada) => Boolean(entrada.value)))

  /** Se ainda não há nenhum endereço utilizável, a tela precisa pedir um. */
  const isEmpty = computed(() => loaded.value && usable.value.length === 0)

  async function fetchAll(force = false): Promise<void> {
    if (loading.value) return
    if (loaded.value && !force) return
    loading.value = true
    error.value = ''
    try {
      const resposta = await apiService.get<ServerAddressesResponse>('/server-addresses')
      entries.value = resposta.data ?? []
      preferredId.value = resposta.preferredId ?? null
      loaded.value = true
    } catch (erro) {
      error.value = erro instanceof Error ? erro.message : 'Não foi possível carregar os endereços.'
    } finally {
      loading.value = false
    }
  }

  /**
   * Grava o documento do operador — correções e personalizados.
   *
   * Os detectados **não** vão de volta: gravá-los os congelaria, e o IP da rede
   * local mudaria sem a tela perceber. Correção em branco desfaz a correção.
   */
  async function save(payload: {
    overrides: Record<string, string>
    custom: { id: string; label: string; value: string }[]
    preferredId: string | null
  }): Promise<boolean> {
    saving.value = true
    error.value = ''
    try {
      const resposta = await apiService.put<ServerAddressesResponse>('/server-addresses', payload)
      entries.value = resposta.data ?? []
      preferredId.value = resposta.preferredId ?? null
      loaded.value = true
      return true
    } catch (erro) {
      error.value = erro instanceof Error ? erro.message : 'Não foi possível salvar os endereços.'
      return false
    } finally {
      saving.value = false
    }
  }

  /** A entrada de um id, para a tela mostrar rótulo e motivo juntos. */
  function byId(id: string | null | undefined): ServerAddressEntry | null {
    if (!id) return null
    return entries.value.find((entrada) => entrada.id === id) ?? null
  }

  return {
    entries,
    preferredId,
    loading,
    saving,
    loaded,
    error,
    usable,
    isEmpty,
    fetchAll,
    save,
    byId,
  }
})

/** Ícone de cada tipo. Mora aqui porque o gerenciador e o seletor usam o mesmo. */
export function addressIcon(kind: string): string {
  switch (kind) {
    case 'lan':
      return 'mdi-lan'
    case 'vpn':
      return 'mdi-shield-lock-outline'
    case 'public':
      return 'mdi-web'
    default:
      return 'mdi-map-marker-outline'
  }
}

export function addressColor(kind: string): string {
  switch (kind) {
    case 'lan':
      return 'primary'
    case 'vpn':
      return 'deep-purple'
    case 'public':
      return 'teal'
    default:
      return 'blue-grey'
  }
}
