import { nextTick, ref, watch, type Ref } from 'vue'

/** Distância do fim (px) em que ainda se considera "acompanhando" a conversa. */
const NEAR_BOTTOM_PX = 80

/**
 * Rolagem de uma conversa que cresce enquanto a IA escreve.
 *
 * Acompanha o fim só enquanto quem lê está nele: rolar para cima para reler
 * uma resposta não é interrompido a cada pedaço que chega — aparece o botão
 * de voltar ao fim. Mensagem nova (a pergunta que acabou de ser enviada)
 * sempre leva ao fim.
 */
export function useChatAutoScroll(
  container: Ref<HTMLElement | null>,
  messageCount: () => number,
  growth: () => unknown
) {
  const following = ref(true)
  const showJump = ref(false)

  function nearBottom(element: HTMLElement): boolean {
    return element.scrollHeight - element.scrollTop - element.clientHeight <= NEAR_BOTTOM_PX
  }

  function onScroll() {
    const element = container.value
    if (!element) return
    following.value = nearBottom(element)
    showJump.value = !following.value
  }

  function scrollToBottom(smooth = false) {
    void nextTick(() => {
      const element = container.value
      if (!element) return
      element.scrollTo({ top: element.scrollHeight, behavior: smooth ? 'smooth' : 'auto' })
      following.value = true
      showJump.value = false
    })
  }

  watch(messageCount, () => scrollToBottom())
  watch(growth, () => {
    if (following.value) scrollToBottom()
    else showJump.value = true
  })

  return { onScroll, scrollToBottom, showJump }
}
