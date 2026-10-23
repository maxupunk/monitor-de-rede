/**
 * Formulário dos argumentos de uma ferramenta a partir do JSON Schema dela —
 * o mesmo contrato que a IA recebe. Uma ferramenta nova no catálogo ganha
 * formulário sem código novo na tela.
 */
import { TOOL_ARG_LABELS } from '@/components/ai/aiToolMeta'

export type SchemaFieldKind = 'text' | 'integer' | 'number' | 'boolean' | 'select' | 'list' | 'any'

export interface SchemaField {
  key: string
  label: string
  hint?: string
  kind: SchemaFieldKind
  required: boolean
  /** `select`: os valores aceitos. */
  options?: string[]
  /** `list`: o tipo de cada item. */
  itemKind?: 'text' | 'integer'
  /** Campo que nomeia um equipamento: a tela sugere os cadastrados. */
  device?: boolean
}

type JsonSchema = Record<string, unknown>

const LABELS = new Map<string, string>(TOOL_ARG_LABELS)

/** Chaves que recebem nome, IP ou id de um equipamento. */
const DEVICE_KEYS = new Set(['device', 'identifier', 'hostname', 'target'])

function asSchema(value: unknown): JsonSchema {
  return value !== null && typeof value === 'object' ? (value as JsonSchema) : {}
}

/** "alert_id" → "Alert id" quando não há rótulo conhecido. */
export function fieldLabel(key: string): string {
  const known = LABELS.get(key)
  if (known) return known
  const words = key.replace(/_/g, ' ').trim()
  return words.charAt(0).toUpperCase() + words.slice(1)
}

function kindOf(spec: JsonSchema): Pick<SchemaField, 'kind' | 'options' | 'itemKind'> {
  if (Array.isArray(spec.enum)) {
    return { kind: 'select', options: spec.enum.map((option) => String(option)) }
  }
  switch (spec.type) {
    case 'string':
      return { kind: 'text' }
    case 'integer':
      return { kind: 'integer' }
    case 'number':
      return { kind: 'number' }
    case 'boolean':
      return { kind: 'boolean' }
    case 'array':
      return {
        kind: 'list',
        itemKind: asSchema(spec.items).type === 'integer' ? 'integer' : 'text',
      }
    default:
      return { kind: 'any' }
  }
}

/** Os campos do formulário, obrigatórios primeiro e na ordem do contrato. */
export function schemaFields(schema: JsonSchema): SchemaField[] {
  const properties = asSchema(schema.properties)
  const required = new Set(Array.isArray(schema.required) ? schema.required.map(String) : [])
  const fields = Object.entries(properties).map(([key, value]) => {
    const spec = asSchema(value)
    const field: SchemaField = {
      key,
      label: fieldLabel(key),
      required: required.has(key),
      ...kindOf(spec),
    }
    if (typeof spec.description === 'string') field.hint = spec.description
    if (DEVICE_KEYS.has(key) && field.kind === 'text') field.device = true
    return field
  })
  return [...fields.filter((field) => field.required), ...fields.filter((field) => !field.required)]
}

function isBlank(value: unknown): boolean {
  return (
    value === undefined ||
    value === null ||
    (typeof value === 'string' && value.trim() === '') ||
    (Array.isArray(value) && value.length === 0)
  )
}

/**
 * O valor digitado no tipo do contrato ("5" → 5). Campo em branco sai do
 * objeto: a ferramenta usa o padrão dela.
 */
export function coerceValue(field: SchemaField, value: unknown): unknown {
  if (isBlank(value)) return undefined
  switch (field.kind) {
    case 'integer': {
      const number = Number(value)
      return Number.isFinite(number) ? Math.trunc(number) : value
    }
    case 'number': {
      const number = Number(value)
      return Number.isFinite(number) ? number : value
    }
    case 'boolean':
      return Boolean(value)
    case 'list': {
      const items = Array.isArray(value) ? value : String(value).split(',')
      return items
        .map((item) => String(item).trim())
        .filter((item) => item !== '')
        .map((item) => (field.itemKind === 'integer' && /^-?\d+$/.test(item) ? Number(item) : item))
    }
    case 'any': {
      const text = String(value).trim()
      return /^-?\d+(\.\d+)?$/.test(text) ? Number(text) : text
    }
    default:
      return value
  }
}

/** Monta o objeto de argumentos, sem os campos em branco. */
export function buildArguments(
  fields: SchemaField[],
  values: Record<string, unknown>
): Record<string, unknown> {
  const result: Record<string, unknown> = {}
  for (const field of fields) {
    const value = coerceValue(field, values[field.key])
    if (value !== undefined) result[field.key] = value
  }
  return result
}

/** Campos obrigatórios em branco. */
export function missingRequired(fields: SchemaField[], values: Record<string, unknown>): string[] {
  return fields
    .filter((field) => field.required && field.kind !== 'boolean' && isBlank(values[field.key]))
    .map((field) => `Preencha "${field.label}"`)
}
