import { onBeforeUnmount, watch } from 'vue'
import { useAiScreenStore } from '@/stores/aiScreen'

/**
 * A página diz ao chat o que está filtrado ou selecionado nela, numa linha
 * ("dispositivo ap-sala, severidade erro, busca 'dhcp'"). Acompanha o que a
 * função devolve e some quando a página desmonta.
 *
 * Só texto que a pessoa vê na tela — nada de segredo.
 */
export function useAiScreenDetail(describe: () => string | null | undefined): void {
  const store = useAiScreenStore()
  const token = store.claimDetail()
  watch(describe, (value) => store.setDetail(token, value?.trim() || null), { immediate: true })
  onBeforeUnmount(() => store.releaseDetail(token))
}
