/**
 * `SnmpClient.get()` devolve `null`/`undefined` para OIDs sem resposta do
 * agente — os coletores (CPU, memória, sistema) repetiam a mesma checagem
 * "existe e converte" para cada OID lido. Centralizado aqui para não divergir
 * entre coletores conforme novos MIBs forem adicionados.
 */

/** Converte a resposta de um OID numérico, ou `undefined` se ausente/inválido */
export function snmpNumber(value: unknown): number | undefined {
  if (value === null || value === undefined) return undefined
  const num = Number(value)
  return Number.isNaN(num) ? undefined : num
}

/** Converte a resposta de um OID textual, ou `undefined` se ausente/vazio */
export function snmpString(value: unknown): string | undefined {
  if (value === null || value === undefined) return undefined
  const text = String(value)
  return text.length > 0 ? text : undefined
}
