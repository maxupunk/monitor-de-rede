import { describe, expect, it } from 'vitest'
import {
  SECRET_MASK,
  acceptedValues,
  defaultsOf,
  fieldsOf,
  normalizeValue,
  isVisible,
  optionsOf,
  paramsOf,
  sectionsOf,
} from '@/utils/pluginSettings'

const schema = {
  type: 'object',
  required: ['country'],
  properties: {
    country: { type: 'string', title: 'País', pattern: '[A-Z]{2}' },
    usteer: { type: 'boolean', default: true },
    channel: { type: 'integer', minimum: 1, maximum: 165 },
    tags: { type: 'array', items: { type: 'string' } },
    networks: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          ssid: { type: 'string' },
          key: { type: 'string', secret: true, minLength: 8 },
          vlan: { type: 'integer' },
        },
      },
    },
  },
}

describe('pluginSettings', () => {
  it('classifica os campos do esquema', () => {
    const fields = fieldsOf(schema)
    expect(fields.map((field) => [field.name, field.kind])).toEqual([
      ['country', 'string'],
      ['usteer', 'boolean'],
      ['channel', 'integer'],
      ['tags', 'list'],
      ['networks', 'objects'],
    ])
    expect(fields[0]?.label).toBe('País')
    expect(fields[0]?.required).toBe(true)
    const key = fieldsOf(fields[4]?.items).find((field) => field.name === 'key')
    expect(key?.secret).toBe(true)
  })

  it('segue a ordem declarada em `order`; o resto vai para o fim', () => {
    const ordered = { ...schema, order: ['networks', 'country'] }
    expect(fieldsOf(ordered).map((field) => field.name)).toEqual([
      'networks',
      'country',
      'usteer',
      'channel',
      'tags',
    ])
  })

  it('regras aceitam o segredo mascarado e recusam o formato errado', () => {
    const country = fieldsOf(schema)[0]!
    const check = (value: unknown) => country.rules.map((rule) => rule(value))
    expect(check('BR').every((result) => result === true)).toBe(true)
    expect(check('')).toContain('Obrigatório')
    expect(check('Brasil')).toContain('Formato inválido')

    const key = fieldsOf(fieldsOf(schema)[4]?.items).find((field) => field.name === 'key')!
    expect(key.rules.every((rule) => rule(SECRET_MASK) === true)).toBe(true)
    expect(key.rules.map((rule) => rule('curta'))).toContain('Ao menos 8 caracteres')
  })

  it('padrões de item novo e conversão para o backend', () => {
    expect(defaultsOf(schema)).toEqual({
      country: '',
      usteer: true,
      tags: [],
      networks: [],
    })
    expect(normalizeValue(schema, { channel: '36', country: 'BR' })).toEqual({
      channel: 36,
      country: 'BR',
    })
    expect(normalizeValue(schema, { channel: '' })).toEqual({})
    expect(normalizeValue(schema, { networks: [{ id: 'a1', ssid: 'Loja', vlan: '10' }] })).toEqual({
      networks: [{ id: 'a1', ssid: 'Loja', vlan: 10 }],
    })
    expect(paramsOf(schema, { country: '', channel: '6', usteer: false })).toEqual({
      channel: 6,
      usteer: false,
    })
  })
})

describe('apresentação dos campos', () => {
  const radio = {
    type: 'object',
    properties: {
      channel: {
        type: 'string',
        enum: ['', 'auto', '1'],
        enumTitles: ['Manter', 'Automático', '1'],
      },
      txpower: { type: 'integer', advanced: true },
      old: { type: 'string', hidden: true },
    },
  }

  it('opções com o nome amigável, campos avançados e ocultos marcados', () => {
    const [channel, txpower, old] = fieldsOf(radio)
    expect(optionsOf(channel!)).toEqual([
      { title: 'Manter', value: '' },
      { title: 'Automático', value: 'auto' },
      { title: '1', value: '1' },
    ])
    expect(txpower?.advanced).toBe(true)
    expect(old?.hidden).toBe(true)
  })

  it('o que o equipamento tem fora das opções não entra no formulário', () => {
    expect(acceptedValues(radio, { channel: '14', txpower: 20, other: 1 })).toEqual({ txpower: 20 })
  })
})

describe('componentes, seções e campos condicionais', () => {
  const rede = {
    type: 'object',
    properties: {
      ssid: { type: 'string', title: 'Nome', group: 'Básico' },
      encryption: { type: 'string', enum: ['none', 'psk2'], default: 'psk2', group: 'Segurança' },
      key: {
        type: 'string',
        title: 'Senha',
        secret: true,
        group: 'Segurança',
        visibleWhen: { field: 'encryption', notIn: ['none'] },
      },
      gateway: { type: 'string', title: 'Gateway', widget: 'ip', group: 'Básico' },
    },
    order: ['ssid', 'gateway', 'encryption', 'key'],
  }

  it('a senha some na rede aberta e o valor escondido não vai ao backend', () => {
    const key = fieldsOf(rede).find((field) => field.name === 'key')
    expect(key && isVisible(key, rede, {})).toBe(true)
    expect(key && isVisible(key, rede, { encryption: 'none' })).toBe(false)
    expect(paramsOf(rede, { ssid: 'Loja', encryption: 'none', key: 'x' })).toEqual({
      ssid: 'Loja',
      encryption: 'none',
    })
  })

  it('as seções seguem a ordem dos campos e o componente traz a própria regra', () => {
    const sections = sectionsOf(fieldsOf(rede))
    expect(sections.map((section) => section.title)).toEqual(['Básico', 'Segurança'])
    expect(sections[0]?.fields.map((field) => field.name)).toEqual(['ssid', 'gateway'])
    const gateway = fieldsOf(rede).find((field) => field.name === 'gateway')
    expect(gateway?.widget).toBe('ip')
    expect(gateway?.rules.map((rule) => rule('10.0.0.999'))).toContain(
      'Precisa ser um endereço IP (ex.: 192.168.1.1)'
    )
    expect(gateway?.rules.every((rule) => rule('10.0.0.1') === true)).toBe(true)
  })
})
