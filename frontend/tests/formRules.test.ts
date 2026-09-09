import { describe, it, expect } from 'vitest'
import {
  passwordRule,
  passwordStrength,
  emailRule,
  minLengthRule,
  matchesRule,
} from '../src/utils/formRules'

describe('formRules - passwordRule', () => {
  const rule = passwordRule()

  it('reprova senhas com menos de 8 caracteres', () => {
    expect(rule('1234567')).toBe('A senha precisa ter ao menos 8 caracteres.')
    expect(rule('')).toBe('A senha precisa ter ao menos 8 caracteres.')
  })

  it('reprova senhas sem letra maiúscula', () => {
    expect(rule('12345678')).toBe('A senha precisa conter ao menos uma letra maiúscula.')
    expect(rule('12345678a')).toBe('A senha precisa conter ao menos uma letra maiúscula.')
    expect(rule('senhafraca123')).toBe('A senha precisa conter ao menos uma letra maiúscula.')
  })

  it('aprova senhas válidas com 8 ou mais caracteres e ao menos uma maiúscula', () => {
    expect(rule('12345678aL')).toBe(true)
    expect(rule('12345678A')).toBe(true)
    expect(rule('12345678Al')).toBe(true)
    expect(rule('SenhaForte123')).toBe(true)
  })
})

describe('formRules - passwordStrength', () => {
  it('mede a força da senha', () => {
    expect(passwordStrength('').score).toBe(0)
    expect(passwordStrength('12345678').score).toBe(2) // len>=8, digit
    expect(passwordStrength('12345678aL').score).toBe(3) // len>=8, upper+lower, digit
    expect(passwordStrength('12345678aL').label).toBe('Boa')
  })
})
