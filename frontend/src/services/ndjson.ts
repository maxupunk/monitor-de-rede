/**
 * Consome apenas as linhas completas de um fluxo NDJSON.
 *
 * `ReadableStream` pode dividir um mesmo objeto JSON entre vários chunks ou
 * entregar centenas deles de uma vez. Manter o restante explicitamente evita
 * perda de eventos quando o backend Rust produz resultados muito rapidamente.
 */
export function drainNdjson<T>(
  buffer: string,
  { final = false }: { final?: boolean } = {}
): { events: T[]; remainder: string } {
  const lines = buffer.split(/\r?\n/)
  const remainder = final ? '' : (lines.pop() ?? '')
  const events = lines.reduce<T[]>((parsed, line) => {
    const trimmed = line.trim()
    if (trimmed) parsed.push(JSON.parse(trimmed) as T)
    return parsed
  }, [])

  if (final && remainder.trim()) events.push(JSON.parse(remainder) as T)
  return { events, remainder }
}
