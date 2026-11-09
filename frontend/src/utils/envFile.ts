/**
 * O `.env` no modo lista do editor: cada linha vira uma entrada e volta ao
 * texto **igual** ao que era — comentários, linhas em branco e a ordem
 * ficam. O backend guarda o texto; a comparação entre versões (o diff) é
 * dele (`POST …/env/preview`), não daqui.
 */

export type EnvLine =
  | { kind: 'variable'; key: string; value: string; exported: boolean }
  | { kind: 'other'; text: string }

const KEY = /^[A-Za-z_][A-Za-z0-9_]*$/

/** Linhas do arquivo como entradas editáveis. */
export function parseEnv(content: string): EnvLine[] {
  if (content === '') return []
  const lines = content.replace(/\r\n/g, '\n').split('\n')
  if (lines.at(-1) === '') lines.pop()
  return lines.map((text) => {
    const trimmed = text.trim()
    if (trimmed === '' || trimmed.startsWith('#')) return { kind: 'other', text }
    const exported = trimmed.startsWith('export ')
    const body = exported ? trimmed.slice('export '.length) : trimmed
    const equals = body.indexOf('=')
    const key = equals > 0 ? body.slice(0, equals).trim() : ''
    if (!KEY.test(key)) return { kind: 'other', text }
    return { kind: 'variable', key, value: body.slice(equals + 1), exported }
  })
}

/** O texto de volta. */
export function serializeEnv(lines: EnvLine[]): string {
  if (lines.length === 0) return ''
  return (
    lines
      .map((line) =>
        line.kind === 'other'
          ? line.text
          : `${line.exported ? 'export ' : ''}${line.key}=${line.value}`
      )
      .join('\n') + '\n'
  )
}

/** Nome de variável aceito. */
export function isValidEnvKey(key: string): boolean {
  return KEY.test(key)
}

/** O valor parece segredo? Decide se o campo nasce mascarado. */
export function looksSecret(key: string): boolean {
  return /(PASS|SECRET|TOKEN|KEY|PRIVATE|CREDENTIAL|AUTH)/i.test(key)
}
