import { ref } from 'vue'
import { revealFields, type Schema } from '@/utils/pluginSettings'

/** Quem lê o segredo: o equipamento, a ação que revela e os valores do formulário. */
export type RevealFn = (
  deviceId: number,
  action: string,
  params: Record<string, unknown>
) => Promise<Record<string, unknown>>

/**
 * Tira dos parâmetros o segredo que o operador não mudou: a senha que veio do
 * equipamento e voltaria igual não vira uma falsa "troca de senha" na revisão.
 */
export function withoutUnchanged(
  params: Record<string, unknown>,
  revealed: Record<string, unknown>
): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(params).filter(
      ([name, value]) => !(name in revealed) || revealed[name] !== value
    )
  )
}

/**
 * Os segredos que um formulário de edição traz do equipamento para o operador
 * conferir (campo `secret` com `reveal` no esquema). O valor chega só a esta
 * tela — o backend não grava nem publica — e fica guardado aqui para saber se
 * o operador o mudou.
 */
export function useSecretReveal(reveal: RevealFn) {
  /** Campo → valor revelado. */
  const revealed = ref<Record<string, unknown>>({})
  /** Campo → texto que explica de onde veio (ou por que não veio). */
  const notes = ref<Record<string, string>>({})

  function reset() {
    revealed.value = {}
    notes.value = {}
  }

  /**
   * Lê os segredos do formulário no equipamento e devolve os valores para
   * preencher os campos. Falha não impede a edição: o campo fica vazio, que
   * mantém o atual.
   */
  async function load(
    schema: Schema | null | undefined,
    values: Record<string, unknown>,
    device: { deviceId: number; name: string }
  ): Promise<Record<string, unknown>> {
    reset()
    const fields = revealFields(schema)
    const filled: Record<string, unknown> = {}
    for (const field of fields) {
      notes.value = { ...notes.value, [field.name]: `Lendo a senha atual em ${device.name}…` }
      try {
        const output = await reveal(device.deviceId, field.action, values)
        const value = output[field.name]
        if (typeof value === 'string' && value !== '') {
          filled[field.name] = value
          revealed.value = { ...revealed.value, [field.name]: value }
          notes.value = {
            ...notes.value,
            [field.name]: `Senha atual, lida agora de ${device.name}. Mude só se quiser trocar.`,
          }
        } else {
          notes.value = {
            ...notes.value,
            [field.name]: `${device.name} não tem senha nesta rede. Digite uma para criar.`,
          }
        }
      } catch (err: unknown) {
        const reason = err instanceof Error ? err.message : 'falha na leitura'
        notes.value = {
          ...notes.value,
          [field.name]: `Não deu para ler a senha atual (${reason}). Deixe em branco para manter.`,
        }
      }
    }
    return filled
  }

  return { revealed, notes, load, reset }
}
