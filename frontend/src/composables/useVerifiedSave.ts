import { ref, watch, type Ref } from 'vue'

/**
 * "Testar conexão" e "Salvar testa antes" de um formulário de destino.
 *
 * - O teste vale para a configuração em que rodou: mudou um campo da conexão
 *   (`signature`), o resultado some.
 * - Salvar roda o teste se ele não estiver valendo. Se o destino não responder,
 *   o botão vira "Salvar mesmo assim" (`forceSave`): dá para cadastrar um
 *   servidor desligado agora, mas não sem saber.
 *
 * Usado pelos formulários de Armazenamento e de Conexão de banco.
 */
export function useVerifiedSave<T>(options: {
  /** Identidade da configuração testável (JSON do que o teste usa). */
  signature: () => string
  /** Roda o teste; erro de requisição deve ser lançado. */
  test: () => Promise<T>
  /** O resultado é um sucesso? */
  succeeded: (result: T) => boolean
}) {
  const result = ref<T | null>(null) as Ref<T | null>
  const testing = ref(false)
  const forceSave = ref(false)
  const error = ref<string | null>(null)
  const testedSignature = ref<string | null>(null)

  watch(
    () => options.signature(),
    (current) => {
      if (testedSignature.value && current !== testedSignature.value) reset()
    }
  )

  function reset() {
    result.value = null
    testedSignature.value = null
    forceSave.value = false
    error.value = null
  }

  /** Roda o teste agora. `null` quando a própria requisição falhou. */
  async function run(): Promise<T | null> {
    testing.value = true
    error.value = null
    try {
      const current = options.signature()
      result.value = await options.test()
      testedSignature.value = current
      return result.value
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Erro ao testar a conexão'
      return null
    } finally {
      testing.value = false
    }
  }

  /**
   * Pode salvar? Testa se preciso; numa falha arma o "Salvar mesmo assim" e
   * devolve `false`. Com ele armado, devolve `true` sem testar de novo.
   */
  async function ready(): Promise<{ ok: boolean; result: T | null }> {
    if (forceSave.value) return { ok: true, result: result.value }
    const fresh = result.value !== null && testedSignature.value === options.signature()
    const current = fresh ? result.value : await run()
    if (current === null || !options.succeeded(current)) {
      forceSave.value = true
      return { ok: false, result: current }
    }
    return { ok: true, result: current }
  }

  return { result, testing, forceSave, error, run, ready, reset }
}
