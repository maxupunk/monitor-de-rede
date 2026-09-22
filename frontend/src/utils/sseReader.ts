/**
 * Lê um corpo `text/event-stream` recebido por `fetch` e entrega o JSON de
 * cada linha `data:`. Linhas de comentário (`:`), keep-alive e JSON parcial
 * ou inválido são ignorados.
 */
export async function readSseJson(
  reader: ReadableStreamDefaultReader<Uint8Array>,
  onEvent: (event: unknown) => void
): Promise<void> {
  const decoder = new TextDecoder()
  let buffer = ''

  const handleLine = (line: string) => {
    const trimmed = line.trim()
    if (!trimmed.startsWith('data:')) return
    const json = trimmed.slice('data:'.length).trim()
    try {
      onEvent(JSON.parse(json))
    } catch {
      // keep-alive ou linha parcial: não é evento
    }
  }

  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() ?? ''
    lines.forEach(handleLine)
  }
  buffer += decoder.decode()
  if (buffer) handleLine(buffer)
}
