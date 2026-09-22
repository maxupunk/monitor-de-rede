/**
 * Conversas do Assistente IA que ficaram no `localStorage`.
 *
 * O histórico agora vive no servidor (`/api/ai/conversations`); este módulo
 * só existe para levar o que foi salvo no navegador antes disso e depois
 * limpar. Leitura e escrita são tolerantes a falha — modo privado ou JSON
 * corrompido resultam em lista vazia, nunca em tela quebrada.
 */

const KEY_PREFIX = 'netmonitor.ai.conversations.'
const TITLE_CHARS = 60

export interface LocalConversation<TMessage> {
  id: string
  title: string
  updatedAt: string
  messages: TMessage[]
}

function storageKey(userKey: string): string {
  return `${KEY_PREFIX}${userKey || 'anon'}`
}

function isLocalConversation<T>(value: unknown): value is LocalConversation<T> {
  if (!value || typeof value !== 'object') return false
  const item = value as Record<string, unknown>
  return (
    typeof item.id === 'string' &&
    typeof item.title === 'string' &&
    typeof item.updatedAt === 'string' &&
    Array.isArray(item.messages)
  )
}

export function loadLocalConversations<T>(userKey: string): LocalConversation<T>[] {
  try {
    const raw = localStorage.getItem(storageKey(userKey))
    if (!raw) return []
    const parsed: unknown = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed.filter((item) => isLocalConversation<T>(item)) : []
  } catch {
    return []
  }
}

export function clearLocalConversations(userKey: string): void {
  try {
    localStorage.removeItem(storageKey(userKey))
  } catch {
    // sem armazenamento, nada a limpar
  }
}

/** Título a partir da primeira pergunta, cortado numa palavra inteira. */
export function conversationTitle(firstQuestion: string | undefined): string {
  const text = (firstQuestion ?? '').replace(/\s+/g, ' ').trim()
  if (!text) return 'Nova conversa'
  if (text.length <= TITLE_CHARS) return text
  const cut = text.slice(0, TITLE_CHARS)
  const lastSpace = cut.lastIndexOf(' ')
  return `${(lastSpace > 20 ? cut.slice(0, lastSpace) : cut).trim()}…`
}
