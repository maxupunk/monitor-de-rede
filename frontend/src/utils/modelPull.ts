import { apiService } from '@/services/apiService'
import type { ModelPullProgress } from '@/bindings/ModelPullProgress'
import { readSseJson } from './sseReader'

function isPullProgress(event: unknown): event is ModelPullProgress {
  return (
    typeof event === 'object' &&
    event !== null &&
    typeof (event as { status?: unknown }).status === 'string' &&
    typeof (event as { done?: unknown }).done === 'boolean'
  )
}

/**
 * Acompanha o download de um modelo num servidor local (Ollama ou Ollaya)
 * pelo SSE do backend. Cada evento vai para `onProgress`; um evento com erro
 * rejeita a promessa com a mensagem dele.
 */
export async function streamModelPull(
  path: string,
  body: unknown,
  signal: AbortSignal,
  onProgress: (progress: ModelPullProgress) => void
): Promise<void> {
  const response = await apiService.postStream(path, body, signal)
  const reader = response.body?.getReader()
  if (!reader) throw new Error('Não foi possível inicializar leitura do download')

  let failure: string | null = null
  await readSseJson(reader, (event) => {
    if (!isPullProgress(event) || failure) return
    onProgress(event)
    if (event.error) failure = event.error
  })
  if (failure) throw new Error(failure)
}
