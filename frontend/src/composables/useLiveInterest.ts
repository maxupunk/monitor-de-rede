import { onMounted, onUnmounted } from 'vue'
import { useEventsStore } from '@/stores/events'

/**
 * Declara, enquanto o componente estiver montado, que a tela consome um dado
 * ao vivo sob demanda (ARQ-05) — ex.: `docker`, cujo snapshot o backend só
 * pede aos agentes enquanto algum stream o declara. O barramento conta as
 * referências e reabre o stream uma vez quando o conjunto muda.
 */
export function useLiveInterest(name: string): void {
  let release: (() => void) | null = null
  onMounted(() => {
    release = useEventsStore().acquireInterest(name)
  })
  onUnmounted(() => {
    release?.()
    release = null
  })
}
