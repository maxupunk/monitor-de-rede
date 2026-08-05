import type { ZabbixItemValueType } from '#models/zabbix_template_item'

/**
 * Parser para o formato oficial de export de templates do Zabbix (JSON), conforme
 * https://www.zabbix.com/documentation/current/en/manual/xml_export_import/templates
 *
 * Suporta apenas o subconjunto necessário para monitoramento via SNMP (polling de
 * valores escalares): itens do tipo SNMP_AGENT, com o passo de pré-processamento
 * MULTIPLIER (usado por fabricantes para expressar tensão/corrente em décimos de
 * unidade). Outros tipos de item (TRAPPER, HTTP_AGENT, SCRIPT, etc.), regras de
 * descoberta (LLD), triggers e gráficos do template são ignorados — o alerta e a
 * visualização de histórico já são responsabilidade dos módulos próprios do sistema.
 */

const SUPPORTED_ITEM_TYPE = 'SNMP_AGENT'

// O exportador do Zabbix omite campos que estão no valor padrão. O padrão de
// value_type para itens numéricos criados sem especificação é "Numeric (unsigned)".
const DEFAULT_VALUE_TYPE: ZabbixItemValueType = 'UNSIGNED'

const VALID_VALUE_TYPES: ZabbixItemValueType[] = ['FLOAT', 'UNSIGNED', 'TEXT', 'CHAR', 'LOG']

export class ZabbixTemplateParseError extends Error {}

export interface ParsedZabbixItem {
  uuid: string | null
  name: string
  key: string
  snmpOid: string
  valueType: ZabbixItemValueType
  units: string | null
  multiplier: number | null
}

export interface ParsedZabbixTemplate {
  uuid: string | null
  name: string
  description: string | null
  zabbixVersion: string
  items: ParsedZabbixItem[]
  /** Itens presentes no template que não são SNMP_AGENT e por isso não foram importados. */
  skippedItems: Array<{ name: string; type: string }>
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : []
}

function parseMultiplier(item: Record<string, unknown>): number | null {
  const steps = asArray(item.preprocessing)
  for (const step of steps) {
    const stepObj = asRecord(step)
    if (!stepObj || stepObj.type !== 'MULTIPLIER') continue
    const params = asArray(stepObj.parameters)
    const raw = params[0]
    const num = typeof raw === 'string' || typeof raw === 'number' ? Number(raw) : Number.NaN
    if (!Number.isNaN(num)) return num
  }
  return null
}

function parseItem(rawItem: unknown): {
  item: ParsedZabbixItem | null
  type: string
  name: string
} {
  const item = asRecord(rawItem)
  if (!item) return { item: null, type: 'unknown', name: 'unknown' }

  const type = typeof item.type === 'string' ? item.type : 'unknown'
  const name = typeof item.name === 'string' ? item.name : String(item.key ?? 'sem nome')

  if (type !== SUPPORTED_ITEM_TYPE) {
    return { item: null, type, name }
  }

  const snmpOid = typeof item.snmp_oid === 'string' ? item.snmp_oid.trim() : ''
  const key = typeof item.key === 'string' ? item.key : null
  if (!snmpOid || !key) {
    return { item: null, type, name }
  }

  const rawValueType = typeof item.value_type === 'string' ? item.value_type : null
  const valueType =
    rawValueType && VALID_VALUE_TYPES.includes(rawValueType as ZabbixItemValueType)
      ? (rawValueType as ZabbixItemValueType)
      : DEFAULT_VALUE_TYPE

  return {
    type,
    name,
    item: {
      uuid: typeof item.uuid === 'string' ? item.uuid : null,
      name,
      key,
      snmpOid,
      valueType,
      units: typeof item.units === 'string' && item.units.length > 0 ? item.units : null,
      multiplier: parseMultiplier(item),
    },
  }
}

/**
 * @param input Conteúdo bruto do arquivo exportado pelo Zabbix (JSON), como string ou já parseado.
 * @throws ZabbixTemplateParseError quando a estrutura não corresponde a um export de template Zabbix válido.
 */
export function parseZabbixTemplateExport(input: string | unknown): ParsedZabbixTemplate[] {
  let data: unknown = input
  if (typeof input === 'string') {
    try {
      data = JSON.parse(input)
    } catch {
      throw new ZabbixTemplateParseError('O arquivo não é um JSON válido.')
    }
  }

  const root = asRecord(data)
  const zabbixExport = root ? asRecord(root.zabbix_export) : null
  if (!zabbixExport) {
    throw new ZabbixTemplateParseError(
      'Estrutura inesperada: esperava um export de template do Zabbix (objeto "zabbix_export").'
    )
  }

  const templates = asArray(zabbixExport.templates)
  if (templates.length === 0) {
    throw new ZabbixTemplateParseError(
      'O export não contém nenhum template ("zabbix_export.templates").'
    )
  }

  const zabbixVersion =
    typeof zabbixExport.version === 'string' ? zabbixExport.version : 'desconhecida'

  return templates.map((rawTemplate) => {
    const template = asRecord(rawTemplate)
    if (!template) {
      throw new ZabbixTemplateParseError('Um dos templates do export está malformado.')
    }

    const name =
      typeof template.name === 'string'
        ? template.name
        : typeof template.template === 'string'
          ? template.template
          : null
    if (!name) {
      throw new ZabbixTemplateParseError('Template sem nome ("name"/"template") no export.')
    }

    const items: ParsedZabbixItem[] = []
    const skippedItems: Array<{ name: string; type: string }> = []

    for (const rawItem of asArray(template.items)) {
      const { item, type, name: itemName } = parseItem(rawItem)
      if (item) {
        items.push(item)
      } else {
        skippedItems.push({ name: itemName, type })
      }
    }

    if (items.length === 0) {
      throw new ZabbixTemplateParseError(
        `O template "${name}" não possui itens SNMP_AGENT com OID válido — nada para importar.`
      )
    }

    return {
      uuid: typeof template.uuid === 'string' ? template.uuid : null,
      name,
      description: typeof template.description === 'string' ? template.description : null,
      zabbixVersion,
      items,
      skippedItems,
    }
  })
}
