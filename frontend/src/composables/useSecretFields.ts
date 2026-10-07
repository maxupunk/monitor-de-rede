import { ref, type Ref } from 'vue'
import { requiredRule } from '@/utils/formRules'

/**
 * Campos de segredo de um formulário de cadastro (senha, chave de acesso).
 *
 * Mascarados, com o olho para conferir o que se digitou, e sem
 * autopreenchimento — o navegador não pode oferecer a senha do login aqui. Na
 * edição o segredo gravado nunca vem: o campo vazio mantém o atual, e a dica
 * diz isso.
 */
export function useSecretFields(labels: Record<string, string>) {
  /** Segredos que já estão gravados (vêm do backend como nomes). */
  const stored: Ref<string[]> = ref([])
  const visible: Ref<string[]> = ref([])

  function toggle(name: string) {
    visible.value = visible.value.includes(name)
      ? visible.value.filter((item) => item !== name)
      : [...visible.value, name]
  }

  /** Dica de "já gravado" — também para áreas de texto, que não têm o olho. */
  function hint(name: string) {
    return stored.value.includes(name)
      ? {
          placeholder: '••••••••',
          hint: `${labels[name] ?? 'Valor'} gravado — deixe em branco para manter.`,
          'persistent-hint': true,
        }
      : {}
  }

  /** Props de um `v-text-field` de segredo. */
  function field(name: string, requiredOnCreate = true) {
    const shown = visible.value.includes(name)
    return {
      type: shown ? 'text' : 'password',
      autocomplete: 'new-password',
      'append-inner-icon': shown ? 'mdi-eye-off' : 'mdi-eye',
      'onClick:appendInner': () => toggle(name),
      rules: requiredOnCreate && !stored.value.includes(name) ? [requiredRule('Obrigatório')] : [],
      ...hint(name),
    }
  }

  function reset(names: string[] = []) {
    stored.value = names
    visible.value = []
  }

  return { stored, field, hint, reset }
}
