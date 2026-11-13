import { defineStore } from 'pinia'
import { ref } from 'vue'

/**
 * O que a tela aberta conta ao chat além da rota: o que está filtrado ou
 * selecionado ("severidade erro, ap-sala"), e se o usuário dispensou o chip
 * da tela para perguntar sem ela.
 *
 * Quem preenche o detalhe é a própria página (`useAiScreenDetail`); quem lê a
 * rota é o chat (`utils/aiScreen`). A store não conhece o router.
 */
export const useAiScreenStore = defineStore('aiScreen', () => {
  const detail = ref<string | null>(null)
  /** A tela (chave de `currentScreen`) cujo chip o usuário fechou. */
  const dismissedKey = ref<string | null>(null)
  let owner: symbol | null = null

  /** A página assume o detalhe; outra que monte depois toma o lugar. */
  function claimDetail(): symbol {
    owner = Symbol('aiScreenDetail')
    detail.value = null
    return owner
  }

  function setDetail(token: symbol, value: string | null) {
    if (token === owner) detail.value = value
  }

  /** Só quem é dono limpa: a página que sai não apaga a que entrou. */
  function releaseDetail(token: symbol) {
    if (token !== owner) return
    owner = null
    detail.value = null
  }

  function dismiss(key: string) {
    dismissedKey.value = key
  }

  function restore() {
    dismissedKey.value = null
  }

  function isDismissed(key: string): boolean {
    return dismissedKey.value === key
  }

  return {
    detail,
    dismissedKey,
    claimDetail,
    setDetail,
    releaseDetail,
    dismiss,
    restore,
    isDismissed,
  }
})
