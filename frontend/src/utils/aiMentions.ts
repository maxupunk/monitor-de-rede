/**
 * Regras puras do `@` no campo do chat: achar o termo que está sendo
 * digitado, trocar pelo recurso escolhido e saber quais marcações ainda estão
 * no texto.
 */
import type { AiMention } from '@/bindings/AiMention'
import type { AiMentionKind } from '@/bindings/AiMentionKind'

export type { AiMention, AiMentionKind }

/** `@` digitado no início ou depois de espaço, até o cursor. */
export interface MentionQuery {
  /** Posição do `@`. */
  start: number
  /** O que vem depois do `@`. */
  query: string
}

const MAX_QUERY_CHARS = 40

/** O termo do `@` sob o cursor, ou `null` quando o cursor não está num. */
export function findMentionQuery(text: string, caret: number): MentionQuery | null {
  const before = text.slice(0, caret)
  const start = before.lastIndexOf('@')
  if (start < 0) return null
  if (start > 0 && !/\s/.test(before[start - 1])) return null
  const query = before.slice(start + 1)
  if (query.length > MAX_QUERY_CHARS || /[\n@]/.test(query)) return null
  // Espaço no fim já fecha a busca: o usuário seguiu escrevendo.
  if (/\s$/.test(query)) return null
  // Rótulo tem no máximo três palavras; passou disso é a frase continuando.
  if (query.split(/\s+/).length > 3) return null
  return { start, query }
}

/** Troca `@termo` por `@Rótulo ` e devolve onde o cursor fica. */
export function insertMention(
  text: string,
  query: MentionQuery,
  caret: number,
  label: string
): { text: string; caret: number } {
  const token = `@${label} `
  const next = text.slice(0, query.start) + token + text.slice(caret).replace(/^ /, '')
  return { text: next, caret: query.start + token.length }
}

function sameMention(a: AiMention, b: AiMention): boolean {
  return a.kind === b.kind && a.id === b.id
}

/** Acrescenta sem repetir. */
export function addMention(list: AiMention[], mention: AiMention): AiMention[] {
  return list.some((item) => sameMention(item, mention)) ? list : [...list, mention]
}

/** As marcações cujo `@Rótulo` ainda está no texto — apagar o texto desmarca. */
export function mentionsInText(text: string, mentions: AiMention[]): AiMention[] {
  return mentions.filter((mention) => text.includes(`@${mention.label}`))
}

export interface MentionKindMeta {
  label: string
  icon: string
  color: string
}

const KIND_META: Record<AiMentionKind, MentionKindMeta> = {
  device: { label: 'Dispositivo', icon: 'mdi-router-network', color: 'primary' },
  monitor: { label: 'Monitor', icon: 'mdi-monitor-eye', color: 'indigo' },
  container: { label: 'Container', icon: 'mdi-docker', color: 'info' },
  source: { label: 'Fonte', icon: 'mdi-database-search-outline', color: 'teal' },
}

export function mentionKindMeta(kind: AiMentionKind): MentionKindMeta {
  return KIND_META[kind]
}

/** Como a marcação aparece no histórico enviado à IA: tipo, rótulo e id. */
export function describeMention(mention: AiMention): string {
  const kind = KIND_META[mention.kind].label.toLowerCase()
  return mention.kind === 'source'
    ? `${kind} ${mention.label}`
    : `${kind} "${mention.label}" (id ${mention.id})`
}
