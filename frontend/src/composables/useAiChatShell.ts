import { computed, nextTick, onMounted, type Ref } from 'vue'
import { useDisplay } from 'vuetify'
import { useAiStore } from '@/stores/ai'

/** Dica do campo da pergunta: curta no celular, com o atalho @ no resto. */
export function aiChatPlaceholder(narrow: boolean): string {
  return narrow
    ? 'Pergunte sobre a rede…'
    : 'Pergunte ou peça um diagnóstico — @ marca um equipamento'
}

interface Focusable {
  focus: () => void
}

/**
 * O que o painel lateral e a tela cheia do chat têm em comum em volta da
 * conversa: a dica do campo, as configurações carregadas ao montar e o foco
 * no campo — nunca no celular, onde o teclado cobriria a conversa.
 */
export function useAiChatShell(
  thread: Ref<Focusable | null>,
  options: { focusOnMount?: boolean } = {}
) {
  const aiStore = useAiStore()
  const display = useDisplay()

  const placeholder = computed(() => aiChatPlaceholder(display.xs.value))

  async function focusThread() {
    if (display.mobile.value) return
    await nextTick()
    thread.value?.focus()
  }

  onMounted(async () => {
    if (!aiStore.settings) await aiStore.loadSettings()
    if (options.focusOnMount) await focusThread()
  })

  return { placeholder, focusThread }
}
