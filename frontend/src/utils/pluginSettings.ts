/**
 * Formulários gerados a partir do esquema de um plugin — parâmetros de ação e
 * configuração guardada. O backend valida de verdade; aqui as regras só dão o
 * retorno imediato na tela. Um lugar só para os dois usos.
 */

import {
  ruleHolds,
  visibleWhenOf,
  widgetOf,
  widgetRule,
  type VisibleWhen,
  type Widget,
} from './pluginWidgets'

export type Schema = Record<string, unknown>
export type Rule = (value: unknown) => boolean | string

/** Valor que o backend devolve no lugar de um segredo guardado. */
export const SECRET_MASK = '********'

export interface SchemaField {
  name: string
  kind: 'string' | 'integer' | 'number' | 'boolean' | 'list' | 'objects'
  label: string
  hint?: string
  options?: unknown[]
  /** Nome de cada opção (`enumTitles`), na ordem de `options`. */
  titles?: string[]
  /** Fica em "Opções avançadas". */
  advanced: boolean
  /** Fora da tela; o valor ainda vai (ex.: o nome atual ao renomear). */
  hidden: boolean
  /** Componente do campo (IP, MAC, porta…), que traz a própria validação. */
  widget?: Widget
  /** Seção do formulário. */
  group?: string
  /** Só aparece (e só vale) quando a regra vale — ex.: a senha some na rede aberta. */
  visibleWhen?: VisibleWhen
  /** Ação que lê o valor atual deste segredo, para conferir ao editar. */
  reveal?: string
  secret: boolean
  required: boolean
  rules: Rule[]
  /** Esquema do item (listas de objetos) ou da string (listas simples). */
  items?: Schema
  schema: Schema
}

function asSchema(value: unknown): Schema {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Schema)
    : {}
}

export function propertiesOf(schema: unknown): Record<string, Schema> {
  const properties = asSchema(schema).properties
  return typeof properties === 'object' && properties !== null
    ? (properties as Record<string, Schema>)
    : {}
}

export function requiredOf(schema: unknown): string[] {
  const list = asSchema(schema).required
  return Array.isArray(list) ? list.filter((item): item is string => typeof item === 'string') : []
}

/** Regras de tela para um campo simples. */
export function rulesFor(schema: Schema, required: boolean): Rule[] {
  const rules: Rule[] = []
  if (required) {
    rules.push((value) => (value !== '' && value !== null && value !== undefined) || 'Obrigatório')
  }
  if (typeof schema.pattern === 'string') {
    const pattern = new RegExp(`^(?:${schema.pattern})$`)
    rules.push(
      (value) =>
        value === '' ||
        value === null ||
        value === undefined ||
        value === SECRET_MASK ||
        pattern.test(String(value)) ||
        'Formato inválido'
    )
  }
  if (typeof schema.minLength === 'number') {
    const min = schema.minLength
    rules.push((value) => !value || String(value).length >= min || `Ao menos ${min} caracteres`)
  }
  if (typeof schema.maxLength === 'number') {
    const max = schema.maxLength
    rules.push((value) => String(value ?? '').length <= max || `No máximo ${max} caracteres`)
  }
  if (typeof schema.minimum === 'number') {
    const min = schema.minimum
    rules.push((value) => value === '' || value === null || Number(value) >= min || `Mínimo ${min}`)
  }
  if (typeof schema.maximum === 'number') {
    const max = schema.maximum
    rules.push((value) => value === '' || value === null || Number(value) <= max || `Máximo ${max}`)
  }
  const widget = widgetRule(widgetOf(schema.widget))
  if (widget) rules.push((value) => value === SECRET_MASK || widget(value))
  return rules
}

export function fieldsOf(schema: unknown): SchemaField[] {
  const required = requiredOf(schema)
  const fields = Object.entries(propertiesOf(schema)).map(([name, property]): SchemaField => {
    const type = String(property.type ?? 'string')
    const items = asSchema(property.items)
    const kind: SchemaField['kind'] =
      type === 'array'
        ? items.type === 'object'
          ? 'objects'
          : 'list'
        : ((['integer', 'number', 'boolean'].includes(type)
            ? type
            : 'string') as SchemaField['kind'])
    return {
      name,
      kind,
      label: typeof property.title === 'string' ? property.title : name,
      hint: typeof property.description === 'string' ? property.description : undefined,
      options: Array.isArray(property.enum) ? property.enum : undefined,
      titles: Array.isArray(property.enumTitles) ? property.enumTitles.map(String) : undefined,
      advanced: property.advanced === true,
      hidden: property.hidden === true,
      widget: widgetOf(property.widget),
      group: typeof property.group === 'string' ? property.group : undefined,
      visibleWhen: visibleWhenOf(property.visibleWhen),
      reveal: typeof property.reveal === 'string' ? property.reveal : undefined,
      secret: property.secret === true,
      required: required.includes(name),
      rules:
        kind === 'list' || kind === 'objects' ? [] : rulesFor(property, required.includes(name)),
      items: kind === 'list' || kind === 'objects' ? items : undefined,
      schema: property,
    }
  })
  return sortByOrder(fields, asSchema(schema).order)
}

/**
 * A ordem da tela vem de `order` (o JSON não guarda a ordem das chaves e o
 * `jsonb` do PostgreSQL a reescreve); campo fora da lista vai para o fim.
 */
function sortByOrder(fields: SchemaField[], order: unknown): SchemaField[] {
  if (!Array.isArray(order)) return fields
  const position = (name: string) => {
    const index = order.indexOf(name)
    return index < 0 ? order.length : index
  }
  return [...fields].sort((a, b) => position(a.name) - position(b.name))
}

/** Os segredos que o formulário lê do equipamento ao editar (`reveal`). */
export function revealFields(schema: unknown): { name: string; action: string }[] {
  return fieldsOf(schema).flatMap((field) =>
    field.secret && field.reveal ? [{ name: field.name, action: field.reveal }] : []
  )
}

/** Os valores com os padrões do esquema — é sobre eles que `visibleWhen` decide. */
export function effectiveValues(
  schema: unknown,
  values: Record<string, unknown>
): Record<string, unknown> {
  const effective: Record<string, unknown> = {}
  for (const [name, property] of Object.entries(propertiesOf(schema))) {
    const given = values[name]
    if (given !== undefined && given !== null) effective[name] = given
    else if (property.default !== undefined) effective[name] = property.default
  }
  return effective
}

/** O campo aparece com estes valores (a regra `visibleWhen` vale)? */
export function isVisible(
  field: SchemaField,
  schema: unknown,
  values: Record<string, unknown>
): boolean {
  return ruleHolds(field.visibleWhen, effectiveValues(schema, values))
}

/** Uma seção do formulário (`group`); a sem título é a dos campos soltos. */
export interface FieldSection {
  title?: string
  fields: SchemaField[]
}

/** Agrupa os campos pela seção, na ordem em que cada seção aparece primeiro. */
export function sectionsOf(fields: SchemaField[]): FieldSection[] {
  const sections: FieldSection[] = []
  for (const field of fields) {
    const section = sections.find((item) => item.title === field.group)
    if (section) section.fields.push(field)
    else sections.push({ title: field.group, fields: [field] })
  }
  return sections
}

/** Um objeto com os valores padrão do esquema (item novo de uma lista). */
export function defaultsOf(schema: unknown): Record<string, unknown> {
  const value: Record<string, unknown> = {}
  for (const field of fieldsOf(schema)) {
    if (field.schema.default !== undefined) value[field.name] = field.schema.default
    else if (field.kind === 'boolean') value[field.name] = false
    else if (field.kind === 'list' || field.kind === 'objects') value[field.name] = []
    else if (field.kind === 'string') value[field.name] = ''
  }
  return value
}

/** Rótulo de uma opção de `enum` (o vazio vira "manter"). */
export function optionLabel(option: unknown): string {
  return option === '' ? '(manter o atual)' : String(option)
}

/** As opções de um campo com o nome que a tela mostra (`enumTitles` ou o valor). */
export function optionsOf(field: SchemaField): { title: string; value: unknown }[] {
  return (field.options ?? []).map((option, index) => ({
    title: field.titles?.[index] ?? optionLabel(option),
    value: option,
  }))
}

/** Só os valores que o campo aceita — o que o equipamento tem pode não estar entre as opções. */
export function acceptedValues(
  schema: unknown,
  values: Record<string, unknown>
): Record<string, unknown> {
  const fields = new Map(fieldsOf(schema).map((field) => [field.name, field]))
  return Object.fromEntries(
    Object.entries(values).filter(([name, value]) => {
      const field = fields.get(name)
      return field !== undefined && (!field.options || field.options.includes(value))
    })
  )
}

/** Converte o que o formulário tem para o que o backend espera (também nos itens de lista). */
export function normalizeValue(
  schema: unknown,
  value: Record<string, unknown>
): Record<string, unknown> {
  const output: Record<string, unknown> = { ...value }
  for (const field of fieldsOf(schema)) {
    // Campo que a regra esconde não existe: não vai para o backend.
    if (!isVisible(field, schema, value)) {
      delete output[field.name]
      continue
    }
    const current = output[field.name]
    if ((field.kind === 'integer' || field.kind === 'number') && current !== undefined) {
      if (current === '' || current === null) delete output[field.name]
      else output[field.name] = Number(current)
    } else if (field.kind === 'objects' && Array.isArray(current)) {
      output[field.name] = current.map((item) =>
        typeof item === 'object' && item !== null && !Array.isArray(item)
          ? normalizeValue(field.items, item as Record<string, unknown>)
          : item
      )
    }
  }
  return output
}

/** Parâmetros de uma ação: valores convertidos, sem os campos vazios. */
export function paramsOf(schema: unknown, value: Record<string, unknown>) {
  return Object.fromEntries(
    Object.entries(normalizeValue(schema, value)).filter(
      ([, item]) => item !== '' && item !== null && item !== undefined
    )
  )
}
