/**
 * Regras de validação compartilhadas pelos formulários de autenticação.
 *
 * Ficam fora dos componentes porque as mesmas regras valem no login e no
 * primeiro acesso, e porque um limite duplicado é um limite que vai divergir:
 * o mínimo de senha muda no backend, alguém ajusta numa tela e esquece a outra.
 *
 * A validação aqui é conveniência — evita a viagem até o servidor. Quem decide
 * continua sendo o backend (`services/auth/setup.rs`).
 */

/** Assinatura que o `:rules` do Vuetify espera. */
export type ValidationRule = (value: string) => true | string

/** A parte do `v-form` que os formulários daqui usam. */
export type VuetifyForm = { validate: () => Promise<{ valid: boolean }> }

/** Espelha o `#[validate(length(min = 8))]` do `SetupParams`. */
export const MIN_PASSWORD_LENGTH = 8

/** Espelha o `#[validate(length(min = 2))]` de `name`. */
export const MIN_NAME_LENGTH = 2

export function requiredRule(message: string): ValidationRule {
  return (value) => value.trim().length > 0 || message
}

export function minLengthRule(min: number, message: string): ValidationRule {
  return (value) => value.trim().length >= min || message
}

/**
 * Formato de e-mail, propositalmente frouxo.
 *
 * Uma regex "completa" de e-mail (RFC 5322) recusa endereços válidos e é fonte
 * de bug em cadastro. O que importa aqui é pegar o erro de digitação óbvio; o
 * `validator` do backend dá a palavra final.
 */
export function emailRule(message = 'Informe um e-mail válido.'): ValidationRule {
  return (value) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.trim()) || message
}

export function passwordRule(): ValidationRule {
  return (value) => {
    if (value.length < MIN_PASSWORD_LENGTH) {
      return `A senha precisa ter ao menos ${MIN_PASSWORD_LENGTH} caracteres.`
    }
    if (!/[A-Z]/.test(value)) {
      return 'A senha precisa conter ao menos uma letra maiúscula.'
    }
    return true
  }
}

export function matchesRule(other: () => string, message: string): ValidationRule {
  return (value) => value === other() || message
}

export interface PasswordStrength {
  /** 0 a 4 — quantos critérios a senha cumpre além do comprimento mínimo. */
  score: number
  label: string
  color: string
}

/**
 * Medidor de força da senha, para orientar sem bloquear.
 *
 * Deliberadamente simples: comprimento e variedade de caracteres. Não é um
 * `zxcvbn` — não estima entropia real nem consulta listas de senhas vazadas, e
 * não deve ser lido como garantia de segurança. Serve para o operador perceber
 * que "12345678" passa no mínimo e ainda assim é ruim.
 */
export function passwordStrength(value: string): PasswordStrength {
  if (!value) return { score: 0, label: '', color: 'grey' }

  const criteria = [
    value.length >= MIN_PASSWORD_LENGTH,
    value.length >= 12,
    /[a-z]/.test(value) && /[A-Z]/.test(value),
    /\d/.test(value),
    /[^\w\s]/.test(value),
  ]
  const score = criteria.filter(Boolean).length

  if (score <= 1) return { score: 1, label: 'Fraca', color: 'error' }
  if (score === 2) return { score: 2, label: 'Razoável', color: 'warning' }
  if (score === 3) return { score: 3, label: 'Boa', color: 'info' }
  return { score: 4, label: 'Forte', color: 'success' }
}
