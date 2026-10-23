import { describe, expect, it } from 'vitest'
import { buildArguments, missingRequired, schemaFields } from '@/utils/scheduledTasks/schemaForm'

const schema = {
  type: 'object',
  properties: {
    count: { type: 'integer', description: 'Pacotes (1 a 5)' },
    target: { type: 'string', description: 'IP ou nome' },
    action: { type: 'string', enum: ['start', 'stop', 'restart'] },
    ports: { type: 'array', items: { type: 'integer' } },
    verbose: { type: 'boolean' },
    value: { description: 'Número ou texto' },
  },
  required: ['target', 'action'],
}

describe('formulário a partir do JSON Schema', () => {
  it('monta um campo por propriedade, obrigatórios primeiro e com o rótulo conhecido', () => {
    const fields = schemaFields(schema)
    expect(fields.map((field) => field.key)).toEqual([
      'target',
      'action',
      'count',
      'ports',
      'verbose',
      'value',
    ])
    const target = fields[0]!
    expect(target.label).toBe('Alvo')
    expect(target.device).toBe(true)
    expect(target.required).toBe(true)
    expect(fields[1]!.kind).toBe('select')
    expect(fields[1]!.options).toEqual(['start', 'stop', 'restart'])
    expect(fields.find((field) => field.key === 'ports')!.itemKind).toBe('integer')
    expect(fields.find((field) => field.key === 'count')!.hint).toBe('Pacotes (1 a 5)')
    expect(fields.find((field) => field.key === 'value')!.kind).toBe('any')
  })

  it('converte para o tipo do contrato e tira o que ficou em branco', () => {
    const fields = schemaFields(schema)
    expect(
      buildArguments(fields, {
        target: '10.0.0.1',
        action: 'restart',
        count: '3',
        ports: ['22', '80'],
        verbose: false,
        value: '150',
      })
    ).toEqual({
      target: '10.0.0.1',
      action: 'restart',
      count: 3,
      ports: [22, 80],
      verbose: false,
      value: 150,
    })
    expect(buildArguments(fields, { target: ' ', count: '', ports: [] })).toEqual({})
  })

  it('lista os obrigatórios em branco', () => {
    const fields = schemaFields(schema)
    expect(missingRequired(fields, { target: '10.0.0.1' })).toEqual(['Preencha "Action"'])
  })
})
